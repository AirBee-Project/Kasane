use crate::{json::input::KeyMode, location, user_error::UserError};

impl KeyMode {
    pub fn as_bytes(&self) -> &[u8] {
        match &self {
            KeyMode::UniqueKey => &[0],
            KeyMode::MultiKey => &[1],
        }
    }
}

impl KeyMode {
    pub fn from_byte(b: u8) -> Result<Self, UserError> {
        match b {
            0 => Ok(KeyMode::UniqueKey),
            1 => Ok(KeyMode::MultiKey),
            other => Err(UserError::UnKnown {
                message: format!("Invalid KeyMode byte: {}", other),
                location: location!(),
            }),
        }
    }
}

impl KeyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            KeyMode::UniqueKey => "UniqueKey",
            KeyMode::MultiKey => "MultiKey",
        }
    }
}
