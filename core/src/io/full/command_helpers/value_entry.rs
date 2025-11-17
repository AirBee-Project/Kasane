use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{interface::input::KeyType, user_error::UserError};

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, TS)]
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
