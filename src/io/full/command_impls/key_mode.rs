use crate::interface::input::ValueMode;

impl ValueMode {
    pub fn start() -> ValueMode {
        ValueMode::UniqueValue
    }

    pub fn end() -> ValueMode {
        ValueMode::MultiValue
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ValueMode::UniqueValue => "UniqueValue",
            ValueMode::MultiValue => "MultiValue",
        }
    }
}
