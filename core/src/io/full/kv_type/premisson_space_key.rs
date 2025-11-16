use redb::{Key, Value};
use std::cmp::Ordering;

use crate::json::input::KeyCommand;

impl Value for KeyCommand {
    type SelfType<'a> = KeyCommand;
    type AsBytes<'a> = [u8; 1]; // 固定長1バイト

    fn fixed_width() -> Option<usize> {
        Some(1)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        match data[0] {
            0 => KeyCommand::ALL,
            1 => KeyCommand::InsertValue,
            2 => KeyCommand::PatchValue,
            3 => KeyCommand::UpdateValue,
            4 => KeyCommand::DropKey,
            5 => KeyCommand::SelectValue,
            6 => KeyCommand::InfoKey,
            7 => KeyCommand::ShowValues,
            8 => KeyCommand::FilterValue,
            other => panic!("Invalid KeyCommand value: {}", other),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        [value.clone() as u8]
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("KeyCommand")
    }
}

impl Key for KeyCommand {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        assert!(data1.len() == 1, "Invalid KeyCommand key length");
        assert!(data2.len() == 1, "Invalid KeyCommand key length");
        data1[0].cmp(&data2[0])
    }
}
