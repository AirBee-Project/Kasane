//! HTTP API の結合テスト（LMDB・メモリバックエンド）。TiKV のビルドでは対象外。
//!
//! `tests/` 直下の `.rs` は 1 つずつ独立したテストバイナリになる。このファイルは
//! そのうち「HTTP を叩いて API 全体を確かめる」ものの入口で、実体は
//! `database/` と `query/` に分かれている。共有のヘルパは `common/` にある。
#![cfg(all(feature = "server", not(feature = "backend-tikv")))]

mod common;
mod database;
mod query;
