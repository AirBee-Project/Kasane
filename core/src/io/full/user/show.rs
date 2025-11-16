use redb::{ReadableDatabase, ReadableTable};

use crate::{
    io::full::{Storage, SPACE_TABLE},
    json::output::{Output, ShowSpaces},
    user_error::UserError,
};

impl Storage {
    pub fn show_users(&self) -> Result<Output, UserError> {
        todo!()
    }
}
