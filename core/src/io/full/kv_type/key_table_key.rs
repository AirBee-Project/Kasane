use std::cmp::Ordering;

use crate::{
    io::full::kv_type::{key_type::KeyTypeKind, uuid::UuidKey},
    json::input::{KeyMode, KeyType},
};
use bincode::{config, decode_from_slice, Decode, Encode};
use redb::{Key, TypeName, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct KeyTableKey {
    pub space_id: UuidKey,
    pub key_name: String,
    pub key_mode: KeyMode,
    pub key_type_kind: KeyTypeKind,
}

impl Value for KeyTableKey {
    type SelfType<'a> = KeyTableKey;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        // Use bincode to deserialize
        bincode::decode_from_slice(data, config::standard())
            .expect("Failed to decode KeyTableKey")
            .0
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        // Use bincode to serialize
        bincode::encode_to_vec(&value, config::standard()).expect("Failed to encode KeyTableKey")
    }

    fn type_name() -> TypeName {
        TypeName::new("KeyTableKey")
    }
}

impl Key for KeyTableKey {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        // デコード
        let key1: KeyTableKey = decode_from_slice(data1, config::standard())
            .expect("Failed to decode KeyTableKey")
            .0;
        let key2: KeyTableKey = decode_from_slice(data2, config::standard())
            .expect("Failed to decode KeyTableKey")
            .0;

        // space_id を比較
        let cmp_space = key1.space_id.cmp(&key2.space_id);
        if cmp_space != Ordering::Equal {
            return cmp_space;
        }

        // space_id が同じ場合は key_name を比較
        key1.key_name.cmp(&key2.key_name)
    }
}
