use std::{collections::HashSet, sync::Arc};

use crate::{
    io::full::Storage,
    json::{input::RevokeDatabase, output::Output},
    user_error::UserError,
};

pub fn revoke_database(v: RevokeDatabase, s: Arc<Storage>) -> Result<Output, UserError> {
    let commands: HashSet<_> = v.command.into_iter().collect();
    s.revoke_database(&v.user_name, commands)
}
