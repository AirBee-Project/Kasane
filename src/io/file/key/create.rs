use std::collections::HashSet;

use bincode::enc::write;
use redb::{ReadableDatabase, ReadableMultimapTable, ReadableTable};

use crate::{
    interface::{
        input::{KeyType, ValueMode},
        output::Output,
    },
    io::full::{
        command_impls::key_type::KeyTypeKind,
        table_types::{key_table_key::KeyTableKey, uuid::UuidKey},
        Storage, KEY_TABLE, SPACE_TABLE,
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
        value_mode: ValueMode,
    ) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;
        let read_txn = self.db.begin_read()?;

        let key_type_kind = key_type.as_kind();

        {
            let table_space = match read_txn.open_table(SPACE_TABLE) {
                Ok(v) => v,
                Err(e) => match e {
                    redb::TableError::TableDoesNotExist(_) => {
                        return Err(UserError::SpaceNotFound {
                            space_name: space_name.to_string(),
                            location: location!(),
                        });
                    }
                    e => return Err(e.into()),
                },
            };
            let mut table_key = write_txn.open_table(KEY_TABLE)?;

            //Spaceの存在の検証
            let space_id = match table_space.get(space_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::SpaceNotFound {
                        space_name: space_name.to_string(),
                        location: location!(),
                    });
                }
            };

            // 範囲スキャン用 start/end
            let start_key = KeyTableKey {
                space_id,
                key_name: key_name.to_string(),      // 最小文字列
                value_mode: ValueMode::start(),      // ダミー
                key_type_kind: KeyTypeKind::start(), // ダミー
            };

            let end_key = KeyTableKey {
                space_id,
                key_name: key_name.to_string(), // Unicode最大文字で終端
                value_mode: ValueMode::end(),
                key_type_kind: KeyTypeKind::end(),
            };

            //Keyの存在の検証
            let mut range_exists = false;
            let mut iter = table_key.range(start_key..=end_key)?;
            if iter.next().is_some() {
                range_exists = true;
            }

            if range_exists {
                return Err(UserError::KeyAlreadyExists {
                    space_name: space_name.to_string(),
                    key_name: key_name.to_string(),
                    location: location!(),
                });
            }

            let key_id = UuidKey::new();

            //Keyの挿入
            let _ = table_key.insert(
                KeyTableKey {
                    space_id,
                    key_name: key_name.to_string(),
                    value_mode,
                    key_type_kind,
                },
                key_id,
            );
        }
        write_txn.commit()?;
        Ok(Output::Success)
    }
}
