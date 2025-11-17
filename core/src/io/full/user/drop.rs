use crate::{
    io::full::{
        redb_implementations::{
            permission_key_key::PermissionKeyKey, permission_space_key::PermissionSpaceKey,
            uuid::UuidKey,
        },
        Storage, KEY_TABLE, PERMISSION_DATABASE, PERMISSION_KEY, PERMISSION_SPACE,
        PERMISSION_USER, SPACE_TABLE, USER_PASSWORD, USER_TABLE,
    },
    json::output::Output,
    user_error::UserError,
};
use redb::ReadableTable;

impl Storage {
    pub fn drop_user(&self, user_name: &str) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table_user = write_txn.open_table(USER_TABLE)?;
            let mut table_password = write_txn.open_table(USER_PASSWORD)?;
            let mut table_permission_database =
                write_txn.open_multimap_table(PERMISSION_DATABASE)?;
            let mut table_permission_space = write_txn.open_multimap_table(PERMISSION_SPACE)?;
            let mut table_permission_key = write_txn.open_multimap_table(PERMISSION_KEY)?;
            let mut table_permission_user = write_txn.open_multimap_table(PERMISSION_USER)?;

            let userid = match table_user.remove(user_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::UserNotFound {
                        user_name: user_name.to_owned(),
                    });
                }
            };

            table_password.remove(userid)?;
            table_permission_database.remove_all(userid)?;

            // For PERMISSION_SPACE and PERMISSION_KEY, we need to scan and remove entries
            // where the user_id matches. We use range scanning for efficiency.

            // Remove PERMISSION_SPACE entries for this user
            // We iterate through all spaces and for each space, remove permissions for this user
            if let Ok(table_space) = write_txn.open_table(SPACE_TABLE) {
                let mut space_ids: Vec<UuidKey> = Vec::new();
                for space_entry in table_space.iter()? {
                    let (_, space_id) = space_entry?;
                    space_ids.push(space_id.value());
                }

                // For each space, remove the user's permissions
                for space_id in space_ids {
                    let permission_key = PermissionSpaceKey { space_id, user_id: userid };
                    table_permission_space.remove_all(permission_key)?;
                }
            }

            // Remove PERMISSION_KEY entries for this user
            // We iterate through all keys and for each key, remove permissions for this user
            if let Ok(table_key) = write_txn.open_table(KEY_TABLE) {
                let mut key_pairs: Vec<(UuidKey, UuidKey)> = Vec::new();
                for key_entry in table_key.iter()? {
                    let (key_table_key, key_id) = key_entry?;
                    let space_id = key_table_key.value().space_id;
                    key_pairs.push((space_id, key_id.value()));
                }

                // For each key, remove the user's permissions
                for (space_id, key_id) in key_pairs {
                    let permission_key = PermissionKeyKey {
                        space_id,
                        key_id,
                        user_id: userid,
                    };
                    table_permission_key.remove_all(permission_key)?;
                }
            }

            table_permission_user.remove_all(userid)?;
        }

        write_txn.commit()?;
        Ok(Output::Success)
    }
}
