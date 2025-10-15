use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::{self, output::Output},
    user_error::UserError,
};

impl Storage {
    pub fn show_spaces(&self) -> Result<Output, UserError> {
        let location = location!();

        let mut spaces = Vec::new();

        // プレフィックススキャン: Data::Space (スペース情報のみ取得)
        let prefix = [Data::Space as u8];
        let iter = self.db.scan_prefix(&prefix);

        for item in iter {
            match item {
                Ok((key, _value)) => {
                    // key: [Data::Space as u8] + space_name
                    if key.len() > 1 {
                        let space_name_bytes = &key[1..];
                        if let Ok(name) = String::from_utf8(space_name_bytes.to_vec()) {
                            spaces.push(name);
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

        Ok(Output::ShowSpaces(json::output::ShowSpaces {
            space_names: spaces,
        }))
    }
}
