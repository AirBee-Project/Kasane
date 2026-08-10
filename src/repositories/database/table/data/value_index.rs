use crate::error::AppError;
use crate::models::{database::table::TableDataType, id::TableId};
use kasane_logic::FlexId;

/// 格納バイト列を「バイト辞書順＝値の自然順」になるよう変換する。
///
/// 値の格納形式（`interpret_value` 準拠）：
/// - `Int`   : i64 ビッグエンディアン → 符号ビット反転（負が先）
/// - `Text`  : UTF-8（辞書順そのまま）
/// - `Boolean`: 1 バイト 0/1（そのまま）
pub fn order_preserving(data_type: TableDataType, value: &[u8]) -> Vec<u8> {
    let mut key = value.to_vec();
    match data_type {
        TableDataType::Int => {
            if let Some(b0) = key.first_mut() {
                *b0 ^= 0x80;
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
    let encoded = flexid.encode();
    let mut key = Vec::with_capacity(16 + vkey.len() + FlexId::ENCODED_LEN);
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

/// インデックスキー末尾 [`FlexId::ENCODED_LEN`] バイトから [`FlexId`] を復元する。
pub fn flexid_from_key(key: &[u8]) -> Result<FlexId, AppError> {
    if key.len() < 16 + FlexId::ENCODED_LEN {
        return Err(AppError::InternalError(
            "value_index key too short".to_string(),
        ));
    }
    let mut bytes = [0u8; FlexId::ENCODED_LEN];
    bytes.copy_from_slice(&key[key.len() - FlexId::ENCODED_LEN..]);
    FlexId::decode(&bytes).map_err(|e| AppError::InternalError(format!("flex_id decode: {e}")))
}
