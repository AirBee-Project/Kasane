use redb::{ReadableDatabase, ReadableTable};

use crate::{
    io::full::{Storage, SPACE_TABLE, USER_TABLE},
    interface::output::{Output, ShowSpaces, ShowUsers},
    user_error::UserError,
};

impl Storage {
    pub fn show_users(&self) -> Result<Output, UserError> {
        let read_txn = self.db.begin_read()?;

        let mut users = vec![];

        {
            let table_user = read_txn.open_table(USER_TABLE)?;

            for user in table_user.iter()? {
                let (key, _) = user?;
                users.push(key.value().to_string());
            }
        }

        return Ok(Output::ShowUsers(ShowUsers { users }));
    }
}
