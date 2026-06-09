use std::collections::HashMap;

use crate::models::users::User;

#[derive(Debug, Default)]
pub struct AuthCache {
    pub users: HashMap<String, User>,
}

impl AuthCache {
    pub fn new() -> Self {
        Self::default()
    }
}
