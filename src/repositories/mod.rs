//! ストレージ層。バックエンドは Cargo feature で排他選択され、選ばれなかった側は
//! コンパイルもされない。

pub mod encoding;
pub mod traits;

pub use traits::{
    CatalogRepository, ReadRepository, ResolvedTable, Storage, ValueGroups, WriteRepository,
};

#[cfg(feature = "backend-lmdb")]
pub mod lmdb;
#[cfg(feature = "backend-tikv")]
pub mod tikv;
