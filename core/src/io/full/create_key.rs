use sled::transaction::{ConflictableTransactionError, TransactionError, Transactional};
use uuid::Uuid;

use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::{
        input::{KeyMode, KeyType},
        output::Output,
    },
    user_error::UserError,
};
enum CreateKeyTxError {
    SpaceNotFound,
    KeyAlreadyExists,
}

impl Storage {
    pub fn create_key(
        &self,
        space_name: &str,
        key_name: &str,
        key_type: KeyType,
        key_mode: KeyMode,
    ) -> Result<Output, UserError> {
        let mut space_bytes = vec![Data::Space as u8];
        space_bytes.extend_from_slice(space_name.as_bytes());

        let result = (&self.db).transaction(|tx| {
            // space が存在するか
            let space_id = tx
                .get(&space_bytes)?
                .ok_or(ConflictableTransactionError::Abort(
                    CreateKeyTxError::SpaceNotFound,
                ))?;

            // key 作成
            let mut key_bytes = vec![Data::Key as u8];
            key_bytes.extend_from_slice(&space_id);
            key_bytes.extend_from_slice(key_name.as_bytes());

            // 存在チェック
            if tx.get(&key_bytes)?.is_some() {
                return Err(ConflictableTransactionError::Abort(
                    CreateKeyTxError::KeyAlreadyExists,
                ));
            }

            // ID + type + mode を value に格納
            let id: u64 = tx.generate_id()?;
            let mut value_bytes = id.to_be_bytes().to_vec();
            value_bytes.extend_from_slice(key_type.as_bytes());
            value_bytes.extend_from_slice(key_mode.as_bytes());

            tx.insert(key_bytes, value_bytes)?;
            Ok(())
        });

        match result {
            Ok(()) => Ok(Output::Success),
            Err(TransactionError::Abort(CreateKeyTxError::SpaceNotFound)) => {
                Err(UserError::SpaceNotFound {
                    space_name: space_name.to_owned(),
                    location: location!(),
                })
            }
            Err(TransactionError::Abort(CreateKeyTxError::KeyAlreadyExists)) => {
                Err(UserError::KeyAlreadyExists {
                    space_name: space_name.to_owned(),
                    key_name: key_name.to_owned(),
                    location: location!(),
                })
            }
            Err(TransactionError::Storage(e)) => Err(UserError::UnKnown {
                message: e.to_string(),
                location: location!(),
            }),
        }
    }
}
