use std::{collections::HashSet, sync::Arc};

use crate::{
    io::full::Storage,
    json::{input::GrantKey, output::Output},
    user_error::UserError,
};

pub fn grant_key(v: GrantKey, s: Arc<Storage>) -> Result<Output, UserError> {
    let commands: HashSet<_> = v.command.into_iter().collect();
    s.grant_key(&v.user_name, commands)
}
