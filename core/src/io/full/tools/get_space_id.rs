use crate::{io::full::Storage, user_error::UserError};

use lmdb::{Error as LmdbError, Transaction as _};

impl Storage {
    pub fn get_space_id(&self, space_name: &str) -> Result<Vec<u8>, UserError> {
        let txn = self.env.begin_ro_txn()?; // 読み取り専用
        let location = location!();

        let space_id = match txn.get(self.space, &space_name.as_bytes()) {
            Ok(v) => v.to_owned(), // コピーして返す
            Err(LmdbError::NotFound) => {
                return Err(UserError::SpaceNotFound {
                    space_name: space_name.to_owned(),
                    location,
                });
            }
            Err(e) => return Err(e.into()),
        };

        Ok(space_id)
    }
}
