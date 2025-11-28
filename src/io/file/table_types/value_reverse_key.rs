use std::cmp::Ordering;

use crate::io::full::table_types::{uuid::UuidKey, value_entry::ValueEntry};

use bincode::{Decode, Encode, config, decode_from_slice};
use redb::{Key, TypeName, Value};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct ValueReverseKey {
    pub key_id: UuidKey,
    pub value: ValueEntry,
}

impl Value for ValueReverseKey {
    type SelfType<'a> = ValueReverseKey;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        decode_from_slice(data, config::standard())
            .expect("Failed to decode ValueReverseKey")
            .0
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        bincode::encode_to_vec(value, config::standard()).expect("Failed to encode ValueReverseKey")
    }

    fn type_name() -> TypeName {
        TypeName::new("ValueReverseKey")
    }
}

impl Key for ValueReverseKey {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        let v1: ValueReverseKey = decode_from_slice(data1, config::standard())
            .expect("Failed to decode ValueReverseKey")
            .0;
        let v2: ValueReverseKey = decode_from_slice(data2, config::standard())
            .expect("Failed to decode ValueReverseKey")
            .0;

        // まず key_id で比較
        let cmp_key = v1.key_id.cmp(&v2.key_id);
        if cmp_key != Ordering::Equal {
            return cmp_key;
        }

        // key_id が同じ場合は value で比較
        let cmp_value = ValueEntry::compare(
            &bincode::encode_to_vec(&v1.value, config::standard()).unwrap(),
            &bincode::encode_to_vec(&v2.value, config::standard()).unwrap(),
        );

        cmp_value
    }
}
