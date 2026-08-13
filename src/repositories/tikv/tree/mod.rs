//! FlexTree（シャードツリー）のデータ操作。
//!
//! ツリーの形と分割・統合の規則は LMDB 実装と同一で、違うのは「ノードをどう読み書きするか」
//! だけ。ノードのバイト表現は [`shard_entry`](crate::repositories::encoding::shard_entry) に
//! 共通化してあるので、両バックエンドで同じデータ形式になる。
//!
//! # ファイルの並びと依存の向き
//!
//! **上の段は下の段だけを呼ぶ。** 逆向きの呼び出しは無い。
//!
//! ```text
//!   read.rs      write.rs     ← 外向きの入口（TikvRead / TikvWrite の固有メソッド）
//!      │            │
//!      ├─ resolve.rs│         ← 葉の解決（CPU・rayon）
//!      │            ├─ leaf.rs ← リーフの書き換え（純粋計算・blocking へ出す）
//!      └──── routing.rs ──────┘ ← 木の降下（どのリーフが担当か）
//!                │
//!             node.rs         ← ノード 1 枚の読み書き
//!                │
//!            super::kv        ← 素の KV（トランザクション・batch_get）
//! ```
//!
//! 外から見えるのは `read.rs` / `write.rs` が `TikvRead` / `TikvWrite` に生やす固有メソッド
//! だけで、それ以外はこのモジュールの内側で閉じている。
//!
//! # LMDB 実装との違い
//!
//! - ノードの取得がネットワーク越しになるため、木の降下では**同じ深さのノードをまとめて**
//!   取得する（`batch_get`）。1 ノードずつ引くと深さ × 往復のレイテンシがかかる。
//! - 受信バッファは信用できないので、rkyv の非検証アクセスへ渡す前に
//!   [`kv::ShardValue`] の完全性検証を通す（`kv.rs` のフレームの節を参照）。
//! - 再帰は `Box::pin` で明示的に間接化する（async fn の再帰のため）。
//!
//! # 読み取りのゼロコピー
//!
//! 読み取り経路は受信バッファを [`ArchivedSpatialIdMap`](kasane_logic::ArchivedSpatialIdMap)
//! で**直接走査**する。LMDB 側が mmap 上でやっているのと同じことを、mmap の代わりに
//! 受信バッファに対して行うだけ。
//!
//! `SpatialIdMap::from_bytes` を通すと `Arc` ベースの作業木を丸ごと組み直すことになり、
//! リーフ 1 枚（最大 [`MAX_FLEX_ID_PER_SHARD`] 件）につき数千回のノード確保と、葉ごとの
//! 値バイト列の複製、保存していない導出値の畳み直しが走る。そのどれも読むだけなら要らない。
//! 復元が要るのは**書き換える**ときだけなので、`from_bytes` は書き込み経路に残してある。
//!
//! # CPU をどこで回すか
//!
//! リーフの走査と集約は FlexId 数に比例する CPU 処理で、TiKV バックエンドではこれが
//! 非同期ワーカー上で回る（LMDB はクロージャ全体が blocking タスク上なので問題にならない）。
//! 大きな検索がワーカーを占有すると無関係なリクエストまで止まるため、ルーティング
//! （ネットワーク）と解決（CPU）を分け、後者を blocking タスクへ出したうえで葉が多ければ
//! rayon で分散する。

mod leaf;
mod node;
mod read;
mod resolve;
mod routing;
mod write;

// 子モジュールが共通して使うもの。ここへ集めておけば、各ファイルの先頭は
// そのファイル固有の依存だけになる（private な `use` も子孫からは見える）。
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

/// 値ごとに FlexId をまとめた中間表現。
type ValueMap = FxHashMap<Vec<u8>, Vec<kasane_logic::FlexId>>;

/// 葉の解決を rayon へ出す基準（触れる**リーフ数**）。
///
/// 基準を「クエリ FlexId 数」ではなく「実際に触れる葉の数」に置くのは LMDB 側と同じ理由で、
/// 広域検索はクエリ FlexId が数個でも数千の葉に及ぶため。
const LEAF_PARALLEL_THRESHOLD: usize = 32;
