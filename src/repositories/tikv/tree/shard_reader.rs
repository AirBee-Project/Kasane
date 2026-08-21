//! シャード/ノードの読み取りを 1 箇所に集めた層。`/query` と `/data/search` の両方の
//! 読み取り経路(`routing::route_leaves_for_ranges` と `routing::route_leaves_batched` の
//! 読み取り専用呼び出し)がここを通る。書き込み経路(`tree::write`)は使わない。
//!
//! L1(呼び出し元がこの呼び出し1回だけ持ち回るノードキャッシュ)→ L2(プロセス寿命の
//! [`ShardCache`]、`consult_l2` の時だけ参照)→ ネットワーク、の順に引く。

use kasane_logic::FlexId;
use rustc_hash::FxHashMap;

use super::node::load_nodes;
use super::{AppError, NodeCache, Reader, Readers, ShardCache, ShardValue, TableId};

pub(in crate::repositories::tikv) struct ShardReader<'a> {
    l1: &'a NodeCache,
    l2: &'a ShardCache,
    /// `false`(`ConsistencyLevel::Strict`)なら L2 は読みに使わない。ネットワークから
    /// 取れた分はこのフラグに関わらず常に L2 へ書き込む(他のリクエストのために温めておく)。
    consult_l2: bool,
}

impl<'a> ShardReader<'a> {
    pub(in crate::repositories::tikv) fn new(
        l1: &'a NodeCache,
        l2: &'a ShardCache,
        consult_l2: bool,
    ) -> Self {
        Self { l1, l2, consult_l2 }
    }

    /// `regions` を L1 → (`consult_l2` なら) L2 → ネットワークの順に解決する。
    pub(in crate::repositories::tikv) async fn nodes<R: Reader>(
        &self,
        txn: &Readers<R>,
        table_id: TableId,
        regions: &[FlexId],
    ) -> Result<FxHashMap<FlexId, ShardValue>, AppError> {
        let mut out = FxHashMap::default();
        let mut missing = Vec::new();

        {
            let cached = self.l1.lock().expect("node cache lock poisoned");
            for &region in regions {
                match cached.get(&region) {
                    Some(Some(value)) => {
                        out.insert(region, value.clone());
                    }
                    // 未作成領域だと確認済み。読み直さない。
                    Some(None) => {}
                    None => missing.push(region),
                }
            }
        }
        if missing.is_empty() {
            return Ok(out);
        }

        let still_missing = if self.consult_l2 {
            let mut l1 = self.l1.lock().expect("node cache lock poisoned");
            let mut still_missing = Vec::new();
            for region in missing {
                match self.l2.get(table_id, region) {
                    Some(value) => {
                        l1.insert(region, value.clone());
                        if let Some(v) = value {
                            out.insert(region, v);
                        }
                    }
                    None => still_missing.push(region),
                }
            }
            still_missing
        } else {
            missing
        };
        if still_missing.is_empty() {
            return Ok(out);
        }

        let fetched = load_nodes(txn, table_id, &still_missing).await?;

        let mut l1 = self.l1.lock().expect("node cache lock poisoned");
        for region in still_missing {
            let value = fetched.get(&region).cloned();
            l1.insert(region, value.clone());
            // Strict でも、後で BoundedStale を選ぶ誰かのために L2 は温めておく
            // (L2 から読むかどうかだけが consult_l2 で決まる)。
            self.l2.insert(table_id, region, value.clone());
            if let Some(v) = value {
                out.insert(region, v);
            }
        }
        Ok(out)
    }
}
