use std::{collections::HashSet, sync::Arc};

use crate::{
    io::full::Storage,
    interface::{input::RevokeKey, output::Output},
    user_error::UserError,
};

pub fn revoke_key(v: RevokeKey, s: Arc<Storage>) -> Result<Output, UserError> {
    let commands: HashSet<_> = v.command.into_iter().collect();
    s.revoke_key(&v.user_name, &v.target_space, &v.target_key, commands)
}
