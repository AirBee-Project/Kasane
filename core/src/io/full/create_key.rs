use std::collections::HashSet;

use bincode::enc::write;
use redb::{ReadableDatabase, ReadableMultimapTable, ReadableTable};

use crate::{
    io::full::{
        kv_type::{key_table_key::KeyTableKey, uuid::UuidKey},
        SpaceKeyTableValue, Storage, KEY_TABLE, SPACE_TABLE,
    },
    json::{
        input::{KeyMode, KeyType},
        output::Output,
    },
    location,
    user_error::UserError,
};

impl Storage {
    pub fn create_key(
        &self,
        space_name: &str,
        key_name: &str,
        key_type: KeyType,
        key_mode: KeyMode,
    ) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;
        let read_txn = self.db.begin_read()?;

        {
            let table_space = read_txn.open_table(SPACE_TABLE)?;
            let mut table_key = write_txn.open_table(KEY_TABLE)?;

            //Spaceの存在の検証
            let space_id = match table_space.get(key_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::SpaceAlreadyExists {
                        space_name: space_name.to_string(),
                        location: location!(),
                    });
                }
            };

            //Keyの存在の検証
            if table_key
                .get(KeyTableKey {
                    space_id,
                    key_name: key_name.to_string(),
                    key_mode,
                    key_type,
                })?
                .is_some()
            {
                return Err(UserError::KeyAlreadyExists {
                    space_name: space_name.to_string(),
                    key_name: key_name.to_string(),
                    location: location!(),
                });
            };

            let key_id = UuidKey::new();

            //Keyの挿入
            table_key.insert(
                KeyTableKey {
                    space_id,
                    key_name: key_name.to_string(),
                    key_mode,
                    key_type,
                },
                key_id,
            );
        }
        write_txn.commit()?;
        Ok(Output::Success)
    }
}
