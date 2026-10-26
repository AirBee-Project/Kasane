use uuid::Uuid;

use crate::models::id::PrincipalId;
use crate::models::users::{UserRecord, UserRole};
use crate::repositories::ROOT_USERNAME;

pub(super) fn user_record(username: &str) -> Option<UserRecord> {
    (username == ROOT_USERNAME).then(|| UserRecord {
        id: PrincipalId(Uuid::nil()),
        password_hash: String::new(),
        token_version: 0,
        global_role: Some(UserRole::Admin),
    })
}

pub(super) fn list_users(after: Option<&str>, limit: usize) -> Vec<(String, UserRecord)> {
    let root = after.is_none_or(|after| after < ROOT_USERNAME) && limit > 0;
    root.then(|| {
        (
            ROOT_USERNAME.to_string(),
            user_record(ROOT_USERNAME).unwrap(),
        )
    })
    .into_iter()
    .collect()
}
