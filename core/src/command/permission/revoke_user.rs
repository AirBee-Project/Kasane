use std::{collections::HashSet, sync::Arc};

use crate::{
    io::full::Storage,
    interface::{input::RevokeUser, output::Output},
    user_error::UserError,
};

pub fn revoke_user(v: RevokeUser, s: Arc<Storage>) -> Result<Output, UserError> {
    let commands: HashSet<_> = v.command.into_iter().collect();
    s.revoke_user(&v.user_name, commands)
}
