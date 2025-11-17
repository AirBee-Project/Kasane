use std::{collections::HashSet, sync::Arc};

use crate::{
    io::full::Storage,
    interface::{input::GrantKey, output::Output},
    user_error::UserError,
};

pub fn grant_key(v: GrantKey, s: Arc<Storage>) -> Result<Output, UserError> {
    let commands: HashSet<_> = v.command.into_iter().collect();
    s.grant_key(&v.user_name, &v.target_space, &v.target_key, commands)
}
