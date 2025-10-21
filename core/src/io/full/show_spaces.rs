use crate::{
    io::full::Storage,
    json::output::{Output, ShowSpaces},
    user_error::UserError,
};
use sled::IVec;

impl Storage {
    /// 登録されているすべての space 名を取得
    pub fn show_spaces(&self) -> Result<Output, UserError> {
        let mut spaces = Vec::new();

        for item in self.space.iter() {
            let (key_bytes, _value) = item.map_err(|e| UserError::UnKnown {
                message: e.to_string(),
                location: location!(),
            })?;

            if let Ok(space_name) = std::str::from_utf8(&key_bytes) {
                spaces.push(space_name.to_string());
            }
        }

        Ok(Output::ShowSpaces(ShowSpaces {
            space_names: spaces,
        }))
    }
}
