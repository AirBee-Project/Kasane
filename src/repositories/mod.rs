//! ストレージ層。
//!
//! [`traits`] がバックエンド非依存の抽象 API を定義し、その下に
//! バックエンドごとの実装が同じ構成で並ぶ。実装は Cargo feature で排他選択され、
//! 選ばれなかった側はコンパイルもされない。
//!
//! ```text
//! traits/      抽象 API（storage / read / write / catalog の 4 つの役割）
//! encoding/    両バックエンドで共通のバイト表現
//! lmdb/        LMDB 実装      ┐ 同じファイル構成
//! tikv/        TiKV 実装      ┘（mod / init / keys / catalog / data / users / query_source / repository）
//! ```

pub mod encoding;
pub mod traits;

pub use traits::{
    CatalogRepository, ReadRepository, ResolvedTable, Storage, ValueGroups, WriteRepository,
};

#[cfg(feature = "backend-lmdb")]
pub mod lmdb;
#[cfg(feature = "backend-tikv")]
pub mod tikv;
