use redb::{ReadableDatabase, ReadableTable};

use crate::{
    io::full::{Storage, PERMISSION_DATABASE, USER_TABLE},
    json::{
        input::DatabaseCommand,
        output::{InfoUser, Output},
    },
    user_error::UserError,
};

impl Storage {
    pub fn info_user(&self, user_name: &str) -> Result<Output, UserError> {
        let read_txn = self.db.begin_read()?;

        {
            let table_user = read_txn.open_table(USER_TABLE)?;
            let table_permission_database = read_txn.open_multimap_table(PERMISSION_DATABASE)?;

            // Get user_id
            let user_id = match table_user.get(user_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::UserNotFound {
                        user_name: user_name.to_string(),
                    });
                }
            };

            // Collect database commands
            let mut database_commands: Vec<DatabaseCommand> = vec![];
            if let Ok(iter) = table_permission_database.get(user_id) {
                for cmd in iter {
                    database_commands.push(cmd?.value());
                }
            }

            // Return InfoUser with database commands and empty vectors for space/key permissions
            // The current permission system doesn't store per-space or per-key permissions
            return Ok(Output::InfoUser(InfoUser {
                user_name: user_name.to_string(),
                database_command: database_commands,
                space_command: vec![],
                key_commnad: vec![],
            }));
        }
    }
}
