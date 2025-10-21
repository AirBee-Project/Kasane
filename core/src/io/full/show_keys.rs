use crate::{
    io::full::Storage,
    json::output::{Output, Showkeys},
    user_error::UserError,
};

impl Storage {
    /// space 内の全キー一覧を取得
    pub fn show_keys(&self, space_name: &str) -> Result<Output, UserError> {
        let space_bytes = space_name.as_bytes();

        // space_id を取得
        let space_id = self
            .space
            .get(space_bytes)?
            .ok_or(UserError::SpaceNotFound {
                space_name: space_name.to_string(),
                location: location!(),
            })?;

        let mut key_names = Vec::new();

        // key データベースをイテレーションして、space_id で始まるキーを抽出
        for item in self.key.iter() {
            let (key_bytes, _value) = item.map_err(|e| UserError::UnKnown {
                message: e.to_string(),
                location: location!(),
            })?;

            if key_bytes.starts_with(&space_id) {
                // space_id の後ろが実際の key_name
                let key_name_bytes = &key_bytes[space_id.len()..];
                if let Ok(key_name) = std::str::from_utf8(key_name_bytes) {
                    key_names.push(key_name.to_string());
                }
            }
        }

        Ok(Output::Showkeys(Showkeys { key_names }))
    }
}
