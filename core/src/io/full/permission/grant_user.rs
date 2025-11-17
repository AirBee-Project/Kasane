use std::collections::HashSet;

use redb::ReadableTable;

use crate::{
    io::full::{Storage, PERMISSION_USER, USER_TABLE},
    json::{input::UserCommand, output::Output},
    user_error::UserError,
};

impl Storage {
    pub fn grant_user(
        &self,
        user_name: &str,
        user_command: HashSet<UserCommand>,
    ) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table_user_perm = write_txn.open_multimap_table(PERMISSION_USER)?;
            let table_user = write_txn.open_table(USER_TABLE)?;

            let user_id = match table_user.get(user_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::UserNotFound {
                        user_name: user_name.to_owned(),
                    });
                }
            };

            if user_command.contains(&UserCommand::ALL) {
                table_user_perm.remove_all(user_id)?;
                for cmd in UserCommand::all() {
                    table_user_perm.insert(user_id, cmd)?;
                }
            } else {
                for cmd in user_command {
                    table_user_perm.insert(user_id, cmd)?;
                }
            }
        }
        write_txn.commit()?;
        return Ok(Output::Success);
    }
}
