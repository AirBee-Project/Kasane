use std::cmp::Ordering;

use bincode::{Decode, Encode, config, decode_from_slice};
use redb::{Key, TypeName, Value};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

use crate::interface::input::KeyType;

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[cfg_attr(feature = "ts-rs", derive(TS))]
pub enum ValueEntry {
    TEXT(String),
    BOOLEAN(bool),
    INT(i32),
    FLOAT(f32),
}

fn discriminant_order(v: &ValueEntry) -> u8 {
    match v {
        ValueEntry::TEXT(_) => 0,
        ValueEntry::BOOLEAN(_) => 1,
        ValueEntry::INT(_) => 2,
        ValueEntry::FLOAT(_) => 3,
    }
}

impl ValueEntry {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            ValueEntry::TEXT(s) => s.as_bytes().to_vec(),
            ValueEntry::BOOLEAN(b) => vec![*b as u8],
            ValueEntry::INT(i) => i.to_le_bytes().to_vec(),
            ValueEntry::FLOAT(f) => f.to_le_bytes().to_vec(),
        }
    }

    pub fn from_bytes(keytype: KeyType, data: &[u8]) -> Option<Self> {
        match keytype {
            KeyType::Text(_) => Some(ValueEntry::TEXT(String::from_utf8_lossy(data).to_string())),
            KeyType::Boolean(_) => Some(ValueEntry::BOOLEAN(data.get(0)? != &0)),
            KeyType::Int(_) => {
                if data.len() != 4 {
                    return None;
                }
                let mut arr = [0u8; 4];
                arr.copy_from_slice(data);
                Some(ValueEntry::INT(i32::from_le_bytes(arr)))
            }
            KeyType::Float(_) => {
                if data.len() != 4 {
                    return None;
                }
                let mut arr = [0u8; 4];
                arr.copy_from_slice(data);
                Some(ValueEntry::FLOAT(f32::from_le_bytes(arr)))
            }
        }
    }
}

impl ValueEntry {
    pub fn matches_keytype(&self, key_type: &KeyType) -> bool {
        match (self, key_type) {
            (ValueEntry::INT(_), KeyType::Int(_)) => true,
            (ValueEntry::BOOLEAN(_), KeyType::Boolean(_)) => true,
            (ValueEntry::TEXT(_), KeyType::Text(_)) => true,
            (ValueEntry::FLOAT(_), KeyType::Float(_)) => true,
            _ => false,
        }
    }
}

impl Value for ValueEntry {
    type SelfType<'a> = ValueEntry;
    type AsBytes<'a> = Vec<u8>;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        decode_from_slice(data, config::standard())
            .expect("Failed to decode ValueEntry")
            .0
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        bincode::encode_to_vec(value, config::standard()).expect("Failed to encode ValueEntry")
    }

    fn type_name() -> TypeName {
        TypeName::new("ValueEntry")
    }
}

impl Key for ValueEntry {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        let v1: ValueEntry = decode_from_slice(data1, config::standard()).unwrap().0;
        let v2: ValueEntry = decode_from_slice(data2, config::standard()).unwrap().0;

        let order1 = discriminant_order(&v1);
        let order2 = discriminant_order(&v2);

        // variant が異なる場合は discriminant_order で比較
        let cmp_discriminant = order1.cmp(&order2);
        if cmp_discriminant != Ordering::Equal {
            return cmp_discriminant;
        }

        // variant が同じ場合は値で比較
        match (v1, v2) {
            (ValueEntry::TEXT(a), ValueEntry::TEXT(b)) => a.cmp(&b),
            (ValueEntry::BOOLEAN(a), ValueEntry::BOOLEAN(b)) => a.cmp(&b),
            (ValueEntry::INT(a), ValueEntry::INT(b)) => a.cmp(&b),
            (ValueEntry::FLOAT(a), ValueEntry::FLOAT(b)) => {
                a.partial_cmp(&b).unwrap_or(Ordering::Equal)
            }
            _ => Ordering::Equal, // 型違いはここに来ない
        }
    }
}
