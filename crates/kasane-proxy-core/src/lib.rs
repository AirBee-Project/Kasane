//! Kasane プラットフォーム非依存プロキシコア
//!
//! Cloudflare Workers, Fastly Compute, AWS Lambda, コンテナ等の様々な環境で動作する、
//! 共通のプロキシリクエスト抽象化とルーティング・ゲートウェイ機能を提供する。

pub mod client;
pub mod gateway;
pub mod grpc_frame;
pub mod merger;
pub mod partition;
pub mod types;

pub use client::ProxyHttpClient;
pub use gateway::{
    ProxyConfig, QUERY_EXECUTE_PATH, handle_proxy_request, is_query_execute_request,
};
pub use grpc_frame::{
    GRPC_HEADER_SIZE, GrpcFrameError, decode_all_grpc_frames, decode_grpc_frame, encode_grpc_frame,
};
pub use merger::{StreamMerger, merge_query_streams};
pub use partition::{QueryPartitionError, SubQueryTask, split_query};
pub use types::{BoxStream, ProxyRequest, ProxyResponse, ProxyResponseBody};

/// gRPC-Web Explorer の組み込み静的 HTML
pub const EXPLORER_HTML: &str = include_str!("explorer.html");
