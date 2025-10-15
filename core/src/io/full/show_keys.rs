use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::output::{Output, Showkeys},
    user_error::UserError,
};

impl Storage {
    pub fn show_keys(&self, space_name: &str) -> Result<Output, UserError> {
        let location = location!();

        // space_idを取得
        let mut space_bytes = vec![Data::Space as u8];
        space_bytes.extend_from_slice(space_name.as_bytes());

        let space_id = match self.db.get(&space_bytes) {
            Ok(Some(id)) => id,
            Ok(None) => {
                return Err(UserError::SpaceNotFound {
                    space_name: space_name.to_string(),
                    location,
                });
            }
            Err(e) => {
                return Err(UserError::UnKnown {
                    message: e.to_string(),
                    location,
                });
            }
        };

        // スペースIDに紐づく全てのキーを取得
        let prefix: Vec<u8> = {
            let mut p = vec![Data::Key as u8];
            p.extend_from_slice(&space_id);
            p
        };

        let mut keys = Vec::new();

        // sled の scan_prefix で取得
        for item in self.db.scan_prefix(&prefix) {
            match item {
                Ok((key_bytes, _value_bytes)) => {
                    // key_bytes は [Data::Key, space_id..., key_name_bytes..., key_type, key_mode]
                    // key_name の部分を抽出
                    let key_name_start = 1 + space_id.len();
                    let key_name_end = key_bytes.len() - 2; // 最後の2バイトが key_type, key_mode
                    if key_name_start < key_name_end {
                        if let Ok(key_name_str) =
                            std::str::from_utf8(&key_bytes[key_name_start..key_name_end])
                        {
                            keys.push(key_name_str.to_string());
                        }
                    }
                }
                Err(e) => {
                    return Err(UserError::UnKnown {
                        message: e.to_string(),
                        location,
                    });
                }
            }
        }

        Ok(Output::Showkeys(Showkeys { key_names: keys }))
    }
}
