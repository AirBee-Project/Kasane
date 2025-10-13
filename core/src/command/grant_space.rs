use std::{collections::HashSet, sync::Arc};

use crate::{
    command::tools::valid_name::valid_name,
    io::{StorageTrait, full::Storage},
    json::{
        input::{AllOrChoose, CommandDatabase, GrantSpace},
        output::Output,
    },
    user_error::UserError,
};

use crate::json::input::AllOrChoose::{All, Choose};

pub fn grant_space(v: GrantSpace, s: Arc<Storage>) -> Result<Output, UserError> {
    todo!()
}
