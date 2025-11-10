use bincode::{config, decode_from_slice, encode_to_vec, Decode, Encode};
use redb::{TypeName, Value};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::io::full::kv_type::uuid::UuidKey;

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SpaceKeyTableValue(pub HashSet<UuidKey>);

impl Value for SpaceKeyTableValue {
    type SelfType<'a> = SpaceKeyTableValue;
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
            .expect("Failed to decode SpaceKeyTableValue")
            .0
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        // Use bincode to serialize
        bincode::encode_to_vec(&value, config::standard())
            .expect("Failed to encode SpaceKeyTableValue")
    }

    fn type_name() -> TypeName {
        TypeName::new("SpaceKeyTableValue")
    }
}
