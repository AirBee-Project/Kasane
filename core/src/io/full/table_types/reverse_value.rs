use std::collections::BTreeSet;

use crate::io::full::table_types::uuid::UuidKey;
use kasane_logic::bit_vec::BitVec;

use bincode::{Decode, Encode, config, decode_from_slice};
use redb::{TypeName, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct ReverseValue {
    pub f: BitVec,
    pub x: BitVec,
    pub y: BitVec,

    pub value_ids: BTreeSet<UuidKey>,
}

impl Value for ReverseValue {
    type SelfType<'a> = ReverseValue;
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
