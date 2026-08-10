//! TiKV 上のシャードツリーを、Kasane-Logic のクエリ入力源として見せるアダプタ。
//!
//! Kasane-Logic の [`Source`] は同期 I/F なので、非同期の TiKV アクセスをその内側で
//! 完了させる必要がある。クエリ実行そのものはサービス層で `spawn_blocking` の中から
//! 呼ばれるため、ここでの `block_on` はブロッキングスレッド上で行われ、
//! 非同期ランタイムのワーカーを塞がない。

use kasane_logic::{Error as LogicError, RangeId, SafeValue, Source, WorkingTree};
use tokio::sync::Mutex;

use crate::models::id::TableId;
use crate::repositories::traits::DecodeFn;

use super::{TikvDb, TikvRead};

/// 1 テーブルを 1 つのクエリ入力源として見せるアダプタ。
///
/// トランザクションを保持しない。`Source` は実行器から複数回・複数スレッドで呼ばれうるため、
/// クライアントだけを持ち、`read_subset` の内側で短命なスナップショットを開く。
pub struct TikvTableSource<V> {
    db: TikvDb,
    table_id: TableId,
    decode: DecodeFn<V>,
}

impl<V> TikvTableSource<V> {
    pub fn new(db: TikvDb, table_id: TableId, decode: DecodeFn<V>) -> Self {
        Self {
            db,
            table_id,
            decode,
        }
    }
}

impl TikvDb {
    /// テーブル 1 つをクエリの入力源として見せるアダプタを作る。
    ///
    /// ストレージのハンドルをサービス層へ露出させないための入口
    /// （LMDB 側の同名メソッドと対になる）。
    pub fn table_source<V>(&self, table_id: TableId, decode: DecodeFn<V>) -> TikvTableSource<V> {
        TikvTableSource::new(self.clone(), table_id, decode)
    }
}

impl<V> Source for TikvTableSource<V>
where
    V: SafeValue + 'static,
{
    type Value = V;

    fn read_subset(&self, bounds: &[RangeId]) -> Result<WorkingTree<V>, LogicError> {
        let handle = tokio::runtime::Handle::current();
        let db = self.db.clone();
        let table_id = self.table_id;
        let bounds = bounds.to_vec();

        let cells = handle.block_on(async move {
            let txn = db
                .client
                .begin_optimistic()
                .await
                .map_err(|e| LogicError::SourceRead(e.to_string()))?;
            let reader = TikvRead {
                txn: Mutex::new(txn),
                _db: &db,
            };

            let mut cells = Vec::new();
            for range in &bounds {
                let got = reader
                    .read_cells_in_range(table_id, range)
                    .await
                    .map_err(|e| LogicError::SourceRead(e.to_string()))?;
                cells.extend(got);
            }
            let _ = reader.txn.into_inner().rollback().await;
            Ok::<_, LogicError>(cells)
        })?;

        // 復元できない値（型に合わない格納値）のセルは結果に含めない。
        // 重なり合う bounds から同じセルを複数回読んでも、いずれも同じ
        // `(FlexId, 値)` なので union がそのまま吸収する。
        Ok(cells
            .into_iter()
            .filter_map(|(id, raw)| (self.decode)(&raw).map(|v| (id, v)))
            .collect())
    }

    fn read_all(self: Box<Self>) -> Result<WorkingTree<V>, LogicError> {
        // テーブル全体の materialize は容量的に現実的でないため提供しない。
        Err(LogicError::Unsupported(
            "full scan of a database-backed table; use a bounded (lazy) query instead",
        ))
    }
}
