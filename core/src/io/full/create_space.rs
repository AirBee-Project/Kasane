use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::output::Output,
    user_error::UserError,
};
use sled::Db;
use tokio::sync::MutexGuard;
use uuid::Uuid;

impl Storage {
    pub async fn create_space(&self, space_name: &str) -> Result<Output, UserError> {
        let mut space_bytes = vec![Data::Space as u8];
        space_bytes.extend_from_slice(space_name.as_bytes());

        // 最小範囲での排他制御
        let _lock: MutexGuard<'_, ()> = self.lock.lock().await;

        // 存在チェック
        if self
            .db
            .get(&space_bytes)
            .map_err(|e| UserError::UnKnown {
                message: e.to_string(),
                location: location!(),
            })?
            .is_some()
        {
            return Err(UserError::SpaceAlreadyExists {
                space_name: space_name.to_string(),
                location: location!(),
            });
        }

        // 一意ID生成
        let id: u64 = self.db.generate_id().map_err(|e| UserError::UnKnown {
            message: e.to_string(),
            location: location!(),
        })?;
        let id_bytes = id.to_be_bytes();

        // insert
        self.db
            .insert(space_bytes, &id_bytes)
            .map_err(|e| UserError::UnKnown {
                message: e.to_string(),
                location: location!(),
            })?;

        Ok(Output::Success)
    }
}
