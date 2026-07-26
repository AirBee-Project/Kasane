//! LMDB 上のシャードされた FlexTree を、Kasane-Logic のクエリ入力源として見せるアダプタ。
//!
//! Kasane-Logic の [`Source`] は「範囲を指定して読む」ことしか要求しないため、テーブル全体を
//! メモリへ展開せずにクエリの入力になれる。演算そのものはインメモリの作業木
//! （[`FlexTreeCore`]）上で行われ、本アダプタは**入力の読み出し方**だけを担う。

use heed::types::Bytes;
use heed::{Database, Env, WithoutTls};
use kasane_logic::{CellValue, Error as LogicError, FlexId, FlexTreeCore, RangeId, Source};

use super::shard;
use crate::db_init::TableIdAndFlexId;
use crate::models::id::TableId;

/// 格納バイト列を、クエリで扱う値型へ復元する関数。
///
/// テーブルの `data_type` に応じてサービス層が渡す（例: Int なら 4 バイト BE → `i32`）。
/// ユーザー指定の変換表を適用する場合もここに載るため、関数ポインタではなくクロージャを取る。
///
/// `None` を返したセルは**結果から除外される**（変換表に載っていない値や、
/// 型に合わない格納値）。
pub type DecodeFn<V> = std::sync::Arc<dyn Fn(&[u8]) -> Option<V> + Send + Sync>;

/// 1テーブルを 1 つのクエリ入力源として見せるアダプタ。
///
/// 読み取りトランザクションを**保持しない**のが要点。`Source` は rayon 有効時に
/// `Send + Sync` を要求されるうえ、実行器は複数回・複数スレッドから読み得るため、
/// `Env` だけを持ち `read_subset` の内側で短命な読み取りトランザクションを開く。
/// ゼロコピー参照はその内側で所有値へデコードするので、トランザクション外へ漏れない。
pub struct TableSource<V> {
    env: Env<WithoutTls>,
    tables_data: Database<TableIdAndFlexId, Bytes>,
    table_id: TableId,
    decode: DecodeFn<V>,
}

impl<V> TableSource<V> {
    pub fn new(
        env: Env<WithoutTls>,
        tables_data: Database<TableIdAndFlexId, Bytes>,
        table_id: TableId,
        decode: DecodeFn<V>,
    ) -> Self {
        Self {
            env,
            tables_data,
            table_id,
            decode,
        }
    }
}

impl<V> Source for TableSource<V>
where
    V: CellValue + 'static,
{
    /// 演算はインメモリの作業木で行う。ディスク側が担うのは入力の読み出しだけ。
    type Value = V;

    fn read_subset(&self, bounds: &[RangeId]) -> Result<FlexTreeCore<V>, LogicError> {
        let txn = self
            .env
            .read_txn()
            .map_err(|e| LogicError::SourceRead(e.to_string()))?;

        let mut cells: Vec<(FlexId, V)> = Vec::new();
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
                    // 変換できない値（変換表に無い・型が合わない）のセルは結果に含めない。
                    if let Some(value) = (self.decode)(raw) {
                        cells.push((id, value));
                    }
                }
            }
        }
        // 重なり合う bounds から同じセルを複数回読むことはあるが、いずれも
        // まったく同じ `(FlexId, 値)` なので、`from_flexids` の union がそのまま吸収する。
        Ok(cells.into_iter().collect())
    }

    fn read_all(self: Box<Self>) -> Result<FlexTreeCore<V>, LogicError> {
        // テーブル全体の materialize は容量的に現実的でないため提供しない。
        // クエリは必ず領域を指定する遅延評価（`Query::lazy`）経由で実行する。
        Err(LogicError::Unsupported(
            "full scan of a database-backed table; use a bounded (lazy) query instead",
        ))
    }
}
