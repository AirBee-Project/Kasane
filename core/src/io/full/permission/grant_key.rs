use std::collections::HashSet;

use redb::ReadableTable;

use crate::{
    io::full::{Storage, PERMISSION_KEY, USER_TABLE},
    json::{input::KeyCommand, output::Output},
    user_error::UserError,
};

impl Storage {
    pub fn grant_key(
        &self,
        user_name: &str,
        key_command: HashSet<KeyCommand>,
    ) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table_key = write_txn.open_multimap_table(PERMISSION_KEY)?;
            let table_user = write_txn.open_table(USER_TABLE)?;

            let user_id = match table_user.get(user_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::UserNotFound {
                        user_name: user_name.to_owned(),
                    });
                }
            };

            if key_command.contains(&KeyCommand::ALL) {
                table_key.remove_all(user_id)?;
                for cmd in KeyCommand::all() {
                    table_key.insert(user_id, cmd)?;
                }
            } else {
                for cmd in key_command {
                    table_key.insert(user_id, cmd)?;
                }
            }
        }
        write_txn.commit()?;
        return Ok(Output::Success);
    }
}
