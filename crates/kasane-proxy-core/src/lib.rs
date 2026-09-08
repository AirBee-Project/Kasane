//! Kasane プラットフォーム非依存プロキシコア
//!
//! Cloudflare Workers, Fastly Compute, AWS Lambda, コンテナ等の様々な環境で動作する、
//! 共通のプロキシリクエスト抽象化とルーティング・ゲートウェイ機能を提供する。

pub mod client;
pub mod gateway;
pub mod types;

pub use client::ProxyHttpClient;
pub use gateway::{
    ProxyConfig, QUERY_EXECUTE_PATH, handle_proxy_request, is_query_execute_request,
};
pub use types::{ProxyRequest, ProxyResponse};
