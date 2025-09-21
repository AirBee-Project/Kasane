use std::sync::Arc;

use crate::io::{StorageTrait, full::Storage};
use crate::json::input::CreateKey;
use crate::json::output::Output;
use crate::{command::tools::valid_name::valid_name, error::Error};

pub fn create_key(v: CreateKey, s: Arc<Storage>) -> Result<Output, Error> {
    if !valid_name(&v.key_name) {
        Err(Error::KeyNameValidationError {
            name: v.key_name,
            reason: "only a-z, A-Z, 0-9, - _ . @ + = allowed, max 256 characters",
            location: "command::addkey::addkey",
        })
    } else {
        s.create_key(&v.space_name, &v.key_name, v.key_type, v.key_mode)
    }
}
