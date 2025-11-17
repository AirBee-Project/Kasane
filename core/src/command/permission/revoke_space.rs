use std::{collections::HashSet, sync::Arc};

use crate::{
    io::full::Storage,
    json::{input::RevokeSpace, output::Output},
    user_error::UserError,
};

pub fn revoke_space(v: RevokeSpace, s: Arc<Storage>) -> Result<Output, UserError> {
    let commands: HashSet<_> = v.command.into_iter().collect();
    s.revoke_space(&v.user_name, &v.target_space, commands)
}
