use redb::{Key, Value};
use std::cmp::Ordering;

use crate::json::input::SpaceCommand;

impl Value for SpaceCommand {
    type SelfType<'a> = SpaceCommand;
    type AsBytes<'a> = [u8; 1]; // 固定長1バイト

    fn fixed_width() -> Option<usize> {
        Some(1)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        assert!(data.len() == 1);
        match data[0] {
            0 => SpaceCommand::ALL,
            1 => SpaceCommand::CreateKey,
            2 => SpaceCommand::DropKey,
            3 => SpaceCommand::ShowKeys,
            4 => SpaceCommand::InfoKey,
            other => panic!("Invalid SpaceCommand value: {}", other),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a> {
        [value.clone() as u8]
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("SpaceCommand")
    }
}

impl Key for SpaceCommand {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        assert!(data1.len() == 1, "Invalid SpaceCommand key length");
        assert!(data2.len() == 1, "Invalid SpaceCommand key length");

        data1[0].cmp(&data2[0])
    }
}
