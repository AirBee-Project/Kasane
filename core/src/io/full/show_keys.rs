use redb::ReadableDatabase;

use crate::{
    io::full::{kv_type::key_table_key::KeyTableKey, Storage, KEY_TABLE, SPACE_TABLE},
    json::{
        input::{KeyMode, KeyType},
        output::{Output, Showkeys},
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
                key_name: "".to_string(),   // 最小文字列
                key_mode: KeyMode::start(), // ダミー
                key_type: KeyType::start(), // ダミー
            };

            let end_key = KeyTableKey {
                space_id,
                key_name: "\u{FFFF}".to_string(), // Unicode最大文字で終端
                key_mode: KeyMode::end(),
                key_type: KeyType::end(),
            };

            for item in table_key.range(start_key..=end_key)? {
                let (key, _value_bytes) = item?;
                result.push(key.value().key_name);
            }
        }
        return Ok(Output::Showkeys(Showkeys { key_names: result }));
    }
}
