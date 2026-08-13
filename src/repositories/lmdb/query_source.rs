//! LMDB 上のシャードされた FlexTree を、Kasane-Logic のクエリ入力源として見せるアダプタ。
//!
//! Kasane-Logic の [`Source`] は「範囲を指定して読む」ことしか要求しないため、テーブル全体を
//! メモリへ展開せずにクエリの入力になれる。演算そのものはインメモリの作業木
//! （[`WorkingTree`]）上で行われ、本アダプタは**入力の読み出し方**だけを担う。
//!
//! # 断面を固定する理由
//!
//! [`Source::read_subset`] は 1 回のクエリ実行で複数回呼ばれる（対象領域ごと、
//! そしてクエリが同じテーブルを複数箇所で参照すればその数だけ）。呼ばれるたびに
//! 読み取りトランザクションを開き直すと、そのつど別のスナップショットを見ることになり、
//! 途中で走った書き込みを一部だけ見た結果が混ざりうる。
//!
//! そこでクエリの開始時に開いた 1 つのトランザクション
//! （[`LmdbQuerySnapshot`](super::LmdbQuerySnapshot)）を全ソースで共有する。
//! TiKV 側が開始タイムスタンプを共有するのと同じ役割。

use heed::Database;
use heed::types::Bytes;
use kasane_logic::{Error as LogicError, FlexId, RangeId, SafeValue, Source, WorkingTree};

use super::LmdbQuerySnapshot;
use super::keys::TableIdAndFlexId;
use super::shard;
use crate::models::id::TableId;

use crate::repositories::traits::DecodeFn;

/// 1テーブルを 1 つのクエリ入力源として見せるアダプタ。
///
/// トランザクションはクエリ全体で 1 つを共有する（`Arc<Mutex<_>>`）。実行器は
/// 複数回・複数スレッドから読み得るので、`Sync` を得るために排他で包んでいる。
/// ゼロコピー参照は `read_subset` の内側で所有値へデコードするので、
/// トランザクション外へ漏れない。
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
    /// テーブル 1 つをクエリの入力源として見せるアダプタを作る。
    ///
    /// ストレージのハンドル（`Env` / `Database`）をサービス層へ露出させないための入口。
    /// サービス層は「どのテーブルを、どう復元して読むか」と、
    /// 「どの断面から読むか」（[`Storage::query_snapshot`](crate::repositories::Storage::query_snapshot)
    /// で 1 度だけ作り、全ソースへ配る）を指定する。
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
    /// 演算はインメモリの作業木で行う。ディスク側が担うのは入力の読み出しだけ。
    type Value = V;

    fn read_subset(&self, bounds: &[RangeId]) -> Result<WorkingTree<V>, LogicError> {
        // クエリ全体で 1 つの断面を共有する。ここでの排他は「同時に触らせない」ためで、
        // トランザクション自体はクエリの開始時に開かれている。
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
        // 重なり合う bounds から同じ FlexId を複数回読むことはあるが、いずれも
        // まったく同じ `(FlexId, 値)` なので、`from_flexids` の union がそのまま吸収する。
        Ok(flex_ids.into_iter().collect())
    }

    fn read_all(self: Box<Self>) -> Result<WorkingTree<V>, LogicError> {
        // テーブル全体の materialize は容量的に現実的でないため提供しない。
        // クエリは必ず領域を指定する遅延評価（`Query::lazy`）経由で実行する。
        Err(LogicError::Unsupported(
            "full scan of a database-backed table; use a bounded (lazy) query instead",
        ))
    }
}
