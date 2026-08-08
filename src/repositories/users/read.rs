use crate::{
    error::AppError,
    models::users::{User, UserMetadata},
    repositories::{KasaneDbRead, meta::MetaRead},
};

impl<'a> KasaneDbRead<'a> {
    #[tracing::instrument(skip_all, fields(username = %username))]
    pub fn get_user(&self, username: &str) -> Result<Option<User>, AppError> {
        Ok(self
            .user_meta(username)?
            .map(|meta| User::from_meta(username, meta)))
    }

    #[tracing::instrument(skip_all, fields(username = %username))]
    pub fn require_user(&self, username: &str) -> Result<User, AppError> {
        Ok(User::from_meta(username, self.require_user_meta(username)?))
    }

    #[tracing::instrument(skip_all)]
    pub fn get_all_users(&self) -> Result<Vec<User>, AppError> {
        let mut users = Vec::new();
        for item in self.db.users.iter(&self.read_txn)? {
            let (username, val) = item?;
            let meta: UserMetadata = serde_json::from_str(val)
                .map_err(|_| AppError::InternalError("Failed to parse user metadata".into()))?;
            users.push(User::from_meta(username, meta));
        }
        Ok(users)
    }
}
