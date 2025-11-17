use redb::{ReadableDatabase, ReadableTable};

use crate::{
    io::full::{Storage, SPACE_TABLE, USER_TABLE},
    json::output::{Output, ShowSpaces, ShowUsers},
    user_error::UserError,
};

impl Storage {
    pub fn info_user(&self) -> Result<Output, UserError> {
        todo!()
    }
}
