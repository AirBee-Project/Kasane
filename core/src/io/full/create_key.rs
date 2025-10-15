use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::{
        input::{KeyMode, KeyType},
        output::Output,
    },
    user_error::UserError,
};
use tokio::sync::MutexGuard;

impl Storage {
    pub async fn create_key(
        &self,
        space_name: &str,
        key_name: &str,
        key_type: KeyType,
        key_mode: KeyMode,
    ) -> Result<Output, UserError> {
        // space key を作成
        let mut space_bytes = vec![Data::Space as u8];
        space_bytes.extend_from_slice(space_name.as_bytes());

        // 最小範囲での排他制御
        let _lock: MutexGuard<'_, ()> = self.lock.lock().await;

        // space が存在するかチェック
        let space_id = self
            .db
            .get(&space_bytes)
            .map_err(|e| UserError::UnKnown {
                message: e.to_string(),
                location: location!(),
            })?
            .ok_or(UserError::SpaceNotFound {
                space_name: space_name.to_owned(),
                location: location!(),
            })?;

        // key 用のバイト列を作成
        let mut key_bytes = vec![Data::Key as u8];
        key_bytes.extend_from_slice(&space_id);
        key_bytes.extend_from_slice(key_name.as_bytes());

        // key がすでに存在するかチェック
        if self
            .db
            .get(&key_bytes)
            .map_err(|e| UserError::UnKnown {
                message: e.to_string(),
                location: location!(),
            })?
            .is_some()
        {
            return Err(UserError::KeyAlreadyExists {
                space_name: space_name.to_owned(),
                key_name: key_name.to_owned(),
                location: location!(),
            });
        }

        // ID + type + mode を value に格納
        let id: u64 = self.db.generate_id().map_err(|e| UserError::UnKnown {
            message: e.to_string(),
            location: location!(),
        })?;
        let mut value_bytes = id.to_be_bytes().to_vec();
        value_bytes.extend_from_slice(key_type.as_bytes());
        value_bytes.extend_from_slice(key_mode.as_bytes());

        // key を insert
        self.db
            .insert(key_bytes, value_bytes)
            .map_err(|e| UserError::UnKnown {
                message: e.to_string(),
                location: location!(),
            })?;

        Ok(Output::Success)
    }
}
