use std::collections::HashSet;

use redb::ReadableTable;

use crate::{
    io::full::{Storage, PERMISSION_SPACE, USER_TABLE},
    json::{input::SpaceCommand, output::Output},
    user_error::UserError,
};

impl Storage {
    pub fn grant_space(
        &self,
        user_name: &str,
        space_command: HashSet<SpaceCommand>,
    ) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table_space = write_txn.open_multimap_table(PERMISSION_SPACE)?;
            let table_user = write_txn.open_table(USER_TABLE)?;

            let user_id = match table_user.get(user_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::UserNotFound {
                        user_name: user_name.to_owned(),
                    });
                }
            };

            if space_command.contains(&SpaceCommand::ALL) {
                table_space.remove_all(user_id)?;
                for cmd in SpaceCommand::all() {
                    table_space.insert(user_id, cmd)?;
                }
            } else {
                for cmd in space_command {
                    table_space.insert(user_id, cmd)?;
                }
            }
        }
        write_txn.commit()?;
        return Ok(Output::Success);
    }
}
