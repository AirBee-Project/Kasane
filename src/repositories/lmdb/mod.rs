//! LMDB(heed) バックエンド。
//!
//! [`Storage::read`] / [`Storage::write`] がクロージャ全体を `spawn_blocking` の中で回すのは、
//! LMDB が「1 つのトランザクションは単一スレッドからのみ」を要求するため（[`heed::RwTxn`]
//! が [`Send`] でも、POSIX ではライタミューテックスに所有スレッド制約がある）。blocking
//! タスクの中で開いて閉じれば、スレッドを跨がないことが構造から保証され、`'static` へ
//! 延長するための unsafe な自己参照も要らなくなる。

pub mod catalog;
pub mod data;
pub mod init;
pub mod keys;
pub mod query_source;
mod repository;
pub mod shard;
pub mod users;

pub use init::initialize_database;

use heed::types::*;
use heed::{Database, Env, WithoutTls};

use crate::error::AppError;
use crate::models::{database::DatabaseMetadata, database::table::TableMetadata};
use crate::repositories::Storage;

use keys::{DbIdAndName, TableIdAndFlexId};

#[derive(Clone)]
pub struct AppDb {
    pub env: Env<heed::WithoutTls>,

    /// データベース名 -> `DatabaseMetadata`
    pub databases: Database<Str, SerdeBincode<DatabaseMetadata>>,

    /// `(DatabaseId, テーブル名)` -> `TableMetadata`
    pub tables: Database<DbIdAndName, SerdeJson<TableMetadata>>,

    /// `DatabaseId` -> データベース名。権限は ID で保存されるので逆引きが要る。
    pub database_id_index: Database<SerdeBincode<crate::models::id::DatabaseId>, Str>,

    /// `TableId` -> テーブル名
    pub table_id_index: Database<SerdeBincode<crate::models::id::TableId>, Str>,

    /// ユーザー名 -> `UserMetadata` の JSON
    pub users: Database<Str, Str>,

    /// `(TableId, FlexId)` -> シャードエントリ（`encoding::shard_entry`）
    pub tables_data: Database<TableIdAndFlexId, Bytes>,

    /// `table_id ‖ 順序保存エンコード値 ‖ flexid` -> 値なし
    pub value_index: Database<Bytes, Unit>,
}

/// `AppError` をバックエンド非依存に保つため、heed のエラー型を知るのはここだけにする。
impl From<heed::Error> for AppError {
    fn from(error: heed::Error) -> Self {
        Self::StorageError(error.to_string())
    }
}

pub struct KasaneDbRead<'a> {
    pub read_txn: heed::RoTxn<'a, heed::WithoutTls>,
    pub db: &'a AppDb,
}

impl<'a> KasaneDbRead<'a> {
    pub fn new(read_txn: heed::RoTxn<'a, heed::WithoutTls>, db: &'a AppDb) -> Self {
        Self { read_txn, db }
    }
}

pub struct KasaneDbWrite<'a> {
    pub write_txn: heed::RwTxn<'a>,
    pub db: &'a AppDb,
}

impl<'a> KasaneDbWrite<'a> {
    pub fn new(write_txn: heed::RwTxn<'a>, db: &'a AppDb) -> Self {
        Self { write_txn, db }
    }

    pub fn commit(self) -> Result<(), AppError> {
        self.write_txn.commit()?;
        Ok(())
    }
}

/// クエリ 1 回分の読み取り断面。読み取りトランザクション自体が MVCC スナップショット。
///
/// [`Env::static_read_txn`] で `'static` にするのは、`'static` を要求する
/// [`Source`](kasane_logic::Source) へそのまま持ち込むため。[`Mutex`](std::sync::Mutex) は
/// [`Sync`] を得るため（[`heed::RoTxn`] は移動はできるが同時使用はできない）。
pub type LmdbQuerySnapshot = std::sync::Arc<std::sync::Mutex<heed::RoTxn<'static, WithoutTls>>>;

impl Storage for AppDb {
    type Read<'a> = KasaneDbRead<'a>;
    type Write<'a> = KasaneDbWrite<'a>;
    type QuerySnapshot = LmdbQuerySnapshot;

    async fn query_snapshot(&self) -> Result<Self::QuerySnapshot, AppError> {
        let txn = self.env.clone().static_read_txn()?;
        Ok(std::sync::Arc::new(std::sync::Mutex::new(txn)))
    }

    async fn read<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a Self::Read<'b>) -> Result<T, AppError> + Send + 'static,
        T: Send + 'static,
    {
        let db = self.clone();
        let handle = tokio::runtime::Handle::current();
        // blocking タスクは呼び出し元のスパンを引き継がないので、明示的に渡す。
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            let _guard = span.enter();
            let r = KasaneDbRead::new(db.env.read_txn()?, &db);
            handle.block_on(f(&r))
        })
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?
    }

    async fn write<T, F>(&self, f: F) -> Result<T, AppError>
    where
        F: for<'a, 'b> AsyncFnOnce(&'a mut Self::Write<'b>) -> Result<T, AppError>
            + Clone
            + Send
            + 'static,
        T: Send + 'static,
    {
        let db = self.clone();
        let handle = tokio::runtime::Handle::current();
        let span = tracing::Span::current();
        tokio::task::spawn_blocking(move || {
            let _guard = span.enter();
            let mut w = KasaneDbWrite::new(db.env.write_txn()?, &db);
            // 単一ライタなので競合でのやり直しは起きない。エラー時は drop で abort される。
            let out = handle.block_on(f(&mut w))?;
            w.commit()?;
            Ok(out)
        })
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?
    }
}
