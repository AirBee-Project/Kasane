//! LMDB バックエンド向けの結合テスト。TiKV バックエンドのビルドでは対象外。
#![cfg(feature = "backend-lmdb")]

pub mod database;
pub mod query;
