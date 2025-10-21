use sled::transaction::{TransactionError, Transactional, abort};

use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::{
        input::{KeyMode, KeyType},
        output::{InfoKey, Output},
    },
    user_error::UserError,
};

impl Storage {
    /// space 内のキー情報を取得
    pub fn info_key(&self, space_name: &str, key_name: &str) -> Result<Output, UserError> {
        let location = location!();

        // sled transaction を使って atomic read
        let result: Result<Output, TransactionError<UserError>> = (&self.db).transaction(|tx| {
            // 1️⃣ space_id を取得
            let mut space_bytes = vec![Data::Space as u8];
            space_bytes.extend_from_slice(space_name.as_bytes());

            let space_id = match tx.get(&space_bytes)? {
                Some(id) => id,
                None => {
                    return abort(UserError::SpaceNotFound {
                        space_name: space_name.to_string(),
                        location: location.clone(),
                    });
                }
            };

            // 2️⃣ key_bytes 作成
            let mut key_bytes = vec![Data::Key as u8];
            key_bytes.extend_from_slice(&space_id);
            key_bytes.extend_from_slice(key_name.as_bytes());

            // 3️⃣ key の取得
            let value_bytes = match tx.get(&key_bytes)? {
                Some(v) => v,
                None => {
                    return abort(UserError::KeyNotFound {
                        space_name: space_name.to_string(),
                        key_name: key_name.to_string(),
                        location: location.clone(),
                    });
                }
            };

            // 4️⃣ バリデーション
            if value_bytes.len() < 10 {
                return abort(UserError::UnKnown {
                    message: "invalid key value length".into(),
                    location: location.clone(),
                });
            }

            // 5️⃣ type/mode を末尾 2 バイトから復元
            let key_type = match KeyType::from_byte(value_bytes[value_bytes.len() - 2]) {
                Ok(t) => t,
                Err(_) => {
                    return abort(UserError::UnKnown {
                        message: "invalid key type byte".into(),
                        location: location.clone(),
                    });
                }
            };

            let key_mode = match KeyMode::from_byte(value_bytes[value_bytes.len() - 1]) {
                Ok(m) => m,
                Err(_) => {
                    return abort(UserError::UnKnown {
                        message: "invalid key mode byte".into(),
                        location: location.clone(),
                    });
                }
            };

            // 6️⃣ 出力
            Ok(Output::InfoKey(InfoKey {
                key_name: key_name.to_string(),
                key_type: key_type.as_str().to_string(),
                key_mode: key_mode.as_str().to_string(),
            }))
        });

        // transaction 結果を UserError に変換
        result.map_err(|e| match e {
            TransactionError::Abort(user_err) => user_err,
            TransactionError::Storage(e) => UserError::SledError {
                message: e.to_string(),
                location,
            },
        })
    }
}
