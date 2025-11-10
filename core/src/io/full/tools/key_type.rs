use crate::json::input::KeyType;
use crate::location;
use crate::user_error::UserError;

impl KeyType {
    pub fn as_bytes(&self) -> &[u8] {
        match &self {
            KeyType::Int => &[0],
            KeyType::Boolean => &[1],
            KeyType::Text => &[2],
            KeyType::Float => &[3],
        }
    }
}

impl KeyType {
    pub fn from_byte(b: u8) -> Result<Self, UserError> {
        match b {
            0 => Ok(KeyType::Int),
            1 => Ok(KeyType::Boolean),
            2 => Ok(KeyType::Text),
            3 => Ok(KeyType::Float),
            other => Err(UserError::UnKnown {
                message: format!("Invalid KeyType byte: {}", other),
                location: location!(),
            }),
        }
    }
}

impl KeyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyType::Int => "Int",
            KeyType::Boolean => "Boolean",
            KeyType::Text => "Text",
            KeyType::Float => "Float",
        }
    }
}
