use crate::io::full::table_types::uuid::UuidKey;

use std::cmp::Ordering;

use bincode::{Decode, Encode, config, decode_from_slice};
use redb::{Key, TypeName, Value};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ReverseKey {
    pub key_id: UuidKey,
    pub index: u64,
}

impl Value for ReverseKey {
    type SelfType<'a> = ReverseKey;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        decode_from_slice(data, config::standard())
            .expect("Failed to decode ReverseKey")
            .0
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        bincode::encode_to_vec(value, config::standard()).expect("Failed to encode ReverseKey")
    }

    fn type_name() -> TypeName {
        TypeName::new("ReverseKey")
    }
}

impl Key for ReverseKey {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        let k1: ReverseKey = decode_from_slice(data1, config::standard())
            .expect("Failed to decode ReverseKey")
            .0;
        let k2: ReverseKey = decode_from_slice(data2, config::standard())
            .expect("Failed to decode ReverseKey")
            .0;

        // まず UUID (key_id) を比較
        let cmp = k1.key_id.cmp(&k2.key_id);
        if cmp != Ordering::Equal {
            return cmp;
        }

        // UUID が同じなら index を比較
        k1.index.cmp(&k2.index)
    }
}
