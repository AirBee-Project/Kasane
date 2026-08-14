//! ストレージ層。バックエンドは Cargo feature で排他選択され、選ばれなかった側は
//! コンパイルもされない。

pub mod encoding;
pub mod traits;

pub use traits::{
    CatalogRepository, ReadRepository, ResolvedTable, Storage, ValueGroups, WriteRepository,
};

/// ディスク形式の版。**互換性を壊す変更を入れたら必ず上げること。**
///
/// バックエンドは選択制だが形式の世代は 1 つで数えるので、定義もここに 1 つだけ置く。
/// 版 1 は権限を利用者レコードの配列で持っていた。版 2 で対象ごとの ACL 行へ分けた。
pub const SCHEMA_VERSION: u32 = 2;

/// 既定の管理者。削除・権限変更の対象にできない唯一の利用者。
pub const ROOT_USERNAME: &str = "root";

/// `ROOT_PASSWORD` 未設定時の既定パスワード。
fn root_password() -> String {
    std::env::var("ROOT_PASSWORD")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "password".to_string())
}

#[cfg(feature = "backend-lmdb")]
pub mod lmdb;
#[cfg(feature = "backend-tikv")]
pub mod tikv;
