use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::{
        input::{KeyMode, KeyType},
        output::{InfoKey, Output},
    },
    user_error::UserError,
};

impl Storage {
    /// 単一キー情報を取得
    pub fn info_key(&self, space_name: &str, key_name: &str) -> Result<Output, UserError> {
        // space_id を取得
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

        // key の prefix 作成
        let mut key_bytes = vec![Data::Key as u8];
        key_bytes.extend_from_slice(&space_id);
        key_bytes.extend_from_slice(key_name.as_bytes());

        // scan_prefix で取得
        let (_, value_bytes) =
            self.db
                .scan_prefix(&key_bytes)
                .next()
                .ok_or(UserError::KeyNotFound {
                    space_name: space_name.to_string(),
                    key_name: key_name.to_string(),
                    location: location!(),
                })??;

        if value_bytes.len() < 10 {
            return Err(UserError::UnKnown {
                message: "invalid key value length".into(),
                location: location!(),
            });
        }

        // type/mode は末尾 2 バイト
        let key_type = KeyType::from_byte(value_bytes[value_bytes.len() - 2])?;
        let key_mode = KeyMode::from_byte(value_bytes[value_bytes.len() - 1])?;

        return Ok(Output::InfoKey(InfoKey {
            key_name: key_name.to_string(),
            key_type: key_type.as_str().to_string(),
            key_mode: key_mode.as_str().to_string(),
        }));
    }
}
