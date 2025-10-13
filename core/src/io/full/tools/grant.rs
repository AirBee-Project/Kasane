use crate::{
    json::input::{AllOrChoose, CommandDatabase, DatabaseCommand},
    user_error::UserError,
};

impl DatabaseCommand {
    pub fn as_bytes(&self) -> Vec<u8> {
        let mut result = Vec::new();
        match self {
            crate::json::input::AllOrChoose::Choose(v) => {
                for cmd in v {
                    match cmd {
                        CommandDatabase::CreateSpace => result.push(1),
                        CommandDatabase::DropSpace => result.push(2),
                        CommandDatabase::ShowSpaces => result.push(3),
                        CommandDatabase::Version => result.push(4),
                    }
                }
            }
            crate::json::input::AllOrChoose::All => result.push(0),
        }
        result
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, UserError> {
        if bytes.len() == 1 && bytes[0] == 0 {
            return Ok(AllOrChoose::All);
        }

        let mut cmds = Vec::new();
        for b in bytes {
            let cmd = match b {
                1 => CommandDatabase::CreateSpace,
                2 => CommandDatabase::DropSpace,
                3 => CommandDatabase::ShowSpaces,
                4 => CommandDatabase::Version,
                other => {
                    return Err(UserError::UnKnown {
                        message: format!("Invalid KeyMode byte: {}", other),
                        location: location!(),
                    });
                }
            };
            cmds.push(cmd);
        }

        Ok(AllOrChoose::Choose(cmds))
    }
}
