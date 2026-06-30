//! 値→空間の二次インデックス（値フィルタ用）のキー構成・エンコード。
//!
//! `value_index` の各エントリ（値なし）はキー
//! `table_id(16) ‖ enc(value)(可変) ‖ flexid.spatial_encode(14)` を持つ。
//!
//! - `flexid` は末尾固定 14 バイトなので、値長を知らずとも取り出せる。
//! - `enc(value)` は**順序保存エンコード**（バイト辞書順＝値の自然順）。これにより
//!   等価はプレフィックススキャン、範囲はレンジスキャンで引ける。

use kasane_logic::FlexId;

use crate::error::AppError;
use crate::models::{database::table::TableDataType, id::TableId};

/// [`FlexId`] の `spatial_encode` のバイト長。
const FLEX_ID_LEN: usize = 14;

/// 格納バイト列を「バイト辞書順＝値の自然順」になるよう変換する。
///
/// 値の格納形式（`interpret_value` 準拠）：
/// - `Int`   : i32 ビッグエンディアン → 符号ビット反転（負が先）
/// - `Float` : f32 BE IEEE754 → 負は全ビット反転、正は符号ビット立て
/// - `Text`  : UTF-8（辞書順そのまま）
/// - `Boolean`: 1 バイト 0/1（そのまま）
pub fn order_preserving(data_type: TableDataType, value: &[u8]) -> Vec<u8> {
    let mut key = value.to_vec();
    match data_type {
        TableDataType::TinyInt
        | TableDataType::SmallInt
        | TableDataType::Int
        | TableDataType::BigInt => {
            if let Some(b0) = key.first_mut() {
                *b0 ^= 0x80;
            }
        }
        TableDataType::Float | TableDataType::Double => {
            if let Some(&b0) = key.first() {
                if b0 & 0x80 != 0 {
                    for b in &mut key {
                        *b ^= 0xFF;
                    }
                } else {
                    key[0] ^= 0x80;
                }
            }
        }
        TableDataType::Text
        | TableDataType::Boolean
        | TableDataType::Enum
        | TableDataType::Presence => {}
    }
    key
}

/// インデックスキー `table_id ‖ vkey ‖ flexid` を組み立てる。
pub fn make_key(table_id: TableId, vkey: &[u8], flexid: &FlexId) -> Vec<u8> {
    let encoded = flexid.spatial_encode();
    let mut key = Vec::with_capacity(16 + vkey.len() + FLEX_ID_LEN);
    key.extend_from_slice(&table_id.into_bytes());
    key.extend_from_slice(vkey);
    key.extend_from_slice(&encoded);
    key
}

/// `table_id ‖ vkey` のプレフィックス（等価スキャン用）。
pub fn make_prefix(table_id: TableId, vkey: &[u8]) -> Vec<u8> {
    let mut prefix = Vec::with_capacity(16 + vkey.len());
    prefix.extend_from_slice(&table_id.into_bytes());
    prefix.extend_from_slice(vkey);
    prefix
}

/// インデックスキー末尾 14 バイトから [`FlexId`] を復元する。
pub fn flexid_from_key(key: &[u8]) -> Result<FlexId, AppError> {
    if key.len() < 16 + FLEX_ID_LEN {
        return Err(AppError::InternalError(
            "value_index key too short".to_string(),
        ));
    }
    let mut bytes = [0u8; FLEX_ID_LEN];
    bytes.copy_from_slice(&key[key.len() - FLEX_ID_LEN..]);
    FlexId::spatial_decode(&bytes)
        .map_err(|e| AppError::InternalError(format!("flex_id decode: {e}")))
}
