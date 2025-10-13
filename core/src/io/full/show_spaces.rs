use lmdb::{Cursor, Transaction as _};

use crate::{
    io::{StorageTrait, full::Storage},
    json::output::Output,
    user_error::UserError,
};

impl StorageTrait for Storage {
    fn show_spaces(&self) -> Result<Output, UserError> {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.space)?;
        let mut spaces = Vec::new();

        for result in cursor.iter_start() {
            let (key_bytes, _val_bytes) = result;
            let s: &str = std::str::from_utf8(key_bytes)?;
            let string: String = s.to_string();
            spaces.push(string);
        }

        Ok(Output::ShowSpaces(crate::json::output::ShowSpaces {
            space_names: spaces,
        }))
    }
}
