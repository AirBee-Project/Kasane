use crate::json::input::KeyType;
use crate::user_error::UserError;

impl KeyType {
    pub fn as_bytes(&self) -> &[u8] {
        match &self {
            KeyType::INT => &[0],
            KeyType::BOOLEAN => &[1],
            KeyType::TEXT => &[2],
            KeyType::FLOAT => &[3],
        }
    }
}

impl KeyType {
    pub fn from_byte(b: u8) -> Result<Self, UserError> {
        match b {
            0 => Ok(KeyType::INT),
            1 => Ok(KeyType::BOOLEAN),
            2 => Ok(KeyType::TEXT),
            3 => Ok(KeyType::FLOAT),
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
            KeyType::INT => "INT",
            KeyType::BOOLEAN => "BOOLEAN",
            KeyType::TEXT => "TEXT",
            KeyType::FLOAT => "FLOAT",
        }
    }
}
