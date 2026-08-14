//! [`Source::read_subset`] は 1 回のクエリ実行で複数回呼ばれるので、そのたびに
//! トランザクションを開き直すと途中で走った書き込みを一部だけ見た結果が混ざる。
//! クエリ開始時に開いた [`LmdbQuerySnapshot`] を全ソースで共有する。

use heed::Database;
use heed::types::Bytes;
use kasane_logic::{Error as LogicError, FlexId, RangeId, SafeValue, Source, WorkingTree};

use super::LmdbQuerySnapshot;
use super::keys::TableIdAndFlexId;
use super::shard;
use crate::models::id::TableId;

use crate::repositories::traits::DecodeFn;

/// 実行器は複数スレッドから読み得るので、[`Sync`] を得るために断面を排他で包む。
pub struct TableSource<V> {
    snapshot: LmdbQuerySnapshot,
    tables_data: Database<TableIdAndFlexId, Bytes>,
    table_id: TableId,
    decode: DecodeFn<V>,
}

impl<V> TableSource<V> {
    pub fn new(
        snapshot: LmdbQuerySnapshot,
        tables_data: Database<TableIdAndFlexId, Bytes>,
        table_id: TableId,
        decode: DecodeFn<V>,
    ) -> Self {
        Self {
            snapshot,
            tables_data,
            table_id,
            decode,
        }
    }
}

impl super::AppDb {
    /// ストレージのハンドル（`Env` / `Database`）をサービス層へ露出させないための入口。
    pub fn table_source<V>(
        &self,
        table_id: TableId,
        decode: DecodeFn<V>,
        snapshot: LmdbQuerySnapshot,
    ) -> TableSource<V> {
        TableSource::new(snapshot, self.tables_data, table_id, decode)
    }
}

impl<V> Source for TableSource<V>
where
    V: SafeValue + 'static,
{
    type Value = V;

    fn read_subset(&self, bounds: &[RangeId]) -> Result<WorkingTree<V>, LogicError> {
        let txn = self.snapshot.lock().map_err(|_| {
            LogicError::SourceRead("query snapshot was poisoned by a panicking reader".to_string())
        })?;

        let mut flex_ids: Vec<(FlexId, V)> = Vec::new();
        for range in bounds {
            let leaves =
                shard::route_leaves_for_range(&self.tables_data, &txn, self.table_id, range)
                    .map_err(|e| LogicError::SourceRead(e.to_string()))?;

            for region in leaves {
                let arch =
                    shard::load_leaf_archived(&self.tables_data, &txn, self.table_id, &region)
                        .map_err(|e| LogicError::SourceRead(e.to_string()))?;
                let Some(arch) = arch else { continue };

                for (id, raw) in arch.get_range(range) {
                    // 復元できない値（型に合わない格納値）の FlexId は結果に含めない。
                    if let Some(value) = (self.decode)(raw) {
                        flex_ids.push((id, value));
                    }
                }
            }
        }
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
