use std::cmp::Ordering;
use std::collections::HashSet;

use bincode::{Decode, Encode, config, decode_from_slice};
use redb::{Key, TypeName, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct DimensionValue {
    pub index: HashSet<u64>,
    pub count: usize,
}

impl Value for DimensionValue {
    type SelfType<'a> = DimensionValue;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        decode_from_slice(data, config::standard())
            .expect("Failed to decode DimensionValue")
            .0
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        bincode::encode_to_vec(value, config::standard()).expect("Failed to encode DimensionValue")
    }

    fn type_name() -> TypeName {
        TypeName::new("DimensionValue")
    }
}
