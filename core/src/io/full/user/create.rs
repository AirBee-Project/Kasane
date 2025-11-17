use bcrypt::{hash, DEFAULT_COST};
use redb::ReadableTable;
use uuid::Uuid;

use crate::{
    io::full::{redb_implementations::uuid::UuidKey, Storage, USER_PASSWORD, USER_TABLE},
    json::output::Output,
    location,
    user_error::UserError,
};

impl Storage {
    pub fn create_user(&self, user_name: &str, password: &str) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table_user = write_txn.open_table(USER_TABLE)?;
            let mut table_password = write_txn.open_table(USER_PASSWORD)?;

            // 既存ユーザーチェック
            if table_user.get(user_name)?.is_some() {
                return Err(UserError::UserAlreadyExists {
                    user_name: user_name.to_string(),
                    location: location!(),
                });
            }

            let user_id = loop {
                let id = UuidKey::new();
                if table_password.get(id)?.is_none() {
                    break id;
                }
            };

            let hashed = hash(password, DEFAULT_COST)?;

            table_password.insert(user_id, hashed.as_str())?;
            table_user.insert(user_name, user_id)?;
        }

        write_txn.commit()?;
        Ok(Output::Success)
    }
}
