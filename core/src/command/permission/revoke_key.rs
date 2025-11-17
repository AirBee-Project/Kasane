use std::{collections::HashSet, sync::Arc};

use crate::{
    io::full::Storage,
    json::{input::RevokeKey, output::Output},
    user_error::UserError,
};

pub fn revoke_key(v: RevokeKey, s: Arc<Storage>) -> Result<Output, UserError> {
    let commands: HashSet<_> = v.command.into_iter().collect();
    s.revoke_key(&v.user_name, commands)
}
