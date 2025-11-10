use crate::json::input::{KeyMode, KeyType};
use bincode::Decode;
use bincode::Encode;
use redb::{TypeName, Value};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct KeyTableValue {
    key_name: String,
    key_mode: KeyMode,
    key_type: KeyType,
}

impl Value for KeyTableValue {
    type SelfType<'a> = KeyTableValue;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None // 可変長
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        // decode_from_slice は (T, bytes_read) を返す
        let (value, _): (KeyTableValue, usize) =
            bincode::decode_from_slice(data, bincode::config::standard())
                .expect("Failed to decode KeyTableValue");
        value
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        bincode::encode_to_vec(value, bincode::config::standard())
            .expect("Failed to encode KeyTableValue")
    }

    fn type_name() -> TypeName {
        TypeName::new("KeyTableValue")
    }
}
