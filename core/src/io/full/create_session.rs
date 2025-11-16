use std::time::{Duration, SystemTime, UNIX_EPOCH};

use redb::ReadableTable;
use uuid::Uuid;

use crate::{
    io::full::{
        kv_type::{user_session_key::UserSessionKey, uuid::UuidKey},
        Storage, USER_PASSWORD, USER_SESSION, USER_TABLE,
    },
    location,
    user_error::UserError,
};

use bcrypt::verify;

impl Storage {
    ///新しいSessionを作成する
    /// また、時間が切れたSessionIDを削除する
    pub fn create_session(
        &self,
        user_name: &str,
        password: &str,
        session_id: &Uuid,
        session_expiration_secs: u64,
    ) -> Result<u64, UserError> {
        //ユーザーが存在するのかの検証

        let write_txn = self.db.begin_write()?;
        let expires_at;

        {
            let table_user = write_txn.open_table(USER_TABLE)?;
            let table_password = write_txn.open_table(USER_PASSWORD)?;
            let mut table_session = write_txn.open_table(USER_SESSION)?;

            //UserIDを取得
            let user_id = match table_user.get(user_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::UserNameOrPasswordMissing);
                }
            };

            let hash = match table_password.get(user_id)? {
                Some(v) => v.value().to_owned(),
                None => {
                    return Err(UserError::UserNameOrPasswordMissing);
                }
            };

            //パスワードの照合
            if !verify(password, &hash).unwrap() {
                return Err(UserError::PasswordError {
                    message: "Incorrect password".to_string(),
                    location: location!(),
                });
            }

            let now = SystemTime::now();

            expires_at = now
                .checked_add(Duration::from_secs(session_expiration_secs))
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            //新しいSessionIDの発行
            let key = UserSessionKey {
                expires_at,
                session_id: session_id.clone().into(),
            };

            let _ = table_session.insert(key, UuidKey::from(session_id.clone()));

            //古いSessionIDの削除
            let delete_at = now
                .checked_sub(Duration::from_secs(session_expiration_secs))
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let start_key = UserSessionKey {
                expires_at: 0,
                session_id: UuidKey([0; 16]),
            };

            let end_key = UserSessionKey {
                expires_at: delete_at,
                session_id: UuidKey([0; 16]),
            };

            table_session.retain_in(start_key..=end_key, |_key, _value| false)?;
        }

        write_txn.commit()?;

        Ok(expires_at)
    }
}
