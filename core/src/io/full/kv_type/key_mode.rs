use crate::json::input::KeyMode;

impl KeyMode {
    pub fn start() -> KeyMode {
        KeyMode::UniqueKey
    }

    pub fn end() -> KeyMode {
        KeyMode::MultiKey
    }
}
