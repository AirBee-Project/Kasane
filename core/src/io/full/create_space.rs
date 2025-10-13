use actix_web::http::Error;
use lmdb::{Cursor, DatabaseFlags, Transaction as _, WriteFlags};
use uuid::Uuid;

use crate::{
    io::{
        StorageTrait,
        full::{Storage, lmdb_error::LmdbResultExt},
    },
    json::output::Output,
    user_error::UserError,
};

impl StorageTrait for Storage {
    fn create_space(&self, space_name: &str) -> Result<Output, UserError> {
        let location = location!();
        let space_bytes = space_name.as_bytes();
        let space_id: [u8; 16] = *Uuid::new_v4().as_bytes();

        // トランザクション作成
        let mut txn = self.env.begin_rw_txn().catch_lmdb_error()?; // ? だけで雑に LMDB エラー拾える

        // SpaceTable に put
        txn.put(self.space, &space_bytes, &space_id, WriteFlags::empty())
            .map_err(|e| match e {
                lmdb::Error::KeyExist => UserError::SpaceAlreadyExists {
                    space_name: space_name.to_owned(),
                    location,
                },
                other => other.catch_lmdb_error().unwrap_err(), // それ以外は catch_lmdb_error() で雑に変換
            })?;

        // コミット
        txn.commit().catch_lmdb_error()?;

        Ok(Output::Success)
    }
}
