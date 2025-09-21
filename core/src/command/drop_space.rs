use std::sync::Arc;

use crate::{
    error::Error,
    io::{StorageTrait, full::Storage},
    json::{input::DropSpace, output::Output},
};

pub fn drop_space(v: DropSpace, s: Arc<Storage>) -> Result<Output, Error> {
    s.drop_space(&v.space_name)
}
