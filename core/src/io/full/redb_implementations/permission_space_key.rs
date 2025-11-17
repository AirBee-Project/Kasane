use std::cmp::Ordering;

use crate::io::full::redb_implementations::uuid::UuidKey;
use redb::{Key, TypeName, Value};

/// Key for Space-level permissions: SpaceID + UserID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PermissionSpaceKey {
    pub space_id: UuidKey, // 16 bytes
    pub user_id: UuidKey,  // 16 bytes
}

impl Value for PermissionSpaceKey {
    type SelfType<'a> = PermissionSpaceKey;
    type AsBytes<'a> = [u8; 32]; // 16 + 16 = 32 bytes (fixed)

    fn fixed_width() -> Option<usize> {
        Some(32)
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        let mut buf = [0u8; 32];
        buf[..16].copy_from_slice(&value.space_id.0);
        buf[16..].copy_from_slice(&value.user_id.0);
        buf
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        assert!(data.len() == 32);
        let mut space_id_bytes = [0u8; 16];
        space_id_bytes.copy_from_slice(&data[..16]);

        let mut user_id_bytes = [0u8; 16];
        user_id_bytes.copy_from_slice(&data[16..32]);

        PermissionSpaceKey {
            space_id: UuidKey(space_id_bytes),
            user_id: UuidKey(user_id_bytes),
        }
    }

    fn type_name() -> TypeName {
        TypeName::new("PermissionSpaceKey")
    }
}

impl Key for PermissionSpaceKey {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        let key1 = PermissionSpaceKey::from_bytes(data1);
        let key2 = PermissionSpaceKey::from_bytes(data2);

        // First compare space_id
        let cmp = key1.space_id.cmp(&key2.space_id);
        if cmp != Ordering::Equal {
            return cmp;
        }
        // If space_id is equal, compare user_id
        key1.user_id.cmp(&key2.user_id)
    }
}
