use crate::{
    io::full::{
        Storage, PERMISSION_DATABASE, PERMISSION_KEY, PERMISSION_SPACE, PERMISSION_USER,
        USER_PASSWORD, USER_TABLE,
    },
    json::output::Output,
    user_error::UserError,
};

impl Storage {
    pub fn drop_user(&self, user_name: &str) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table_user = write_txn.open_table(USER_TABLE)?;
            let mut table_password = write_txn.open_table(USER_PASSWORD)?;
            let mut table_permission_database =
                write_txn.open_multimap_table(PERMISSION_DATABASE)?;
            let mut table_permission_space = write_txn.open_multimap_table(PERMISSION_SPACE)?;
            let mut table_permission_key = write_txn.open_multimap_table(PERMISSION_KEY)?;
            let mut table_permission_user = write_txn.open_multimap_table(PERMISSION_USER)?;

            let userid = match table_user.remove(user_name)? {
                Some(v) => v.value(),
                None => {
                    return Err(UserError::UserNotFound {
                        user_name: user_name.to_owned(),
                    });
                }
            };

            table_password.remove(userid)?;
            table_permission_database.remove_all(userid)?;
            table_permission_space.remove_all(userid)?;
            table_permission_key.remove_all(userid)?;
            table_permission_user.remove_all(userid)?;
        }

        todo!()
    }
}
