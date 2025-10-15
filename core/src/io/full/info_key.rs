use sled::transaction::{ConflictableTransactionError, TransactionError, Transactional};

use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::{
        input::{KeyMode, KeyType},
        output::{InfoKey, Output},
    },
    user_error::UserError,
};

enum InfoKeyTxError {
    SpaceNotFound,
    KeyNotFound,
    InvalidValueLength,
    InvalidKeyType,
    InvalidKeyMode,
}

impl Storage {
    pub fn info_key(&self, space_name: &str, key_name: &str) -> Result<Output, UserError> {
        let location = location!();

        let result = (&self.db).transaction(|tx| {
            // 1. space_id を取得
            let mut space_bytes = vec![Data::Space as u8];
            space_bytes.extend_from_slice(space_name.as_bytes());

            let space_id = tx
                .get(&space_bytes)?
                .ok_or(ConflictableTransactionError::Abort(
                    InfoKeyTxError::SpaceNotFound,
                ))?;

            // 2. key_bytes 作成
            let mut key_bytes = vec![Data::Key as u8];
            key_bytes.extend_from_slice(&space_id);
            key_bytes.extend_from_slice(key_name.as_bytes());

            // 3. key の取得
            let value_bytes = tx
                .get(&key_bytes)?
                .ok_or(ConflictableTransactionError::Abort(
                    InfoKeyTxError::KeyNotFound,
                ))?;

            if value_bytes.len() < 10 {
                return Err(ConflictableTransactionError::Abort(
                    InfoKeyTxError::InvalidValueLength,
                ));
            }

            // 4. type/mode を末尾 2 バイトから復元
            let key_type = KeyType::from_byte(value_bytes[value_bytes.len() - 2])
                .map_err(|_| ConflictableTransactionError::Abort(InfoKeyTxError::InvalidKeyType))?;
            let key_mode = KeyMode::from_byte(value_bytes[value_bytes.len() - 1])
                .map_err(|_| ConflictableTransactionError::Abort(InfoKeyTxError::InvalidKeyMode))?;

            Ok(Output::InfoKey(InfoKey {
                key_name: key_name.to_string(),
                key_type: key_type.as_str().to_string(),
                key_mode: key_mode.as_str().to_string(),
            }))
        });

        // transaction 結果を UserError に変換
        match result {
            Ok(output) => Ok(output),
            Err(TransactionError::Abort(err)) => match err {
                InfoKeyTxError::SpaceNotFound => Err(UserError::SpaceNotFound {
                    space_name: space_name.to_string(),
                    location,
                }),
                InfoKeyTxError::KeyNotFound => Err(UserError::KeyNotFound {
                    space_name: space_name.to_string(),
                    key_name: key_name.to_string(),
                    location,
                }),
                InfoKeyTxError::InvalidValueLength => Err(UserError::UnKnown {
                    message: "invalid key value length".into(),
                    location,
                }),
                InfoKeyTxError::InvalidKeyType => Err(UserError::UnKnown {
                    message: "invalid key type byte".into(),
                    location,
                }),
                InfoKeyTxError::InvalidKeyMode => Err(UserError::UnKnown {
                    message: "invalid key mode byte".into(),
                    location,
                }),
            },
            Err(TransactionError::Storage(e)) => Err(UserError::UnKnown {
                message: e.to_string(),
                location,
            }),
        }
    }
}
