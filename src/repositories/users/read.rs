use crate::{
    error::AppError,
    models::users::{User, UserMetadata},
    repositories::{KasaneDbRead, MetaRepository},
};

impl<'a> KasaneDbRead<'a> {
    #[tracing::instrument(skip_all, fields(username = %username))]
    pub async fn get_user_impl(&self, username: &str) -> Result<Option<User>, AppError> {
        Ok(MetaRepository::user_meta(self, username)
            .await?
            .map(|meta| User::from_meta(username, meta)))
    }

    #[tracing::instrument(skip_all, fields(username = %username))]
    pub async fn require_user_impl(&self, username: &str) -> Result<User, AppError> {
        Ok(User::from_meta(
            username,
            self.require_user_meta(username).await?,
        ))
    }

    #[tracing::instrument(skip_all)]
    pub fn get_all_users_impl(&self) -> Result<Vec<User>, AppError> {
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
