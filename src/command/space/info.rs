use std::sync::Arc;

use crate::io::io::Storage;

use crate::{
    command::tools::valid_name::valid_name,
    interface::{input::InfoSpace, output::Output},
    location,
    user_error::UserError,
};

pub fn info_space(v: InfoSpace, s: Arc<Storage>) -> Result<Output, UserError> {
    s.info_space(&v.space_name)
}
