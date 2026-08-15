//! HTTP リクエストの所要時間を計測へ流す。
//!
//! スパンにも同じ情報は載るが、サンプリングを入れると計器としては不正確になる。
//! レートやレイテンシ分布はここで別途持つ。

use axum::{extract::MatchedPath, extract::Request, middleware::Next, response::Response};
use std::time::Instant;

pub async fn record(req: Request, next: Next) -> Response {
    let method = req.method().as_str().to_string();
    // ルートのテンプレートを使う（生パスだとカーディネリティが利用者数に比例する）。
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| req.uri().path().to_string());

    let start = Instant::now();
    let response = next.run(req).await;

    crate::telemetry::metrics::http_request(
        &method,
        route,
        response.status().as_u16(),
        start.elapsed().as_secs_f64(),
    );
    response
}
