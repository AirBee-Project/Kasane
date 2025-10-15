use serde::{Deserialize, Serialize};

use crate::{json::input::KeyType, user_error::UserError};

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ValueEntry {
    TEXT(String),
    BOOLEAN(bool),
    INT(i32),
    FLOAT(f32),
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
            KeyType::Text => Some(ValueEntry::TEXT(String::from_utf8_lossy(data).to_string())),
            KeyType::Boolean => Some(ValueEntry::BOOLEAN(data.get(0)? != &0)),
            KeyType::Int => {
                if data.len() != 4 {
                    return None;
                }
                let mut arr = [0u8; 4];
                arr.copy_from_slice(data);
                Some(ValueEntry::INT(i32::from_le_bytes(arr)))
            }
            KeyType::Float => {
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
            (ValueEntry::INT(_), KeyType::Int) => true,
            (ValueEntry::BOOLEAN(_), KeyType::Boolean) => true,
            (ValueEntry::TEXT(_), KeyType::Text) => true,
            (ValueEntry::FLOAT(_), KeyType::Float) => true,
            _ => false,
        }
    }
}
