use bytes::Bytes;
use futures::StreamExt;
use http::{HeaderMap, HeaderName, HeaderValue, Method, Uri};
use kasane_proxy_core::{
    ProxyConfig, ProxyRequest, ProxyResponse, ProxyResponseBody, handle_proxy_request,
};
use worker::{
    Context, Env, Headers as CfHeaders, Request as CfRequest, Response as CfResponse,
    Result as CfResult, event,
};

pub mod client;

pub use client::CfWorkerHttpClient;

#[event(fetch)]
pub async fn main(mut req: CfRequest, env: Env, _ctx: Context) -> CfResult<CfResponse> {
    // Web Explorer の静的 HTML 配信 (GET /, /explorer, /docs)
    if req.method() == worker::Method::Get {
        let path = req.path();
        if path == "/" || path == "/explorer" || path == "/docs" {
            return CfResponse::from_html(kasane_proxy_core::EXPLORER_HTML);
        }
    }

    // 1. 環境変数から Origin Kasane の URL を取得
    let origin_url = env
        .var("KASANE_ORIGIN_URL")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "http://127.0.0.1:5172".to_string());

    // 2. 環境変数から Query Worker の URL リストを取得 (カンマ区切り)
    let query_worker_urls: Vec<String> = env
        .var("KASANE_QUERY_WORKER_URLS")
        .map(|v| {
            v.to_string()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let config = ProxyConfig::new(origin_url).with_query_worker_urls(query_worker_urls);
    let client = CfWorkerHttpClient::new();

    // 3. CF Request -> ProxyRequest 変換
    let proxy_req = to_proxy_request(&mut req).await?;

    // 4. 共通プロキシゲートウェイを実行
    match handle_proxy_request(&client, &config, proxy_req).await {
        Ok(proxy_res) => to_cf_response(proxy_res),
        Err(e) => {
            worker::console_error!("Proxy error: {e}");
            CfResponse::error(format!("Kasane Proxy Error: {e}"), 502)
        }
    }
}

/// worker::Request を kasane_proxy_core::ProxyRequest に変換
async fn to_proxy_request(req: &mut CfRequest) -> CfResult<ProxyRequest> {
    let method = match req.method() {
        worker::Method::Get => Method::GET,
        worker::Method::Post => Method::POST,
        worker::Method::Put => Method::PUT,
        worker::Method::Delete => Method::DELETE,
        worker::Method::Head => Method::HEAD,
        worker::Method::Options => Method::OPTIONS,
        worker::Method::Patch => Method::PATCH,
        _ => Method::POST,
    };

    let url_str = req.url()?.to_string();
    let uri = url_str
        .parse::<Uri>()
        .map_err(|e| worker::Error::RustError(format!("Invalid URI: {e}")))?;

    let mut headers = HeaderMap::new();
    for (k, v) in req.headers() {
        if let (Ok(name), Ok(val)) = (
            HeaderName::from_bytes(k.as_bytes()),
            HeaderValue::from_str(&v),
        ) {
            headers.insert(name, val);
        }
    }

    let body_vec = req.bytes().await?;
    let body = Bytes::from(body_vec);

    Ok(ProxyRequest::new(method, uri, headers, body))
}

/// kasane_proxy_core::ProxyResponse を worker::Response に変換
fn to_cf_response(res: ProxyResponse) -> CfResult<CfResponse> {
    let cf_headers = CfHeaders::new();
    for (name, val) in res.headers.iter() {
        if let Ok(val_str) = val.to_str() {
            cf_headers.set(name.as_str(), val_str)?;
        }
    }

    let status = res.status.as_u16();

    let cf_res = match res.body {
        ProxyResponseBody::Bytes(b) => {
            CfResponse::from_bytes(b.to_vec())?
                .with_status(status)
                .with_headers(cf_headers)
        }
        ProxyResponseBody::Stream(stream) => {
            // worker::Response::from_stream は Stream<Item = std::result::Result<Bytes/Vec<u8>, E>> を受け取る
            let byte_stream = stream.map(|chunk_res| {
                chunk_res
                    .map(|b| b.to_vec())
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            });
            CfResponse::from_stream(byte_stream)?
                .with_status(status)
                .with_headers(cf_headers)
        }
    };

    Ok(cf_res)
}
