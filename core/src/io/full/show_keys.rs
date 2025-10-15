use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::output::{Output, Showkeys},
    user_error::UserError,
};
impl Storage {
    /// space 内の全キー一覧を取得
    pub fn show_keys(&self, space_name: &str) -> Result<Output, UserError> {
        // space_id 取得
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

        // prefix = Data::Key + space_id
        let mut prefix = vec![Data::Key as u8];
        prefix.extend_from_slice(&space_id);

        let mut keys = Vec::new();

        for item in self.db.scan_prefix(&prefix) {
            let (key_bytes, _value_bytes) = item.map_err(|e| UserError::UnKnown {
                message: e.to_string(),
                location: location!(),
            })?;

            // key_name = key_bytes[1 + space_id.len() .. len]
            // value に type/mode が格納されているので key_name の末尾2バイトは除外
            if key_bytes.len() < 1 + space_id.len() + 1 {
                continue; // 不正な key はスキップ
            }

            let key_name_bytes = &key_bytes[1 + space_id.len()..]; // key_name + ?
            // key_name だけ取り出すため、末尾2バイトを除外
            if key_name_bytes.len() >= 2 {
                let key_name_bytes = &key_name_bytes[..key_name_bytes.len() - 2];
                if let Ok(key_name) = std::str::from_utf8(key_name_bytes) {
                    keys.push(key_name.to_string());
                }
            }
        }

        Ok(Output::Showkeys(Showkeys { key_names: keys }))
    }
}
