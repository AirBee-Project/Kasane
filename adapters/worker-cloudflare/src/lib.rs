use bytes::Bytes;
use http::{HeaderMap, HeaderName, HeaderValue, Method, Uri};
use kasane_proxy_core::{ProxyConfig, ProxyRequest, ProxyResponse, handle_proxy_request};
use worker::{
    Context, Env, Headers as CfHeaders, Request as CfRequest, Response as CfResponse,
    Result as CfResult, event,
};

pub mod client;

pub use client::CfWorkerHttpClient;

#[event(fetch)]
pub async fn main(mut req: CfRequest, env: Env, _ctx: Context) -> CfResult<CfResponse> {
    // 1. 環境変数から Origin Kasane の URL を取得
    let origin_url = env
        .var("KASANE_ORIGIN_URL")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "http://127.0.0.1:5172".to_string());

    let config = ProxyConfig::new(origin_url);
    let client = CfWorkerHttpClient::new();

    // 2. CF Request -> ProxyRequest 変換
    let proxy_req = to_proxy_request(&mut req).await?;

    // 3. 共通プロキシゲートウェイを実行
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

    let cf_res = CfResponse::from_bytes(res.body.to_vec())?
        .with_status(res.status.as_u16())
        .with_headers(cf_headers);

    Ok(cf_res)
}
