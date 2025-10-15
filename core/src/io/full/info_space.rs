use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::input::{KeyMode, KeyType},
    json::output::{InfoKey, InfoSpace, Output},
    user_error::UserError,
};
use tokio::sync::MutexGuard;

impl Storage {
    pub async fn info_space(&self, space_name: &str) -> Result<Output, UserError> {
        // 1. space_id と prefix を最小ロックで取得
        let (space_id, prefix) = {
            let _lock: MutexGuard<'_, ()> = self.lock.lock().await;

            let mut space_bytes = vec![Data::Space as u8];
            space_bytes.extend_from_slice(space_name.as_bytes());

            let space_id = self
                .db
                .get(&space_bytes)
                .map_err(|e| UserError::UnKnown {
                    message: e.to_string(),
                    location: location!(),
                })?
                .ok_or(UserError::SpaceNotFound {
                    space_name: space_name.to_string(),
                    location: location!(),
                })?;

            let mut prefix = vec![Data::Key as u8];
            prefix.extend_from_slice(&space_id);

            (space_id, prefix)
        };

        // 2. scan_prefix でキー情報を取得（ロック外でOK）
        let mut key_infos = Vec::new();

        for item in self.db.scan_prefix(&prefix) {
            let (key_bytes, value_bytes) = item.map_err(|e| UserError::UnKnown {
                message: e.to_string(),
                location: location!(),
            })?;

            // key_name を安全に UTF-8 変換
            if key_bytes.len() <= 1 + space_id.len() {
                continue;
            }
            let key_name_bytes = &key_bytes[1 + space_id.len()..];
            let key_name = match std::str::from_utf8(key_name_bytes) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };

            // value_bytes: [id(8)] + [type(1)] + [mode(1)]
            if value_bytes.len() < 10 {
                continue;
            }
            let key_type_byte = value_bytes[8];
            let key_mode_byte = value_bytes[9];

            let key_type = KeyType::from_byte(key_type_byte)?.as_str().to_string();
            let key_mode = KeyMode::from_byte(key_mode_byte)?.as_str().to_string();

            key_infos.push(InfoKey {
                key_name,
                key_type,
                key_mode,
            });
        }

        Ok(Output::InfoSpace(InfoSpace {
            space_name: space_name.to_string(),
            key_names: key_infos,
        }))
    }
}
