use std::cmp::Ordering;

use crate::interface::input::DatabaseCommand;
use redb::{Key, Value};

impl Value for DatabaseCommand {
    type SelfType<'a> = DatabaseCommand;
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
            0 => DatabaseCommand::ALL,
            1 => DatabaseCommand::CreateSpace,
            2 => DatabaseCommand::DropSpace,
            3 => DatabaseCommand::ShowSpaces,
            4 => DatabaseCommand::Version,
            5 => DatabaseCommand::InfoSpace,
            other => panic!("Invalid DatabaseCommand value: {}", other),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        [value.clone() as u8]
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("DatabaseCommand")
    }
}

impl Key for DatabaseCommand {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        assert!(data1.len() == 1, "Invalid DatabaseCommand key length");
        assert!(data2.len() == 1, "Invalid DatabaseCommand key length");

        data1[0].cmp(&data2[0])
    }
}
