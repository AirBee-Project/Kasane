use crate::{
    io::full::Storage,
    json::{
        input::{KeyMode, KeyType},
        output::{InfoKey, Output},
    },
    location,
    user_error::UserError,
};

impl Storage {
    /// space 内のキー情報を取得
    pub fn info_key(&self, space_name: &str, key_name: &str) -> Result<Output, UserError> {
        let space_bytes = space_name.as_bytes();

        // space_id を取得
        let space_id = self
            .space
            .get(space_bytes)?
            .ok_or(UserError::SpaceNotFound {
                space_name: space_name.to_string(),
                location: location!(),
            })?;

        // key の完全なバイト列を作成
        let mut key_bytes = Vec::new();
        key_bytes.extend_from_slice(&space_id);
        key_bytes.extend_from_slice(key_name.as_bytes());

        // key の値を取得
        let value_bytes = self.key.get(&key_bytes)?.ok_or(UserError::KeyNotFound {
            space_name: space_name.to_string(),
            key_name: key_name.to_string(),
            location: location!(),
        })?;

        // 値は [key_id(8バイト) + key_type + key_mode]
        if value_bytes.len() < 8 {
            return Err(UserError::UnKnown {
                message: "Invalid key value length".to_string(),
                location: location!(),
            });
        }

        let key_type_start = 8;
        let key_type_end = value_bytes.len() - 1; // key_mode は最後の1バイト想定
        let key_type_bytes = &value_bytes[key_type_start..key_type_end];
        let key_mode_bytes = &value_bytes[key_type_end..];

        let key_type = KeyType::from_byte(key_type_bytes[0])?.as_str().to_string();

        let key_mode = KeyMode::from_byte(key_mode_bytes[0])?.as_str().to_string();

        Ok(Output::InfoKey(InfoKey {
            key_name: key_name.to_string(),
            key_type,
            key_mode,
        }))
    }
}
