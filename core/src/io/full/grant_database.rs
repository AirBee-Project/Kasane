use actix_web::http::Error;
use lmdb::{Cursor, DatabaseFlags, Transaction as _, WriteFlags};
use uuid::Uuid;

use crate::{
    io::{StorageTrait, full::Storage},
    json::{
        input::{AllOrChoose, CommandDatabase, DatabaseCommand},
        output::Output,
    },
    user_error::UserError,
};

impl StorageTrait for Storage {
    fn grant_database(
        &self,
        user_name: &str,
        command: DatabaseCommand,
    ) -> Result<Output, UserError> {
        let user_id = Self::get_user_id(&self, user_name)?;
        let mut txn = self.env.begin_rw_txn()?;

        let existing_bytes = match txn.get(self.grant, &user_id) {
            Ok(v) => v.to_vec(),
            Err(lmdb::Error::NotFound) => Vec::new(), // まだ権限がなければ空ベクタ
            Err(e) => return Err(e.into()),
        };

        let mut existing_cmds = DatabaseCommand::from_bytes(&existing_bytes)?;

        let new_cmds = match (&mut existing_cmds, &command) {
            (AllOrChoose::All, _) => AllOrChoose::All,
            (_, AllOrChoose::All) => AllOrChoose::All,
            (AllOrChoose::Choose(existing), AllOrChoose::Choose(new)) => {
                for cmd in new {
                    if !existing.contains(cmd) {
                        existing.push(cmd.clone());
                    }
                }
                AllOrChoose::Choose(existing.clone())
            }
        };

        let new_bytes = new_cmds.as_bytes();

        txn.put(self.grant, &user_id, &new_bytes, WriteFlags::empty())?;

        txn.commit()?;

        Ok(Output::Success)
    }
}
