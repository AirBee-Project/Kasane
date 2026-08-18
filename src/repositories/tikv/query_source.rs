//! TiKV 上のシャードツリーを Kasane-Logic のクエリ入力源として見せるアダプタ。
//!
//! [`Source::read_range_ids`] は 1 回のクエリで複数回呼ばれるので、断面を固定しないと途中で
//! 走った書き込みを一部だけ見た結果が混ざる。
//!
//! ランタイムハンドルを持ち回るのは、[`tokio::runtime::Handle::current`] を `read_range_ids` の
//! 中で呼ぶと、実行器が rayon 側から読みに来たときにランタイム文脈が無くて panic するため。

use kasane_logic::{Error as LogicError, RangeId, SafeValue, Source, WorkingTree};
use tikv_client::Timestamp;
use tokio::runtime::Handle;

use crate::models::id::TableId;
use crate::repositories::traits::DecodeFn;

use super::{TikvDb, TikvRead};

/// トランザクションは保持しない（`Source` は複数回・複数スレッドから呼ばれうる）。
/// 代わりに開始タイムスタンプを保持し、読み取りのたびにその時点のスナップショットを開く。
pub struct TikvTableSource<V> {
    db: TikvDb,
    table_id: TableId,
    decode: DecodeFn<V>,
    /// 構築時に 1 度だけ確定する。
    snapshot_ts: Timestamp,
    /// 構築時（ランタイム文脈内）に捕まえたハンドル。
    handle: Handle,
}

impl TikvDb {
    /// ストレージのハンドルをサービス層へ露出させないための入口。`snapshot_ts` は
    /// [`Storage::query_snapshot`](crate::repositories::Storage::query_snapshot) で
    /// 1 度だけ取ったものを全ソースへ配る。
    pub fn table_source<V>(
        &self,
        table_id: TableId,
        decode: DecodeFn<V>,
        snapshot_ts: Timestamp,
    ) -> TikvTableSource<V> {
        TikvTableSource {
            db: self.clone(),
            table_id,
            decode,
            snapshot_ts,
            handle: Handle::current(),
        }
    }
}

impl<V> Source for TikvTableSource<V>
where
    V: SafeValue + 'static,
{
    type Value = V;

    fn read_range_ids(&self, bounds: &[RangeId]) -> Result<WorkingTree<V>, LogicError> {
        let db = self.db.clone();
        let table_id = self.table_id;
        let bounds = bounds.to_vec();
        let snapshot_ts = self.snapshot_ts.clone();
        let decode = self.decode.clone();

        let started = std::time::Instant::now();
        let flex_ids = self.handle.block_on(async move {
            // 読み取り専用なので commit も rollback も要らない。
            let reader = TikvRead::at(db.client.clone(), snapshot_ts);

            // 1 本ごとに降りると往復が「境界の本数 × 木の深さ」になる。復元関数も渡す。
            reader
                .read_values_in_ranges(table_id, &bounds, decode.as_ref())
                .await
                .map_err(|e| LogicError::SourceRead(e.to_string()))
        })?;

        // kasane_logic.query.source_read はネットワーク待ち＋木構築の合計。ネットワークだけの
        // 時間はここでしか分からないので、フィールドとして残す。1 クエリで複数回発火しうるので
        // debug ではなく trace（`kasane=trace` でのみ出る）に留める。
        tracing::trace!(
            table_id = ?table_id,
            elapsed_ms = started.elapsed().as_millis() as u64,
            value_count = flex_ids.len(),
            "tikv read_range_ids network wait"
        );

        // 重なり合う bounds が同じ FlexId を複数回返しても、値は同じなので union が吸収する。
        Ok(flex_ids.into_iter().collect())
    }

    fn read_all(self: Box<Self>) -> Result<WorkingTree<V>, LogicError> {
        // テーブル全体の materialize は容量的に現実的でない。
        Err(LogicError::Unsupported(
            "full scan of a database-backed table; use a bounded (lazy) query instead",
        ))
    }
}
