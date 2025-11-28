use bincode::{Decode, Encode};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::interface::input::KeyType;

impl KeyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyType::Int(_) => "Int",
            KeyType::Boolean(_) => "Boolean",
            KeyType::Text(_) => "Text",
            KeyType::Float(_) => "Float",
        }
    }

    pub fn as_kind(&self) -> KeyTypeKind {
        match self {
            KeyType::Text(_) => KeyTypeKind::Text,
            KeyType::Float(_) => KeyTypeKind::Float,
            KeyType::Int(_) => KeyTypeKind::Int,
            KeyType::Boolean(_) => KeyTypeKind::Boolean,
        }
    }
}

#[derive(Debug, Clone, Encode, Decode)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum KeyTypeKind {
    Text = 0,
    Float,
    Int,
    Boolean = 255,
}

impl KeyTypeKind {
    pub fn start() -> KeyTypeKind {
        KeyTypeKind::Text
    }

    pub fn end() -> KeyTypeKind {
        KeyTypeKind::Boolean
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyTypeKind::Int => "Int",
            KeyTypeKind::Boolean => "Boolean",
            KeyTypeKind::Text => "Text",
            KeyTypeKind::Float => "Float",
        }
    }
}
