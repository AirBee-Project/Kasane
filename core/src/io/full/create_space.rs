use lmdb::{Cursor, DatabaseFlags, Error as LmdbError, Transaction as _, WriteFlags};
use uuid::Uuid;

use crate::{
    io::{StorageTrait, full::Storage},
    json::output::Output,
    user_error::UserError,
};

impl StorageTrait for Storage {
    fn create_space(&self, spacename: &str) -> Result<Output, UserError> {
        let space_id: [u8; 16] = *Uuid::new_v4().as_bytes();
        let space_bytes = spacename.as_bytes();
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(
            self.space,
            &space_bytes,
            &space_id,
            lmdb::WriteFlags::empty(),
        )
        //既に同じ名前のSpaceが存在する場合にはエラーを返す
        .map_err(|e| match e {
            LmdbError::KeyExist => UserError::SpaceAlreadyExists {
                space_name: spacename.to_owned(),
                location: todo!(),
            },
            _ => UserError::from(e),
        })?;
        txn.commit()?;
        Ok(Output::Success)
    }
}
