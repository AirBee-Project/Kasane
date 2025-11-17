use std::collections::HashSet;

use redb::ReadableTable;

use crate::{
    io::full::{
        redb_implementations::permission_space_key::PermissionSpaceKey, Storage, PERMISSION_SPACE,
        SPACE_TABLE, USER_TABLE,
    },
    interface::{input::SpaceCommand, output::Output},
    location,
    user_error::UserError,
};

impl Storage {
    pub fn grant_space(
        &self,
        user_name: &str,
        target_spaces: &[String],
        space_command: HashSet<SpaceCommand>,
    ) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table_space = write_txn.open_multimap_table(PERMISSION_SPACE)?;
            let table_user = write_txn.open_table(USER_TABLE)?;
            let table_spaces = write_txn.open_table(SPACE_TABLE)?;

            let user_id = match table_user.get(user_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::UserNotFound {
                        user_name: user_name.to_owned(),
                    });
                }
            };

            // Grant permissions for each target space
            for space_name in target_spaces {
                let space_id = match table_spaces.get(space_name.as_str())? {
                    Some(v) => v.value(),
                    None => {
                        return Err(UserError::SpaceNotFound {
                            space_name: space_name.clone(),
                            location: location!(),
                        });
                    }
                };

                let permission_key = PermissionSpaceKey { space_id, user_id };

                if space_command.contains(&SpaceCommand::ALL) {
                    table_space.remove_all(permission_key)?;
                    for cmd in SpaceCommand::all() {
                        table_space.insert(permission_key, cmd)?;
                    }
                } else {
                    for cmd in &space_command {
                        table_space.insert(permission_key, cmd.clone())?;
                    }
                }
            }
        }
        write_txn.commit()?;
        return Ok(Output::Success);
    }
}
