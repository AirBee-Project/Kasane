use std::time::{SystemTime, UNIX_EPOCH};

use redb::ReadableDatabase;
use uuid::Uuid;

use crate::{
    io::full::{
        kv_type::{user_session_key::UserSessionKey, uuid::UuidKey},
        Storage, USER_SESSION,
    },
    location,
    user_error::UserError,
};

impl Storage {
    /// SessionIDが有効かを検証して、有効な場合はUserIDを返す
    pub fn verify_session(&self, session_id: &str) -> Result<UuidKey, UserError> {
        let session_uuid = Uuid::parse_str(session_id).map_err(|_| UserError::ParseError {
            message: "Invalid session ID".to_string(),
            location: location!(),
        })?;

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let user_id;

        let read_txn = self.db.begin_read()?;
        {
            let table_session = read_txn.open_table(USER_SESSION)?;

            // 範囲検索の開始と終了を作る
            let start_key = UserSessionKey {
                expires_at: now_secs,
                session_id: session_uuid.into(),
            };
            let end_key = UserSessionKey {
                expires_at: u64::MAX,
                session_id: session_uuid.into(),
            };

            // 範囲検索
            user_id = match table_session.range(start_key..=end_key)?.next() {
                Some(result) => {
                    let (key, value) = result?;
                    // セッションIDが一致するか確認
                    if key.value().session_id == session_uuid.into() {
                        value.value()
                    } else {
                        return Err(UserError::SessionError {
                            message: "Session not found or expired".to_string(),
                            location: location!(),
                        });
                    }
                }
                None => {
                    return Err(UserError::SessionError {
                        message: "Session not found or expired".to_string(),
                        location: location!(),
                    });
                }
            };
        }

        Ok(user_id)
    }
}
