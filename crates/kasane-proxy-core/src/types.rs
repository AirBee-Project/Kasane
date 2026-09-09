use bytes::Bytes;
use futures::Stream;
use http::{HeaderMap, Method, StatusCode, Uri};
use std::pin::Pin;

/// プラットフォーム非依存の非同期ストリーム型 (?Send / Wasm 互換)
pub type BoxStream<T> = Pin<Box<dyn Stream<Item = T> + 'static>>;

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

/// プロキシレスポンスのボディ表現（一括バッファまたはストリーム）
pub enum ProxyResponseBody {
    Bytes(Bytes),
    Stream(BoxStream<Result<Bytes, String>>),
}

impl std::fmt::Debug for ProxyResponseBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bytes(b) => write!(f, "ProxyResponseBody::Bytes(len={})", b.len()),
            Self::Stream(_) => write!(f, "ProxyResponseBody::Stream(..)"),
        }
    }
}

/// プラットフォーム非依存のプロキシレスポンス表現
pub struct ProxyResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: ProxyResponseBody,
}

impl ProxyResponse {
    pub fn new(status: StatusCode, headers: HeaderMap, body: Bytes) -> Self {
        Self {
            status,
            headers,
            body: ProxyResponseBody::Bytes(body),
        }
    }

    pub fn from_bytes(status: StatusCode, headers: HeaderMap, body: Bytes) -> Self {
        Self::new(status, headers, body)
    }

    pub fn from_stream(
        status: StatusCode,
        headers: HeaderMap,
        stream: BoxStream<Result<Bytes, String>>,
    ) -> Self {
        Self {
            status,
            headers,
            body: ProxyResponseBody::Stream(stream),
        }
    }

    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    /// レスポンスボディが一括バイト列の場合はその参照を取得
    pub fn as_bytes(&self) -> Option<&Bytes> {
        match &self.body {
            ProxyResponseBody::Bytes(b) => Some(b),
            ProxyResponseBody::Stream(_) => None,
        }
    }
}
