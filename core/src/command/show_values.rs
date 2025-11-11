use std::sync::Arc;

use crate::{
    io::full::Storage,
    json::{
        input::ShowValues,
        output::{Output, Value},
    },
    user_error::UserError,
};

pub fn show_values(v: ShowValues, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
