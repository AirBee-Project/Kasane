use crate::{
    io::{StorageTrait, full::Storage},
    json::output::{Output, Showkeys},
    user_error::UserError,
};
use lmdb::{Cursor, Transaction as _};

impl StorageTrait for Storage {
    fn show_users(&self) -> Result<Output, UserError> {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.user)?;
        let mut users = Vec::new();

        for result in cursor.iter_start() {
            let (key_bytes, _val_bytes) = result;
            let s: &str = std::str::from_utf8(key_bytes)?;
            let string: String = s.to_string();
            users.push(string);
        }

        Ok(Output::ShowUsers(crate::json::output::ShowUsers {
            users: users,
        }))
    }
}
