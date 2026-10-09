pub mod authorize;
pub mod data_response;
pub mod name_valid;
pub mod spatial_ids;
pub mod value;

use crate::error::AppError;

/// CPU を使う同期処理 `f` を、async ワーカーを塞がない場所で実行して結果を返す。
pub async fn run_blocking<T, F>(f: F) -> Result<T, AppError>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    // ブラウザの Wasm はスレッドを作れないので、その場で実行する。
    #[cfg(target_arch = "wasm32")]
    return Ok(f());

    #[cfg(not(target_arch = "wasm32"))]
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))
}
