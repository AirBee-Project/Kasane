use std::collections::HashSet;

use redb::ReadableTable;

use crate::{
    io::full::{Storage, PERMISSION_DATABASE, USER_TABLE},
    interface::{input::DatabaseCommand, output::Output},
    user_error::UserError,
};

impl Storage {
    pub fn grant_database(
        &self,
        user_name: &str,
        database_command: HashSet<DatabaseCommand>,
    ) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table_database = write_txn.open_multimap_table(PERMISSION_DATABASE)?;
            let table_user = write_txn.open_table(USER_TABLE)?;

            let user_id = match table_user.get(user_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::UserNotFound {
                        user_name: user_name.to_owned(),
                    });
                }
            };

            if database_command.contains(&DatabaseCommand::ALL) {
                table_database.remove_all(user_id)?;
                for cmd in DatabaseCommand::all() {
                    table_database.insert(user_id, cmd)?;
                }
            } else {
                for cmd in database_command {
                    table_database.insert(user_id, cmd)?;
                }
            }
        }
        write_txn.commit()?;
        return Ok(Output::Success);
    }
}
