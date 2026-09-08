use crate::types::{ProxyRequest, ProxyResponse};
use async_trait::async_trait;

/// プラットフォーム固有の HTTP / gRPC 通信クライアントを抽象化するポート
///
/// Wasm (シングルスレッド / JsFuture) とネイティブの両方に対応するため `?Send` を指定。
#[async_trait(?Send)]
pub trait ProxyHttpClient: 'static {
    type Error: std::fmt::Display + 'static;

    /// 任意のリクエストを Origin Kasane へ透過転送する
    async fn forward(
        &self,
        origin_base_url: &str,
        request: ProxyRequest,
    ) -> Result<ProxyResponse, Self::Error>;
}
