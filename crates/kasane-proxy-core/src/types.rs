use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode, Uri};

/// プラットフォーム非依存のプロキシリクエスト表現
#[derive(Debug, Clone)]
pub struct ProxyRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl ProxyRequest {
    pub fn new(method: Method, uri: Uri, headers: HeaderMap, body: Bytes) -> Self {
        Self {
            method,
            uri,
            headers,
            body,
        }
    }

    /// リクエストのパス部分を取得
    pub fn path(&self) -> &str {
        self.uri.path()
    }

    /// パスとクエリ文字列を取得
    pub fn path_and_query(&self) -> &str {
        self.uri
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or_else(|| self.uri.path())
    }
}

/// プラットフォーム非依存のプロキシレスポンス表現
#[derive(Debug, Clone)]
pub struct ProxyResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl ProxyResponse {
    pub fn new(status: StatusCode, headers: HeaderMap, body: Bytes) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
}
