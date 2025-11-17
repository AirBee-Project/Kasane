use std::collections::HashMap;

use redb::{ReadableDatabase, ReadableMultimapTable, ReadableTable};

use crate::{
    io::full::{
        redb_implementations::{
            key_table_key::KeyTableKey, key_type::KeyTypeKind, permission_key_key::PermissionKeyKey,
            permission_space_key::PermissionSpaceKey, uuid::UuidKey,
        },
        Storage, KEY_TABLE, PERMISSION_DATABASE, PERMISSION_KEY, PERMISSION_SPACE, SPACE_TABLE,
        USER_TABLE,
    },
    json::{
        input::{DatabaseCommand, KeyCommand, KeyMode, SpaceCommand},
        output::{InfoUser, InfoUserKey, InfoUserSpace, Output},
    },
    user_error::UserError,
};

impl Storage {
    pub fn info_user(&self, user_name: &str) -> Result<Output, UserError> {
        let read_txn = self.db.begin_read()?;

        let user_id: UuidKey;
        let mut database_commands = vec![];
        let mut space_commands_map: HashMap<UuidKey, Vec<SpaceCommand>> = HashMap::new();
        let mut key_commands_map: HashMap<(UuidKey, UuidKey), Vec<KeyCommand>> = HashMap::new();

        {
            let table_user = read_txn.open_table(USER_TABLE)?;
            let table_permission_database = read_txn.open_multimap_table(PERMISSION_DATABASE)?;
            let table_permission_space = read_txn.open_multimap_table(PERMISSION_SPACE)?;
            let table_permission_key = read_txn.open_multimap_table(PERMISSION_KEY)?;

            // Get user_id from user_name
            user_id = match table_user.get(user_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::UserNotFound {
                        user_name: user_name.to_owned(),
                    });
                }
            };

            // Get database-level permissions
            for item in table_permission_database.get(user_id)? {
                let cmd = item?;
                database_commands.push(cmd.value());
            }

            // Get space-level permissions
            // We need to iterate through all permission_space entries for this user
            // The key structure is PermissionSpaceKey { space_id, user_id }
            // We need to scan all entries where user_id matches

            // Get all spaces first to iterate
            let table_space = read_txn.open_table(SPACE_TABLE)?;
            for space_entry in table_space.iter()? {
                let (space_name, space_id_guard) = space_entry?;
                let space_id = space_id_guard.value();

                let permission_key = PermissionSpaceKey { space_id, user_id };

                // Get all space commands for this space and user
                let mut commands = vec![];
                for cmd_entry in table_permission_space.get(permission_key)? {
                    let cmd = cmd_entry?;
                    commands.push(cmd.value());
                }

                if !commands.is_empty() {
                    space_commands_map.insert(space_id, commands);
                }
            }

            // Get key-level permissions
            // Similar to space-level, we need to iterate through keys
            let table_key = read_txn.open_table(KEY_TABLE)?;
            for key_entry in table_key.iter()? {
                let (key_table_key, key_id_guard) = key_entry?;
                let key_id = key_id_guard.value();
                let space_id = key_table_key.value().space_id;

                let permission_key = PermissionKeyKey {
                    space_id,
                    key_id,
                    user_id,
                };

                // Get all key commands for this key and user
                let mut commands = vec![];
                for cmd_entry in table_permission_key.get(permission_key)? {
                    let cmd = cmd_entry?;
                    commands.push(cmd.value());
                }

                if !commands.is_empty() {
                    key_commands_map.insert((space_id, key_id), commands);
                }
            }
        }

        // Now we need to build the output structure
        // We need to map space_id and key_id back to their names
        let mut space_command_list = vec![];
        let mut key_command_list = vec![];

        {
            let table_space = read_txn.open_table(SPACE_TABLE)?;
            let table_key = read_txn.open_table(KEY_TABLE)?;

            // Map space permissions back to space names
            for (space_id, commands) in space_commands_map {
                // Find the space name for this space_id
                let mut space_name_opt = None;
                for space_entry in table_space.iter()? {
                    let (name, id_guard) = space_entry?;
                    if id_guard.value() == space_id {
                        space_name_opt = Some(name.value().to_string());
                        break;
                    }
                }

                if let Some(space_name) = space_name_opt {
                    space_command_list.push(InfoUserSpace {
                        space_name,
                        space_commnad: commands,
                    });
                }
            }

            // Map key permissions back to space and key names
            for ((space_id, key_id), commands) in key_commands_map {
                // Find the space name for this space_id
                let mut space_name_opt = None;
                for space_entry in table_space.iter()? {
                    let (name, id_guard) = space_entry?;
                    if id_guard.value() == space_id {
                        space_name_opt = Some(name.value().to_string());
                        break;
                    }
                }

                // Find the key name for this key_id in this space
                let mut key_name_opt = None;
                if space_name_opt.is_some() {
                    for key_entry in table_key.iter()? {
                        let (key_table_key, id_guard) = key_entry?;
                        if id_guard.value() == key_id
                            && key_table_key.value().space_id == space_id
                        {
                            key_name_opt = Some(key_table_key.value().key_name.clone());
                            break;
                        }
                    }
                }

                if let (Some(space_name), Some(key_name)) = (space_name_opt, key_name_opt) {
                    key_command_list.push(InfoUserKey {
                        space_name,
                        key_name,
                        key_commnad: commands,
                    });
                }
            }
        }

        Ok(Output::InfoUser(InfoUser {
            user_name: user_name.to_string(),
            database_command: database_commands,
            space_command: space_command_list,
            key_commnad: key_command_list,
        }))
    }
}
