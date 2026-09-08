use async_trait::async_trait;
use bytes::Bytes;
use http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use kasane_proxy_core::{ProxyHttpClient, ProxyRequest, ProxyResponse};
use worker::{Fetch, Headers as CfHeaders, Method as CfMethod, Request as CfRequest, RequestInit};

/// Cloudflare Workers の Fetch API を使用した ProxyHttpClient 実装
pub struct CfWorkerHttpClient;

impl CfWorkerHttpClient {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CfWorkerHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl ProxyHttpClient for CfWorkerHttpClient {
    type Error = worker::Error;

    async fn forward(
        &self,
        origin_base_url: &str,
        request: ProxyRequest,
    ) -> Result<ProxyResponse, Self::Error> {
        let target_url = format!("{}{}", origin_base_url, request.path_and_query());

        // 1. CF Method の変換
        let cf_method = match request.method.as_str() {
            "GET" => CfMethod::Get,
            "POST" => CfMethod::Post,
            "PUT" => CfMethod::Put,
            "DELETE" => CfMethod::Delete,
            "HEAD" => CfMethod::Head,
            "OPTIONS" => CfMethod::Options,
            "PATCH" => CfMethod::Patch,
            _ => CfMethod::Post,
        };

        // 2. CF Headers の構築
        let cf_headers = CfHeaders::new();
        for (name, value) in request.headers.iter() {
            if let Ok(val_str) = value.to_str() {
                // Host ヘッダ等の転送先不整合を防ぐため、Host は除外（Fetch が自動設定）
                if name.as_str().eq_ignore_ascii_case("host") {
                    continue;
                }
                cf_headers.set(name.as_str(), val_str)?;
            }
        }

        // 3. RequestInit の構成
        let mut init = RequestInit::new();
        init.with_method(cf_method);
        init.with_headers(cf_headers);

        if !request.body.is_empty() {
            init.with_body(Some(request.body.to_vec().into()));
        }

        let cf_req = CfRequest::new_with_init(&target_url, &init)?;

        // 4. Origin へリクエスト送信
        let mut cf_res = Fetch::Request(cf_req).send().await?;

        // 5. レスポンスの変換
        let status =
            StatusCode::from_u16(cf_res.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        let mut res_headers = HeaderMap::new();
        for (k, v) in cf_res.headers() {
            if let (Ok(name), Ok(val)) = (
                HeaderName::from_bytes(k.as_bytes()),
                HeaderValue::from_str(&v),
            ) {
                res_headers.insert(name, val);
            }
        }

        let body_bytes = cf_res.bytes().await?;
        let body = Bytes::from(body_bytes);

        Ok(ProxyResponse::new(status, res_headers, body))
    }
}
