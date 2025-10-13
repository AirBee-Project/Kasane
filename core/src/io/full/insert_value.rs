use actix_web::http::Error;
use lmdb::{Cursor, DatabaseFlags, Transaction as _, WriteFlags};
use uuid::Uuid;

use crate::{
    io::{
        StorageTrait,
        full::{Storage, tools::value_entry::ValueEntry},
    },
    json::output::Output,
    r#type::spacetimeid::SpaceTimeId,
    user_error::UserError,
};

impl StorageTrait for Storage {
    fn insert_value(
        &self,
        space_name: &str,
        key_name: &str,
        ids: Vec<SpaceTimeId>,
        value: ValueEntry,
    ) -> Result<Output, UserError> {
        let space_id = Self::get_space_id(&self, space_name)?;
        let key_id = Self::get_key_id(&self, &space_id, key_name)?;

        //入力されたValueEntryの型が正しいか確認する

        //KeyModeを確認する

        //ValueTableに仮で突っ込む

        for id in ids {
            //まずxにおけるstartとendを作る
        }
    }
}
