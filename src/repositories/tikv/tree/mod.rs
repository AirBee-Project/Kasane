//! FlexTree のデータ操作。木の形も分割・統合の規則も LMDB 実装と同一。
//!
//! 依存は `read`/`write` → `leaf`/`routing` → `node` → `super::kv` の一方向。LMDB との違いは、
//! 降下が**同じ深さをまとめて**取得すること、受信バッファを [`kv::ShardValue`] で検証すること、
//! 葉の走査を blocking タスクへ出すこと。

mod leaf;
mod node;
mod read;
mod routing;
mod shard_reader;
mod write;

pub(in crate::repositories::tikv) use shard_reader::ShardReader;

// private な `use` も子孫からは見える。
use super::kv::{Reader, Readers, ShardValue};
use super::shard_cache::ShardCache;
use super::{TikvRead, TikvWrite, keys, kv};
use crate::error::{AppError, Stored};
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::repositories::encoding::shard_entry::{
    MAX_FLEX_ID_PER_SHARD, MAX_SHARD_BYTES, MERGE_FLEX_ID_THRESHOLD, ShardEntry, shard_needs_split,
};
use crate::repositories::encoding::value_index;

use kasane_logic::FlexId;
use rustc_hash::FxHashMap;
use std::sync::Mutex;

type ValueMap = FxHashMap<Vec<u8>, Vec<kasane_logic::FlexId>>;

/// rayon へ出す基準（触れる**リーフ数**）。広域検索はクエリ FlexId が数個でも数千の葉に及ぶ。
const LEAF_PARALLEL_THRESHOLD: usize = 32;

/// 1 クエリ限りのノードキャッシュ。**トレードオフなしの最適化。**
///
/// クエリは断面（`snapshot_ts`）を 1 度固定して使い回す（[`Storage::query_snapshot`]
/// 参照）ので、その間は同じ (table_id, region) が同じ値を返すことを TiKV の MVCC が
/// 保証する。無効化の要らない安全なキャッシュになるのはこの寿命に閉じているときだけで、
/// クエリを跨いで使い回すと通常の（bounded staleness な）キャッシュに戻ってしまう。
///
/// [`Storage::query_snapshot`]: crate::repositories::Storage::query_snapshot
pub(in crate::repositories::tikv) type NodeCache = Mutex<FxHashMap<FlexId, Option<ShardValue>>>;

pub(in crate::repositories::tikv) fn new_node_cache() -> NodeCache {
    Mutex::new(FxHashMap::default())
}
