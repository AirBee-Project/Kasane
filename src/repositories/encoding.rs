//! バックエンドに依存しないバイト表現。
//!
//! シャードエントリの形式と値インデックスのキー組み立ては、どのストレージへ保存しても
//! 同じでよい（LMDB も TiKV もキーをバイト辞書順で並べる）。バックエンド実装が
//! 増えても表現が分岐しないよう、純粋な符号化だけをここへ集めている。

pub mod flat_keys;
pub mod shard_entry;
pub mod value_index;

/// 値インデックスと `tables_data` のキー先頭に置く `TableId` のバイト長。
pub const TABLE_ID_LEN: usize = 16;
