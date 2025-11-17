use std::{collections::HashSet, sync::Arc};

use crate::{
    io::full::Storage,
    json::{input::GrantUser, output::Output},
    user_error::UserError,
};

pub fn grant_user(v: GrantUser, s: Arc<Storage>) -> Result<Output, UserError> {
    let commands: HashSet<_> = v.command.into_iter().collect();
    s.grant_user(&v.user_name, commands)
}
