use std::sync::Arc;

use crate::{
    io::full::Storage,
    interface::{
        input::{InsertValue, UpdateValue},
        output::Output,
    },
    user_error::UserError,
};

pub fn update_value(v: UpdateValue, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
