//! ストリーミングレスポンス組み立ての共通部分。中身の生成は `arrow`/`stream_response` 側が担う。

use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use bytes::Bytes;

/// チャンネルから届くバイト列をそのまま `Body` へ流し込む。JSON/Arrow どちらのストリームも
/// 中身（チャンクの作り方）だけが違い、レスポンスの組み立ては共通なのでここに集約する。
pub(crate) fn build_stream_response(
    rx: tokio::sync::mpsc::Receiver<Result<Bytes, String>>,
    content_type: &'static str,
) -> Response {
    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .body(axum::body::Body::from_stream(stream))
        .unwrap()
        .into_response()
}
