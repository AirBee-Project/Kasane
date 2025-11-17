use std::{collections::HashSet, sync::Arc};

use crate::{
    io::full::Storage,
    interface::{input::GrantDatabase, output::Output},
    user_error::UserError,
};

pub fn grant_database(v: GrantDatabase, s: Arc<Storage>) -> Result<Output, UserError> {
    let commands: HashSet<_> = v.command.into_iter().collect();
    s.grant_database(&v.user_name, commands)
}
