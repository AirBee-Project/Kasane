use crate::interface::input::KeyMode;

impl KeyMode {
    pub fn start() -> KeyMode {
        KeyMode::UniqueKey
    }

    pub fn end() -> KeyMode {
        KeyMode::MultiKey
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            KeyMode::UniqueKey => "UniqueKey",
            KeyMode::MultiKey => "MultiKey",
        }
    }
}
