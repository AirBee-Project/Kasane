use redb::ReadableDatabase;

use crate::{
    io::full::{
        kv_type::{key_table_key::KeyTableKey, key_type::KeyTypeKind},
        Storage, KEY_TABLE, SPACE_TABLE,
    },
    json::{
        input::{KeyMode, KeyType},
        output::{InfoKey, InfoSpace, Output},
    },
    location,
    user_error::UserError,
};

impl Storage {
    /// space 内の全キー情報を取得
    pub fn info_space(&self, space_name: &str) -> Result<Output, UserError> {
        let mut result: Vec<InfoKey> = vec![];

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
                key_name: "".to_string(), // 最小文字列
                key_mode: KeyMode::start(),
                key_type_kind: KeyTypeKind::start(),
            };

            let end_key = KeyTableKey {
                space_id,
                key_name: "\u{FFFF}".to_string(), // Unicode最大文字で終端
                key_mode: KeyMode::end(),
                key_type_kind: KeyTypeKind::end(),
            };

            for item in table_key.range(start_key..=end_key)? {
                let (key, _value_bytes) = item?;
                result.push(InfoKey {
                    key_name: key.value().key_name,
                    key_type: key.value().key_type_kind.as_str().to_owned(),
                    key_mode: key.value().key_type_kind.as_str().to_owned(),
                });
            }
        }

        return Ok(Output::InfoSpace(InfoSpace {
            space_name: space_name.to_string(),
            keys: result,
        }));
    }
}
