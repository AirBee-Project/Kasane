use crate::client::ProxyHttpClient;
use crate::types::{ProxyRequest, ProxyResponse};

/// クエリ実行の gRPC パス定数
pub const QUERY_EXECUTE_PATH: &str = "/kasane.QueryService/Execute";

/// プロキシゲートウェイの設定
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// データ元となる Origin Kasane のベース URL (例: "http://127.0.0.1:5172")
    pub origin_url: String,
}

impl ProxyConfig {
    pub fn new(origin_url: impl Into<String>) -> Self {
        let mut origin_url = origin_url.into();
        // 末尾のスラッシュを削除して正規化
        while origin_url.ends_with('/') {
            origin_url.pop();
        }
        Self { origin_url }
    }
}

/// 全プラットフォーム共通のプロキシエントリーポイント
///
/// すべての着信リクエスト（CRUD, 認証, カタログ等）を受け付け、適切な処理先へ振り分ける。
/// 現フェーズ（アダプタ抽象化）では全リクエストを透過フォワードし、
/// 将来のフェーズで `QUERY_EXECUTE_PATH` に対するインターセプト計算をシームレスに差し込める構造にしている。
pub async fn handle_proxy_request<C: ProxyHttpClient>(
    client: &C,
    config: &ProxyConfig,
    request: ProxyRequest,
) -> Result<ProxyResponse, C::Error> {
    if is_query_execute_request(&request) {
        // [Query Intercept Point]
        // 次フェーズでここで Worker 内部計算（またはクエリ分割・ファンアウト）を呼び出す。
        // 現フェーズでは透過フォワードして Origin Kasane に委任。
        tracing::debug!("Intercepted query execute request, forwarding to origin");
        client.forward(&config.origin_url, request).await
    } else {
        // [Transparent Forwarding]
        // CRUD, 認証, メタデータ, システム等の全リクエストを透過フォワード
        tracing::debug!(
            path = request.path(),
            "Transparently forwarding request to origin"
        );
        client.forward(&config.origin_url, request).await
    }
}

/// クエリ実行リクエストかどうかを判定する
pub fn is_query_execute_request(request: &ProxyRequest) -> bool {
    request.path() == QUERY_EXECUTE_PATH
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use http::{HeaderMap, Method, StatusCode, Uri};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    struct MockHttpClient {
        forward_call_count: Arc<AtomicUsize>,
        last_url: Arc<Mutex<Option<String>>>,
    }

    #[async_trait::async_trait(?Send)]
    impl ProxyHttpClient for MockHttpClient {
        type Error = String;

        async fn forward(
            &self,
            origin_base_url: &str,
            request: ProxyRequest,
        ) -> Result<ProxyResponse, Self::Error> {
            self.forward_call_count.fetch_add(1, Ordering::SeqCst);
            *self.last_url.lock().unwrap() =
                Some(format!("{}{}", origin_base_url, request.path_and_query()));
            Ok(ProxyResponse::new(
                StatusCode::OK,
                HeaderMap::new(),
                Bytes::from_static(b"mock response"),
            ))
        }
    }

    #[tokio::test]
    async fn test_handle_transparent_forwarding() {
        let forward_call_count = Arc::new(AtomicUsize::new(0));
        let last_url = Arc::new(Mutex::new(None));
        let client = MockHttpClient {
            forward_call_count: forward_call_count.clone(),
            last_url: last_url.clone(),
        };

        let config = ProxyConfig::new("http://origin-kasane:5172/");
        let request = ProxyRequest::new(
            Method::POST,
            Uri::from_static("/kasane.TableService/Create"),
            HeaderMap::new(),
            Bytes::from_static(b"table create payload"),
        );

        let response = handle_proxy_request(&client, &config, request)
            .await
            .unwrap();

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.body, Bytes::from_static(b"mock response"));
        assert_eq!(forward_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            last_url.lock().unwrap().as_deref(),
            Some("http://origin-kasane:5172/kasane.TableService/Create")
        );
    }

    #[tokio::test]
    async fn test_handle_query_execute_path() {
        let forward_call_count = Arc::new(AtomicUsize::new(0));
        let last_url = Arc::new(Mutex::new(None));
        let client = MockHttpClient {
            forward_call_count: forward_call_count.clone(),
            last_url: last_url.clone(),
        };

        let config = ProxyConfig::new("http://origin-kasane:5172");
        let request = ProxyRequest::new(
            Method::POST,
            Uri::from_static("/kasane.QueryService/Execute"),
            HeaderMap::new(),
            Bytes::from_static(b"query payload"),
        );

        let response = handle_proxy_request(&client, &config, request)
            .await
            .unwrap();

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(forward_call_count.load(Ordering::SeqCst), 1);
        assert_eq!(
            last_url.lock().unwrap().as_deref(),
            Some("http://origin-kasane:5172/kasane.QueryService/Execute")
        );
    }
}
