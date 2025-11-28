use redb::ReadableDatabase;

use crate::{
    interface::{
        input::{KeyType, ValueMode},
        output::{Output, Showkeys},
    },
    io::full::{
        command_impls::key_type::KeyTypeKind,
        table_types::key_table_key::KeyTableKey,
        Storage, KEY_TABLE, SPACE_TABLE,
    },
    location,
    user_error::UserError,
};

impl Storage {
    /// space 内の全キー一覧を取得
    pub fn show_keys(&self, space_name: &str) -> Result<Output, UserError> {
        let mut result: Vec<String> = vec![];

        let read_txn = self.db.begin_read()?;
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

            let space_id = match table_space.get(space_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::SpaceNotFound {
                        space_name: space_name.to_string(),
                        location: location!(),
                    });
                }
            };

            let table_key = match read_txn.open_table(KEY_TABLE) {
                Ok(v) => v,
                Err(e) => match e {
                    redb::TableError::TableDoesNotExist(_) => {
                        return Ok(Output::Showkeys(Showkeys { key_names: result }));
                    }
                    e => return Err(e.into()),
                },
            };

            // 範囲スキャン用 start/end
            let start_key = KeyTableKey {
                space_id,
                key_name: "".to_string(), // 最小文字列
                value_mode: ValueMode::start(),
                key_type_kind: KeyTypeKind::start(),
            };

            let end_key = KeyTableKey {
                space_id,
                key_name: "\u{FFFF}".to_string(), // Unicode最大文字で終端
                value_mode: ValueMode::end(),
                key_type_kind: KeyTypeKind::end(),
            };

            for item in table_key.range(start_key..=end_key)? {
                let (key, _value_bytes) = item?;
                result.push(key.value().key_name);
            }
        }
        return Ok(Output::Showkeys(Showkeys { key_names: result }));
    }
}
