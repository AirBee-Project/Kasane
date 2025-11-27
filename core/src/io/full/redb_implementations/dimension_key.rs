use std::cmp::Ordering;

use crate::io::full::redb_implementations::uuid::UuidKey;
use kasane_logic::bit_vec::BitVec;

use bincode::{Decode, Encode, config, decode_from_slice};
use redb::{Key, TypeName, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct DimensionKey {
    pub key_id: UuidKey,
    pub bit_vec: BitVec,
}

impl Value for DimensionKey {
    type SelfType<'a> = DimensionKey;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        decode_from_slice(data, config::standard())
            .expect("Failed to decode DimensionKey")
            .0
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        bincode::encode_to_vec(&value, config::standard()).expect("Failed to encode DimensionKey")
    }

    fn type_name() -> TypeName {
        TypeName::new("DimensionKey")
    }
}

impl Key for DimensionKey {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        // デコード
        let key1: DimensionKey = decode_from_slice(data1, config::standard())
            .expect("Failed to decode DimensionKey")
            .0;
        let key2: DimensionKey = decode_from_slice(data2, config::standard())
            .expect("Failed to decode DimensionKey")
            .0;

        // まず UUID を比較
        let cmp_uuid = key1.key_id.cmp(&key2.key_id);
        if cmp_uuid != Ordering::Equal {
            return cmp_uuid;
        }

        // UUID が同じなら BitVec を比較
        key1.bit_vec.cmp(&key2.bit_vec)
    }
}
