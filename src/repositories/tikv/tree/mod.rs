//! FlexTree のデータ操作。ツリーの形と分割・統合の規則は LMDB 実装と同一で、
//! 違うのはノードの読み書きだけ。
//!
//! **上の段は下の段だけを呼ぶ。** 逆向きの呼び出しは無い。
//!
//! ```text
//!   read.rs      write.rs     ← 外向きの入口（TikvRead / TikvWrite の固有メソッド）
//!      │            │           read.rs は葉の解決（CPU・rayon）も持つ
//!      │            ├─ leaf.rs ← リーフの書き換え（純粋計算・blocking へ出す）
//!      └──── routing.rs ──────┘ ← 木の降下（どのリーフが担当か）
//!                │
//!             node.rs         ← ノード 1 枚の読み書き
//!                │
//!            super::kv        ← 素の KV（トランザクション・batch_get）
//! ```
//!
//! # LMDB 実装との違い
//!
//! - ノードの取得がネットワーク越しになるので、降下では**同じ深さのノードをまとめて**
//!   取得する。1 ノードずつ引くと深さ × 往復のレイテンシがかかる。
//! - 受信バッファは信用できないので、rkyv の非検証アクセスへ渡す前に
//!   [`kv::ShardValue`] の完全性検証を通す。
//! - リーフの走査と集約は FlexId 数に比例する CPU 処理で、TiKV ではこれが非同期ワーカー上
//!   で回る（LMDB はクロージャ全体が blocking タスクなので問題にならない）。大きな検索が
//!   ワーカーを占有すると無関係なリクエストまで止まるため、ネットワークと CPU を分けて
//!   後者を blocking タスクへ出し、葉が多ければ rayon で分散する。
//!
//! 読み取り経路は受信バッファを
//! [`ArchivedSpatialIdMap`](kasane_logic::ArchivedSpatialIdMap) で直接走査する。
//! `SpatialIdMap::from_bytes` は `Arc` ベースの作業木を丸ごと組み直すので、リーフ 1 枚
//! （最大 [`MAX_FLEX_ID_PER_SHARD`] 件）につき数千回のノード確保と値バイト列の複製が走る。
//! 読むだけなら要らないので、`from_bytes` は書き込み経路に残してある。

mod leaf;
mod node;
mod read;
mod routing;
mod write;

// 子モジュールが共通して使うもの（private な `use` も子孫からは見える）。
use super::kv::{Reader, Readers, ShardValue};
use super::{TikvRead, TikvWrite, keys, kv};
use crate::error::AppError;
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::repositories::encoding::shard_entry::{
    MAX_FLEX_ID_PER_SHARD, MERGE_FLEX_ID_THRESHOLD, ShardEntry,
};
use crate::repositories::encoding::value_index;

use rustc_hash::FxHashMap;

type ValueMap = FxHashMap<Vec<u8>, Vec<kasane_logic::FlexId>>;

/// rayon へ出す基準（触れる**リーフ数**）。クエリ FlexId 数で判定しないのは LMDB と
/// 同じ理由で、広域検索はクエリ FlexId が数個でも数千の葉に及ぶため。
const LEAF_PARALLEL_THRESHOLD: usize = 32;
