use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::output::Output,
    user_error::UserError,
};

enum CreateUserTxError {
    UserAlreadyExists,
}

impl Storage {
    pub fn create_user(&self, user_name: &str, hash: String) -> Result<Output, UserError> {
        let location = location!();
        let mut user_bytes = vec![Data::User as u8];
        user_bytes.extend_from_slice(user_name.as_bytes());

        let result = (&self.db).transaction(|tx| {
            // すでに存在していないかチェック
            if tx.get(&user_bytes)?.is_some() {
                return Err(sled::transaction::ConflictableTransactionError::Abort(
                    (CreateUserTxError::UserAlreadyExists),
                ));
            }

            // generate_id で一意IDを作成
            let id: u64 = tx.generate_id()?;
            let id_bytes = id.to_be_bytes();

            // insert user ID
            tx.insert(user_bytes.clone(), &id_bytes)?;

            // passwordを処理
            let mut password_key = vec![Data::Password as u8];
            password_key.extend_from_slice(&id_bytes);

            tx.insert(password_key, &*hash)?;

            Ok(())
        });

        // トランザクション結果の変換
        match result {
            Ok(()) => Ok(Output::Success),
            Err(sled::transaction::TransactionError::Abort(e)) => match e {
                CreateUserTxError::UserAlreadyExists => Err(UserError::UserAlreadyExists {
                    user_name: user_name.to_string(),
                    location,
                }),
            },
            Err(sled::transaction::TransactionError::Storage(e)) => Err(UserError::UnKnown {
                message: e.to_string(),
                location,
            }),
        }
    }
}
