use sled::transaction::{ConflictableTransactionError, TransactionError, Transactional};
use uuid::Uuid;

use crate::{
    io::{
        StorageTrait,
        full::{Storage, tools::data_prefix::Data},
    },
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

impl StorageTrait for Storage {
    fn create_key(
        &self,
        space_name: &str,
        key_name: &str,
        key_type: KeyType,
        key_mode: KeyMode,
    ) -> Result<Output, UserError> {
        let location = location!();

        let mut space_bytes = vec![Data::Space as u8];
        space_bytes.extend_from_slice(space_name.as_bytes());

        let result = (&self.db).transaction(|tx| {
            // spaceが存在するかチェック
            let spaceid = match tx.get(&space_bytes)? {
                Some(val) => val,
                None => {
                    return Err(sled::transaction::ConflictableTransactionError::Abort(
                        CreateKeyTxError::SpaceNotFound,
                    ));
                }
            };

            // key_bytesを作成
            let mut key_bytes = vec![Data::Key as u8];
            key_bytes.extend_from_slice(&spaceid);
            key_bytes.extend_from_slice(key_name.as_bytes());
            key_bytes.extend_from_slice(key_type.as_bytes());
            key_bytes.extend_from_slice(key_mode.as_bytes());

            // すでに存在していないかチェック
            if tx.get(&key_bytes)?.is_some() {
                return Err(sled::transaction::ConflictableTransactionError::Abort(
                    CreateKeyTxError::KeyAlreadyExists,
                ));
            }

            // generate_idで一意IDを作成
            let id: u64 = tx.generate_id()?;
            let id_bytes = id.to_be_bytes();

            tx.insert(key_bytes, &id_bytes)?;
            Ok(())
        });

        match result {
            Ok(()) => Ok(Output::Success),
            Err(sled::transaction::TransactionError::Abort(CreateKeyTxError::SpaceNotFound)) => {
                Err(UserError::SpaceNotFound {
                    space_name: space_name.to_owned(),
                    location,
                })
            }
            Err(sled::transaction::TransactionError::Abort(CreateKeyTxError::KeyAlreadyExists)) => {
                Err(UserError::KeyAlreadyExists {
                    space_name: space_name.to_owned(),
                    key_name: key_name.to_owned(),
                    location,
                })
            }
            Err(sled::transaction::TransactionError::Storage(e)) => Err(UserError::UnKnown {
                message: e.to_string(),
                location,
            }),
        }
    }
}
