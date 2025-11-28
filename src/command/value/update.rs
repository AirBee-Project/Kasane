use crate::io::io::Storage;

use crate::{
    interface::{input::UpdateValue, output::Output},
    user_error::UserError,
};
use std::sync::Arc;

pub fn update_value(v: UpdateValue, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
