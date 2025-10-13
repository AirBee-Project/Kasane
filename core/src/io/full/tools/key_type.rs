use crate::json::input::KeyType;

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
