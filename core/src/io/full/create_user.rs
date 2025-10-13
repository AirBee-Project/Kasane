use actix_web::http::Error;
use lmdb::{Cursor, DatabaseFlags, Transaction as _, WriteFlags};
use uuid::Uuid;

use crate::{
    io::{StorageTrait, full::Storage},
    json::output::Output,
    user_error::UserError,
};

impl StorageTrait for Storage {
    fn create_user(&self, user_name: &str, password: &str) -> Result<Output, UserError> {
        let location = location!();
        let user_bytes = user_name.as_bytes();
        let user_id: [u8; 16] = *Uuid::new_v4().as_bytes();

        // トランザクション作成
        let mut txn = self.env.begin_rw_txn()?;

        // SpaceTable に put
        txn.put(self.user, &user_bytes, &user_id, WriteFlags::empty())
            .map_err(|e| match e {
                lmdb::Error::KeyExist => UserError::UserAlreadyExists {
                    user_name: user_name.to_owned(),
                    location: location,
                },
                other => other.into(),
            })?;

        // コミット
        txn.commit()?;

        Ok(Output::Success)
    }
}
