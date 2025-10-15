use actix_web::http::Error;
use uuid::Uuid;

use crate::{
    io::{
        StorageTrait,
        full::{
            Storage,
            tools::{data_prefix::Data, password_hash::hash_password},
        },
    },
    json::output::Output,
    user_error::UserError,
};
use argon2::password_hash::{self, SaltString};
use argon2::{self, Algorithm, Argon2, Params, Version};

enum CreateUserTxError {
    PasswordHashError(e),
    UserAlreadyExists,
}

impl StorageTrait for Storage {
    fn create_user(&self, user_name: &str, password: &str) -> Result<Output, UserError> {
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

            // ハッシュ化（?で早期リターン）
            let hashed = hash_password(password).map_err(|err| {
                sled::transaction::ConflictableTransactionError::Abort(
                    (CreateUserTxError::PasswordHashError(err)),
                )
            })?;

            tx.insert(password_key, &*hashed)?;

            Ok(())
        });

        // トランザクション結果の変換
        match result {
            Ok(()) => Ok(Output::Success),
            Err(sled::transaction::TransactionError::Abort(e)) => match e {
                CreateUserTxError::PasswordHashError(e) => Err(UserError::UnKnown {
                    message: e,
                    location: location,
                }),
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
