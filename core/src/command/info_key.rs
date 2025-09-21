use std::sync::Arc;

use crate::{
    command::tools::valid_len::valid_len,
    error::Error,
    io::{StorageTrait, full::Storage},
    json::{
        input::{InfoKey, InfoSpace, InfoUser},
        output::Output,
    },
};

pub fn info_key(v: InfoKey, s: Arc<Storage>) -> Result<Output, Error> {
    if valid_len(&v.space_name) {
        if valid_len(&v.key_name) {
            s.info_key(&v.space_name, &v.key_name)
        } else {
            return Err(Error::KeyNotFound {
                key_name: v.key_name,
                space_name: v.space_name,
                location: "command::info_key",
            });
        }
    } else {
        return Err(Error::SpaceNotFound {
            space_name: v.space_name,
        });
    }
}
