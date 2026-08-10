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

/// このビルドで使うストレージ。
#[cfg(feature = "backend-lmdb")]
pub type Db = crate::db_init::AppDb;

#[cfg(feature = "backend-tikv")]
pub type Db = crate::repositories::tikv::TikvDb;

/// 選択中のバックエンド名（起動ログ・診断用）。
#[cfg(feature = "backend-lmdb")]
pub const NAME: &str = "lmdb";

#[cfg(feature = "backend-tikv")]
pub const NAME: &str = "tikv";
