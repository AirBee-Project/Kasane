use std::sync::Arc;

use crate::{
    command::tools::valid_len::valid_len,
    error::Error,
    io::{StorageTrait, full::Storage},
    json::{input::InfoUser, output::Output},
};

pub fn info_user(v: InfoUser, s: Arc<Storage>) -> Result<Output, Error> {
    if valid_len(&v.user_name) {
        s.info_user(&v.user_name)
    } else {
        return Err(Error::UserNotFound {
            user_name: v.user_name,
        });
    }
}
