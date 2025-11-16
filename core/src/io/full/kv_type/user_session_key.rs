use std::cmp::Ordering;

use bincode::{config, decode_from_slice, Decode, Encode};
use redb::{Key, TypeName, Value};

use crate::io::full::kv_type::uuid::UuidKey;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct UserSessionKey {
    pub expires_at: u64,
    pub session_id: UuidKey,
}

impl Value for UserSessionKey {
    type SelfType<'a> = UserSessionKey;

    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        Some(80)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        bincode::decode_from_slice(data, config::standard())
            .expect("Failed to decode UserSessionKey")
            .0
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        // Use bincode to serialize
        bincode::encode_to_vec(&value, config::standard()).expect("Failed to encode UserSessionKey")
    }

    fn type_name() -> redb::TypeName {
        TypeName::new("UserSessionKey")
    }
}

impl Key for UserSessionKey {
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        let key1: UserSessionKey = decode_from_slice(data1, config::standard())
            .expect("Failed to decode KeyTableKey")
            .0;
        let key2: UserSessionKey = decode_from_slice(data2, config::standard())
            .expect("Failed to decode KeyTableKey")
            .0;

        // space_id を比較
        let cmp_space = key1.expires_at.cmp(&key2.expires_at);
        if cmp_space != Ordering::Equal {
            return cmp_space;
        }

        // space_id が同じ場合は key_name を比較
        key1.session_id.cmp(&key2.session_id)
    }
}
