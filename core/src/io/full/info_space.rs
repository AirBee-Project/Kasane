use crate::{
    io::full::Storage,
    json::{
        input::{KeyMode, KeyType},
        output::{InfoKey, InfoSpace, Output},
    },
    location,
    user_error::UserError,
};

impl Storage {
    /// space 内の全キー情報を取得
    pub fn info_space(&self, space_name: &str) -> Result<Output, UserError> {
        let space_bytes = space_name.as_bytes();

        // space_id を取得
        let space_id = self
            .space
            .get(space_bytes)?
            .ok_or(UserError::SpaceNotFound {
                space_name: space_name.to_string(),
                location: location!(),
            })?;

        let mut key_infos = Vec::new();

        // key データベースをイテレーションして、space_id で始まるキーを抽出
        for item in self.key.iter() {
            let (key_bytes, value_bytes) = item.map_err(|e| UserError::UnKnown {
                message: e.to_string(),
                location: location!(),
            })?;

            if key_bytes.starts_with(&space_id) {
                // space_id の後ろが key_name
                let key_name_bytes = &key_bytes[space_id.len()..];
                let key_name =
                    String::from_utf8(key_name_bytes.to_vec()).map_err(|e| UserError::UnKnown {
                        message: e.to_string(),
                        location: location!(),
                    })?;

                if value_bytes.len() < 8 {
                    return Err(UserError::UnKnown {
                        message: "Invalid key value length".to_string(),
                        location: location!(),
                    });
                }

                // 値は [key_id(8バイト) + key_type + key_mode] 形式
                let key_type_start = 8;
                let key_type_end = value_bytes.len() - 1;
                let key_type_bytes = &value_bytes[key_type_start..key_type_end];
                let key_mode_bytes = &value_bytes[key_type_end..];

                let key_type = KeyType::from_byte(key_type_bytes[0])?.as_str().to_string();

                let key_mode = KeyMode::from_byte(key_mode_bytes[0])?.as_str().to_string();

                key_infos.push(InfoKey {
                    key_name,
                    key_type,
                    key_mode,
                });
            }
        }

        Ok(Output::InfoSpace(InfoSpace {
            space_name: space_name.to_string(),
            key_names: key_infos,
        }))
    }
}
