//! ビルド時に選択されたストレージバックエンドを 1 つの名前へ束ねる。
//!
//! アプリケーション（`AppState`・サービス層・ハンドラ層）はこの [`Db`] だけを見る。
//! [`Db`] が満たす契約は [`Storage`](crate::repositories::Storage) 以下の trait 群であり、
//! 実装の中身（LMDB の `Env` なのか TiKV のクライアントなのか）は外へ出さない。
//!
//! 切り替えは Cargo feature で行う：
//!
//! ```text
//! cargo build                                            # backend-lmdb（既定）
//! cargo build --no-default-features --features backend-tikv
//! ```

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

/// 接続先の既定値。
///
/// 何を指すかはバックエンドによって変わる（LMDB はデータディレクトリ、
/// TiKV は PD エンドポイント）。呼び出し側はこの区別を知らずに、
/// [`open`] へそのまま渡せばよい。
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

/// ストレージを開く。
///
/// バックエンドごとの構築手順をここに閉じることで、呼び出し側（`main`）に
/// feature 分岐が残らない。
#[cfg(feature = "backend-lmdb")]
pub async fn open(target: &str) -> Result<Db, AppError> {
    Ok(crate::repositories::lmdb::initialize_database(target))
}

#[cfg(feature = "backend-tikv")]
pub async fn open(target: &str) -> Result<Db, AppError> {
    use crate::repositories::tikv::{TikvConfig, TikvDb};

    // 接続設定の解釈（既定値・TLS・区切り方）は init.rs に一本化してある。
    let db = TikvDb::connect(TikvConfig::from_endpoints(target)).await?;
    // 論理削除されたテーブルの実体を回収し続ける常駐処理。プロセスの寿命を持つので、
    // 接続を開くたびではなくここで 1 度だけ起こす（`repositories::tikv::gc` を参照）。
    let gc_config = crate::repositories::tikv::GcConfig::from_env();
    // 消したテーブルの実体回収と、生きているキーの古い版の回収。別々の仕組みなので両方要る。
    db.spawn_sweeper(gc_config.clone());
    db.spawn_gc(gc_config);
    Ok(db)
}
