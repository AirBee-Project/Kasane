use std::cmp::Ordering;

use crate::json::input::UserCommand;
use redb::{Key, Value};

impl Value for UserCommand {
    type SelfType<'a> = UserCommand;
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
            0 => UserCommand::ALL,
            1 => UserCommand::CreateUser,
            2 => UserCommand::DropUser,
            3 => UserCommand::InfoUser,
            4 => UserCommand::ShowUsers,
            5 => UserCommand::GrantDatabase,
            6 => UserCommand::GrantSpace,
            7 => UserCommand::GrantKey,
            8 => UserCommand::GrauntUser,
            9 => UserCommand::RevokeDatabase,
            10 => UserCommand::RevokeSpace,
            11 => UserCommand::RevokeKey,
            12 => UserCommand::RevokeUser,
            other => panic!("Invalid UserCommand value: {}", other),
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        [value.clone() as u8]
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("UserCommand")
    }
}

impl Key for UserCommand {
    fn compare(data1: &[u8], data2: &[u8]) -> Ordering {
        assert!(data1.len() == 1, "Invalid UserCommand key length");
        assert!(data2.len() == 1, "Invalid UserCommand key length");

        data1[0].cmp(&data2[0])
    }
}
