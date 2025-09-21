use std::sync::Arc;

use crate::{
    command::tools::valid_len::valid_len,
    error::Error,
    io::{StorageTrait, full::Storage},
    json::{input::DropUser, output::Output},
};

pub fn drop_user(v: DropUser, s: Arc<Storage>) -> Result<Output, Error> {
    if valid_len(&v.user_name) {
        s.drop_user(&v.user_name)
    } else {
        return Err(Error::UserNotFound {
            user_name: v.user_name,
        });
    }
}
