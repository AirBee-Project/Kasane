use redb::ReadableDatabase;

use crate::{
    io::full::{kv_type::key_table_key::KeyTableKey, Storage, KEY_TABLE, SPACE_TABLE},
    json::{
        input::{KeyMode, KeyType},
        output::{InfoKey, Output},
    },
    location,
    user_error::UserError,
};

impl Storage {
    /// space 内のキー情報を取得
    pub fn info_key(&self, space_name: &str, key_name: &str) -> Result<Output, UserError> {
        let read_txn = self.db.begin_read()?;
        {
            let table_space = read_txn.open_table(SPACE_TABLE)?;
            let table_key = read_txn.open_table(KEY_TABLE)?;

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
                key_name: key_name.to_string(), // 最小文字列
                key_mode: KeyMode::start(),     // ダミー
                key_type: KeyType::start(),     // ダミー
            };

            let end_key = KeyTableKey {
                space_id,
                key_name: key_name.to_string(), // Unicode最大文字で終端
                key_mode: KeyMode::end(),
                key_type: KeyType::end(),
            };

            match table_key.range(start_key..=end_key)?.next() {
                Some(v) => {
                    let (key, _value_bytes) = v?;
                    return Ok(Output::InfoKey(InfoKey {
                        key_name: key.value().key_name,
                        key_type: key.value().key_type.as_str().to_string(),
                        key_mode: key.value().key_mode.as_str().to_string(),
                    }));
                }
                None => {
                    return Err(UserError::KeyNotFound {
                        space_name: space_name.to_string(),
                        key_name: key_name.to_string(),
                        location: location!(),
                    });
                }
            }
        }
    }
}
