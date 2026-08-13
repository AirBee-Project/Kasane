//! ビルド時に選択されたストレージバックエンドを 1 つの名前へ束ねる。
//!
//! 呼び出し側が見るのは [`Db`] だけで、その契約は
//! [`Storage`](crate::repositories::Storage) 以下の trait 群。実装の中身は外へ出さない。

#[cfg(all(feature = "backend-lmdb", feature = "backend-tikv"))]
compile_error!(
    "backend-lmdb と backend-tikv は同時に有効にできません。\
     バックエンドはビルドごとに 1 つだけ選んでください\
     （例: --no-default-features --features backend-tikv）"
);

#[cfg(not(any(feature = "backend-lmdb", feature = "backend-tikv")))]
compile_error!(
    "ストレージバックエンドが選択されていません。\
     backend-lmdb か backend-tikv のどちらかを有効にしてください"
);

use crate::error::AppError;

/// このビルドで使うストレージ。
#[cfg(feature = "backend-lmdb")]
pub type Db = crate::repositories::lmdb::AppDb;

#[cfg(feature = "backend-tikv")]
pub type Db = crate::repositories::tikv::TikvDb;

/// 選択中のバックエンド名（起動ログ・診断用）。
#[cfg(feature = "backend-lmdb")]
pub const NAME: &str = "lmdb";

#[cfg(feature = "backend-tikv")]
pub const NAME: &str = "tikv";

/// 接続先の既定値。指すものはバックエンドで変わる（ディレクトリ / PD エンドポイント）。
#[cfg(feature = "backend-lmdb")]
pub fn default_target() -> String {
    std::env::var("DATABASE_DIR").unwrap_or_else(|_| "default_kasane_db".to_string())
}

#[cfg(feature = "backend-tikv")]
pub fn default_target() -> String {
    crate::repositories::tikv::TikvConfig::from_env()
        .pd_endpoints
        .join(",")
}

/// 構築手順をここに閉じることで、呼び出し側に feature 分岐が残らない。
#[cfg(feature = "backend-lmdb")]
pub async fn open(target: &str) -> Result<Db, AppError> {
    Ok(crate::repositories::lmdb::initialize_database(target))
}

#[cfg(feature = "backend-tikv")]
pub async fn open(target: &str) -> Result<Db, AppError> {
    use crate::repositories::tikv::{TikvConfig, TikvDb};

    let db = TikvDb::connect(TikvConfig::from_endpoints(target)).await?;
    // 常駐処理はプロセスの寿命を持つので、接続のたびではなくここで 1 度だけ起こす。
    let gc_config = crate::repositories::tikv::GcConfig::from_env();
    // 消したテーブルの実体回収と、生きているキーの古い版の回収は別の仕組み。
    db.spawn_sweeper(gc_config.clone());
    db.spawn_gc(gc_config);
    Ok(db)
}
