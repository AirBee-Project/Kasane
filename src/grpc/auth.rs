//! 各 RPC ハンドラーが呼び出し元の利用者を得る入口。

#[cfg(feature = "server")]
mod token;
#[cfg(feature = "server")]
pub use token::*;

/// インメモリでは利用者は root だけなので、トークンを見ずに root として通す。
#[cfg(not(feature = "server"))]
pub async fn authenticate<T>(
    app_state: &crate::AppState,
    _request: &tonic::Request<T>,
) -> Result<crate::services::auth::AuthUser, tonic::Status> {
    use crate::repositories::{ReadRepository, Storage};

    let user = app_state
        .db
        .read(async move |repo| repo.require_user(crate::repositories::ROOT_USERNAME).await)
        .await?;
    Ok(crate::services::auth::AuthUser { user })
}
