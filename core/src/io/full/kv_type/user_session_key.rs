use crate::io::full::kv_type::uuid::UuidKey;
use redb::{Key, TypeName, Value};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserSessionKey {
    pub expires_at: u64,     // 8 bytes
    pub session_id: UuidKey, // 16 bytes
}

impl Value for UserSessionKey {
    type SelfType<'a> = UserSessionKey;
    type AsBytes<'a> = [u8; 24]; // 完全固定長

    fn fixed_width() -> Option<usize> {
        Some(24)
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        let mut buf = [0u8; 24];
        buf[..8].copy_from_slice(&value.expires_at.to_le_bytes());
        buf[8..].copy_from_slice(&value.session_id.0);
        buf
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        assert!(data.len() == 24);
        let mut expires_bytes = [0u8; 8];
        expires_bytes.copy_from_slice(&data[..8]);
        let expires_at = u64::from_le_bytes(expires_bytes);

        let mut session_id_bytes = [0u8; 16];
        session_id_bytes.copy_from_slice(&data[8..24]);

        UserSessionKey {
            expires_at,
            session_id: UuidKey(session_id_bytes),
        }
    }

    fn type_name() -> TypeName {
        TypeName::new("UserSessionKey")
    }
}

impl Key for UserSessionKey {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        let key1 = UserSessionKey::from_bytes(data1);
        let key2 = UserSessionKey::from_bytes(data2);

        let cmp = key1.expires_at.cmp(&key2.expires_at);
        if cmp != Ordering::Equal {
            return cmp;
        }
        key1.session_id.cmp(&key2.session_id)
    }
}
