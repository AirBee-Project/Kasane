use crate::{
    io::full::{Storage, tools::data_prefix::Data},
    json::{self, output::Output},
    user_error::UserError,
};

impl Storage {
    pub fn show_spaces(&self) -> Result<Output, UserError> {
        let spaces = {
            // ロック範囲を最小化（必要な処理だけ）
            let prefix = [Data::Space as u8];
            let iter = self.db.scan_prefix(&prefix);

            let mut result = Vec::new();
            for item in iter {
                let (key, _value) = item.map_err(|e| UserError::UnKnown {
                    message: e.to_string(),
                    location: location!(),
                })?;

                if key.len() > 1 {
                    if let Ok(name) = String::from_utf8(key[1..].to_vec()) {
                        result.push(name);
                    }
                }
            }

            result // この時点で iter がスコープを抜け、必要ならロックも解除
        };

        Ok(Output::ShowSpaces(json::output::ShowSpaces {
            space_names: spaces,
        }))
    }
}
