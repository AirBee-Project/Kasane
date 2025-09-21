use std::sync::Arc;

use crate::{
    command::tools::valid_len::valid_len,
    error::Error,
    io::{StorageTrait, full::Storage},
    json::{
        input::{InfoSpace, InfoUser},
        output::Output,
    },
};

pub fn info_space(v: InfoSpace, s: Arc<Storage>) -> Result<Output, Error> {
    if valid_len(&v.space_name) {
        s.info_space(&v.space_name)
    } else {
        return Err(Error::SpaceNotFound {
            space_name: v.space_name,
        });
    }
}
