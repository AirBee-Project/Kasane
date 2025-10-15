use sled::transaction::{ConflictableTransactionResult, TransactionError};
use uuid::Uuid;

use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::output::Output,
    user_error::UserError,
};

impl Storage {
    pub fn create_space(&self, space_name: &str) -> Result<Output, UserError> {
        let mut space_bytes = vec![Data::Space as u8];
        space_bytes.extend_from_slice(space_name.as_bytes());

        let result = (&self.db).transaction(|tx| {
            // すでに存在していないかチェック
            if tx.get(&space_bytes)?.is_some() {
                return Err(sled::transaction::ConflictableTransactionError::Abort(()));
            }

            // generate_id で一意IDを作成
            let id: u64 = tx.generate_id()?; // u64
            let id_bytes = id.to_be_bytes(); // バイト列に変換

            // insert
            tx.insert(space_bytes.clone(), &id_bytes)?;

            Ok(())
        });

        match result {
            Ok(()) => Ok(Output::Success),
            Err(sled::transaction::TransactionError::Abort(_)) => {
                Err(UserError::SpaceAlreadyExists {
                    space_name: space_name.to_string(),
                    location: location!(),
                })
            }
            Err(sled::transaction::TransactionError::Storage(e)) => Err(UserError::UnKnown {
                message: e.to_string(),
                location: location!(),
            }),
        }
    }
}
