pub mod permissions;
pub use permissions::*;

#[cfg(feature = "server")]
mod credentials;
#[cfg(feature = "server")]
pub use credentials::*;

use crate::Kasane;
use crate::error::AppError;
use crate::models::users::User;
use crate::repositories::{ReadRepository, Storage};

impl Kasane {
    /// 認可に使う利用者を取り出す。プロセス内の呼び出し側は信頼できる前提なので、
    /// パスワードもトークンも確かめない。
    pub async fn user(&self, username: &str) -> Result<User, AppError> {
        let username = username.to_string();
        self.db
            .read(async move |r| r.require_user(&username).await)
            .await
    }
}
