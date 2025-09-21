use std::sync::Arc;

use crate::{
    command::tools::valid_name::valid_name,
    error::Error,
    io::{StorageTrait, full::Storage},
    json::{input::CreateUser, output::Output},
};

pub fn create_user(v: CreateUser, s: Arc<Storage>) -> Result<Output, Error> {
    if !valid_name(&v.user_name) {
        Err(Error::SpaceNameValidationError {
            name: v.user_name,
            reason: "only a-z, A-Z, 0-9, - _ . @ allowed, max 256 characters",
            location: "command::addspace::addspace",
        })
    } else {
        s.create_user(&v.user_name, &v.password)
    }
}
