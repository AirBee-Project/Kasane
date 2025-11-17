use std::{collections::HashSet, sync::Arc};

use crate::{
    io::full::Storage,
    json::{input::GrantSpace, output::Output},
    user_error::UserError,
};

pub fn grant_space(v: GrantSpace, s: Arc<Storage>) -> Result<Output, UserError> {
    let commands: HashSet<_> = v.command.into_iter().collect();
    s.grant_space(&v.user_name, commands)
}
