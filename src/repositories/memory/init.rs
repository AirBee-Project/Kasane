//! 起動時に一度だけ行う設定をここに集める。

use std::collections::BTreeMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::error::AppError;
use crate::models::id::PrincipalId;
use crate::models::users::{UserRecord, UserRole};
use crate::repositories::{ROOT_USERNAME, root_password};
use crate::services::auth::hash_password;

use super::{MemoryDb, State};

impl MemoryDb {
    /// root だけを持つ空のストレージを作る。
    pub fn new() -> Result<Self, AppError> {
        let root = UserRecord {
            id: PrincipalId(Uuid::now_v7()),
            password_hash: hash_password(&root_password())?,
            token_version: 0,
            global_role: Some(UserRole::Admin),
        };
        let state = State {
            users: BTreeMap::from([(ROOT_USERNAME.to_string(), root)]),
            ..State::default()
        };
        Ok(Self(Arc::new(tokio::sync::Mutex::new(Arc::new(state)))))
    }
}
