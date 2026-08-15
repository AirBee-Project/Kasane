//! `url.scheme` スパン属性を実態に合わせる。
//!
//! HTTP/1.1 の受信リクエストは origin-form（パスのみ）なので、`req.uri()` は
//! スキームを持たない。何もしないと `axum_tracing_opentelemetry` が付ける
//! `url.scheme` は常に空文字になる。
//!
//! Kasane 自身は TLS を終端しない（`main.rs` は常に平文の `TcpListener` で listen する）。
//! そのため実スキームの手がかりは、TLS 終端プロキシが付ける `X-Forwarded-Proto` しかない。
//! これを見て URI を補完する。

use axum::{
    extract::Request,
    http::{Uri, header::HOST, uri::Scheme},
    middleware::Next,
    response::Response,
};

/// **`OtelAxumLayer` より外側（先に実行される側）に挟むこと。** 内側だとスパン生成後の
/// 補完になり、属性へ反映されない。
pub async fn normalize(mut req: Request, next: Next) -> Response {
    let forwarded = req
        .headers()
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        // マルチホップのプロキシは先頭が最も外側（クライアントに近い側）。
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .and_then(|v| Scheme::try_from(v).ok());

    let scheme = forwarded.unwrap_or(Scheme::HTTP);

    if let Some(path_and_query) = req.uri().path_and_query().cloned() {
        let mut parts = req.uri().clone().into_parts();
        parts.scheme = Some(scheme);
        // `Uri` はスキームがあれば authority も要求する。無ければ `Host` ヘッダで補う。
        if parts.authority.is_none() {
            parts.authority = req
                .headers()
                .get(HOST)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok());
        }
        parts.path_and_query = Some(path_and_query);
        // `Host` も無い（HTTP/1.0 相当）なら諦めて元の URI のまま進む。
        if let Ok(uri) = Uri::from_parts(parts) {
            *req.uri_mut() = uri;
        }
    }

    next.run(req).await
}
