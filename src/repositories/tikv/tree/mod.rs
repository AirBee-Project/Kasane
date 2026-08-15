//! FlexTree のデータ操作。木の形も分割・統合の規則も LMDB 実装と同一。
//!
//! 依存は `read`/`write` → `leaf`/`routing` → `node` → `super::kv` の一方向。LMDB との違いは、
//! 降下が**同じ深さをまとめて**取得すること、受信バッファを [`kv::ShardValue`] で検証すること、
//! 葉の走査を blocking タスクへ出すこと。

mod leaf;
mod node;
mod read;
mod routing;
mod write;

// private な `use` も子孫からは見える。
use super::kv::{Reader, Readers, ShardValue};
use super::{TikvRead, TikvWrite, keys, kv};
use crate::error::{AppError, Stored};
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::repositories::encoding::shard_entry::{
    MAX_SHARD_BYTES, MERGE_FLEX_ID_THRESHOLD, ShardEntry, shard_needs_split,
};
use crate::repositories::encoding::value_index;

use rustc_hash::FxHashMap;

type ValueMap = FxHashMap<Vec<u8>, Vec<kasane_logic::FlexId>>;

/// rayon へ出す基準（触れる**リーフ数**）。広域検索はクエリ FlexId が数個でも数千の葉に及ぶ。
const LEAF_PARALLEL_THRESHOLD: usize = 32;
