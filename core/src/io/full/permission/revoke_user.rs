use std::collections::HashSet;

use redb::{ReadableMultimapTable, ReadableTable};

use crate::{
    io::full::{Storage, PERMISSION_USER, USER_TABLE},
    interface::{input::UserCommand, output::Output},
    user_error::UserError,
};

impl Storage {
    pub fn revoke_user(
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
            } else {
                for cmd in user_command {
                    table_user_perm.remove(user_id, cmd)?;
                }
            }
        }
        write_txn.commit()?;
        return Ok(Output::Success);
    }
}
