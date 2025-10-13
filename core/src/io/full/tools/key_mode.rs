use crate::json::input::KeyMode;

impl KeyMode {
    pub fn as_bytes(&self) -> &[u8] {
        match &self {
            KeyMode::UniqueKey => &[0],
            KeyMode::MultiKey => &[1],
        }
    }
}
