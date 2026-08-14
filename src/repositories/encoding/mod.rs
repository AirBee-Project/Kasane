//! バックエンドに依存しない純粋な符号化。固有のキーレイアウトは各実装の `keys` にある。

pub mod acl;
pub mod shard_entry;
pub mod value_index;

use crate::error::AppError;
use crate::models::id::DatabaseId;

/// キーへ埋め込む識別子のバイト長。
///
/// 固定長であることが「識別子 ‖ 可変長の続き」を曖昧さなく分解できる根拠になっている。
pub const UUID_LEN: usize = 16;

/// プレフィックスで始まる全キーを覆う範囲の終端（排他）。
/// 全バイトが 0xFF なら上限が存在しないので `None`。
pub fn prefix_end(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut end = prefix.to_vec();
    while let Some(last) = end.last_mut() {
        if *last < 0xFF {
            *last += 1;
            return Some(end);
        }
        end.pop();
    }
    None
}

/// `db_id(16) ‖ name` の並び。所属データベース付きの名前を 1 つの値に収める。
///
/// テーブル ID の逆引きが「名前」だけでなく「どのデータベースの配下か」も持つのに使う。
/// LMDB 側は同じ並びを heed のコーデック（`lmdb::keys::DbIdAndName`）で表す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedName {
    pub db_id: DatabaseId,
    pub name: String,
}

impl OwnedName {
    pub fn new(db_id: DatabaseId, name: &str) -> Self {
        Self {
            db_id,
            name: name.to_string(),
        }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(UUID_LEN + self.name.len());
        out.extend_from_slice(&self.db_id.into_bytes());
        out.extend_from_slice(self.name.as_bytes());
        out
    }

    pub fn into_parts(self) -> (DatabaseId, String) {
        (self.db_id, self.name)
    }
}

impl TryFrom<&[u8]> for OwnedName {
    type Error = AppError;

    fn try_from(bytes: &[u8]) -> Result<Self, AppError> {
        let id: [u8; UUID_LEN] = bytes
            .get(..UUID_LEN)
            .and_then(|s| s.try_into().ok())
            .ok_or_else(|| {
                AppError::InternalError("entry is too short to carry a database id".to_string())
            })?;
        let name = std::str::from_utf8(&bytes[UUID_LEN..])
            .map_err(|e| AppError::InternalError(format!("name is not valid utf-8: {e}")))?;
        Ok(Self::new(DatabaseId::from(id), name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_end_rolls_over_trailing_max_bytes() {
        assert_eq!(prefix_end(&[0x01, 0x02]), Some(vec![0x01, 0x03]));
        assert_eq!(prefix_end(&[0x01, 0xFF]), Some(vec![0x02]));
        assert_eq!(prefix_end(&[0xFF, 0xFF]), None);
        assert_eq!(prefix_end(&[]), None);
    }

    #[test]
    fn owned_name_round_trips() {
        let entry = OwnedName::new(DatabaseId::from([3u8; 16]), "my_table");
        let bytes = entry.clone().into_bytes();
        assert_eq!(OwnedName::try_from(bytes.as_slice()).unwrap(), entry);
    }

    #[test]
    fn truncated_owned_name_is_rejected() {
        assert!(OwnedName::try_from([0u8; 4].as_slice()).is_err());
    }
}
