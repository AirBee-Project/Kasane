use lmdb::{Cursor, DatabaseFlags, Error as LmdbError, Transaction as _, WriteFlags};
use uuid::Uuid;

use crate::{
    io::{
        StorageTrait,
        full::{Storage, lmdb_error::LmdbResultExt},
    },
    json::{
        input::{KeyMode, KeyType},
        output::Output,
    },
    user_error::UserError,
};

impl StorageTrait for Storage {
    fn create_key(
        &self,
        space_name: &str,
        key_name: &str,
        key_type: KeyType,
        key_mode: KeyMode,
    ) -> Result<crate::json::output::Output, UserError> {
        let location = location!();
        let space_bytes = space_name.as_bytes();

        //トランザクションを作成
        let mut txn = self.env.begin_rw_txn().with_user_error()?;

        //space_idの取得を試みる
        let space_id = match txn.get(self.space, &space_bytes).with_user_error() {
            Ok(v) => v,
            Err(LmdbError::NotFound) => {
                return Err(UserError::SpaceNotFound {
                    space_name: space_name.to_owned(),
                    location,
                });
            }
            Err(e) => (e),
        };

        let key_id: [u8; 16] = *Uuid::new_v4().as_bytes();

        let key_bytes = [
            &space_id[..],
            key_name.as_bytes(),
            key_type.as_bytes(),
            key_mode.as_bytes(),
        ]
        .concat();

        txn.put(self.key, &key_bytes, &key_id, lmdb::WriteFlags::empty())
            .map_err(|e| match e {
                LmdbError::KeyExist => UserError::KeyAlreadyExists {
                    space_name: space_name.to_string(),
                    key_name: key_name.to_string(),
                    location,
                },
                _ => UserError::from(e),
            })?;

        txn.commit().with_user_error()?;
        Ok(Output::Success)
    }
}
