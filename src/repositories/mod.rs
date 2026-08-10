//! ストレージ層。
//!
//! [`storage`] がバックエンド非依存の抽象 API（trait 群）を定義し、その下に
//! バックエンドごとの実装が並ぶ。実装は Cargo feature で排他選択され、
//! 選ばれなかった側はコンパイルもされない。

pub mod encoding;
pub mod storage;

pub use storage::{MetaRepository, ReadRepository, Storage, ValueGroups, WriteRepository};

// --- LMDB バックエンド ---

#[cfg(feature = "backend-lmdb")]
pub mod database;
#[cfg(feature = "backend-lmdb")]
pub mod lmdb;
#[cfg(feature = "backend-lmdb")]
pub mod meta;
#[cfg(feature = "backend-lmdb")]
pub mod users;

#[cfg(feature = "backend-lmdb")]
pub use lmdb::{KasaneDbRead, KasaneDbWrite};
#[cfg(feature = "backend-lmdb")]
pub use meta::MetaRead;

// --- TiKV バックエンド ---

#[cfg(feature = "backend-tikv")]
pub mod tikv;
