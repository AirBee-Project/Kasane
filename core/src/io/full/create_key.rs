use lmdb::{Cursor, DatabaseFlags, Error as LmdbError, Transaction as _, WriteFlags};
use uuid::Uuid;

use crate::{
    io::{StorageTrait, full::Storage},
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
    ) -> Result<Output, UserError> {
        let location = location!();
        let space_id = Self::get_space_id(&self, space_name)?;
        let key_id: [u8; 16] = *Uuid::new_v4().as_bytes();

        // トランザクション作成
        let mut txn = self.env.begin_rw_txn()?;

        let key_bytes = [
            &space_id[..],
            key_name.as_bytes(),
            key_type.as_bytes(),
            key_mode.as_bytes(),
        ]
        .concat();

        txn.put(self.key, &key_bytes, &key_id, WriteFlags::empty())
            .map_err(|e| match e {
                lmdb::Error::KeyExist => UserError::KeyAlreadyExists {
                    space_name: space_name.to_owned(),
                    key_name: key_name.to_owned(),
                    location: location,
                },
                other => other.into(),
            })?;

        txn.commit()?;

        Ok(Output::Success)
    }
}
