use std::collections::HashSet;

use redb::{ReadableMultimapTable, ReadableTable};

use crate::{
    io::full::{
        redb_implementations::{
            key_table_key::KeyTableKey, key_type::KeyTypeKind, permission_key_key::PermissionKeyKey,
        },
        Storage, KEY_TABLE, PERMISSION_KEY, SPACE_TABLE, USER_TABLE,
    },
    json::{input::{KeyCommand, KeyMode}, output::Output},
    location,
    user_error::UserError,
};

impl Storage {
    pub fn revoke_key(
        &self,
        user_name: &str,
        target_space: &str,
        target_keys: &[String],
        key_command: HashSet<KeyCommand>,
    ) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table_key_perm = write_txn.open_multimap_table(PERMISSION_KEY)?;
            let table_user = write_txn.open_table(USER_TABLE)?;
            let table_spaces = write_txn.open_table(SPACE_TABLE)?;
            let table_keys = write_txn.open_table(KEY_TABLE)?;

            let user_id = match table_user.get(user_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::UserNotFound {
                        user_name: user_name.to_owned(),
                    });
                }
            };

            let space_id = match table_spaces.get(target_space)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::SpaceNotFound {
                        space_name: target_space.to_string(),
                        location: location!(),
                    });
                }
            };

            // Revoke permissions for each target key
            for key_name in target_keys {
                // Find the key_id by scanning the key table
                let start_key = KeyTableKey {
                    space_id,
                    key_name: key_name.to_string(),
                    key_mode: KeyMode::start(),
                    key_type_kind: KeyTypeKind::start(),
                };

                let end_key = KeyTableKey {
                    space_id,
                    key_name: key_name.to_string(),
                    key_mode: KeyMode::end(),
                    key_type_kind: KeyTypeKind::end(),
                };

                let key_id = match table_keys.range(start_key..=end_key)?.next() {
                    Some(entry) => {
                        let (_key, value) = entry?;
                        value.value()
                    }
                    None => {
                        return Err(UserError::KeyNotFound {
                            space_name: target_space.to_string(),
                            key_name: key_name.clone(),
                            location: location!(),
                        });
                    }
                };

                let permission_key = PermissionKeyKey {
                    space_id,
                    key_id,
                    user_id,
                };

                if key_command.contains(&KeyCommand::ALL) {
                    table_key_perm.remove_all(permission_key)?;
                } else {
                    for cmd in &key_command {
                        table_key_perm.remove(permission_key, cmd.clone())?;
                    }
                }
            }
        }
        write_txn.commit()?;
        return Ok(Output::Success);
    }
}
