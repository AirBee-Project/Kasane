use crate::json::input::KeyType;

impl KeyType {
    pub fn start() -> KeyType {
        KeyType::Text
    }

    pub fn end() -> KeyType {
        KeyType::Boolean
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            KeyType::Int => "Int",
            KeyType::Boolean => "Boolean",
            KeyType::Text => "Text",
            KeyType::Float => "Float",
        }
    }
}
