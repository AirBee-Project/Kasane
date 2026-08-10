//! LMDB(heed) バックエンド。
//!
//! ファイル構成は TiKV 実装（`super::tikv`）と対になっている。
//!
//! | ファイル | 役割 |
//! |---|---|
//! | `mod.rs` | ストレージ本体と、トランザクション境界（[`Storage`] 実装） |
//! | `init.rs` | このバックエンド固有の初期化設定 |
//! | `keys.rs` | キーのバイト表現 |
//! | `shard.rs` | このバックエンド固有の低レベルアクセス（TiKV 側は `kv.rs`） |
//! | `catalog.rs` | データベース・テーブルのカタログ操作 |
//! | `data.rs` | FlexTree のデータ操作 |
//! | `users.rs` | ユーザーと権限 |
//! | `query_source.rs` | クエリ実行器への入力源 |
//! | `repository.rs` | 抽象 trait への適合 |
//!
//! # なぜクロージャ全体を `spawn_blocking` の中で回すのか
//!
//! LMDB は「1 つのトランザクションは単一スレッドからのみ使うこと」を要求する
//! （`RwTxn` が `Send` であっても、POSIX ではライタミューテックスの所有スレッド制約がある）。
//! そこで [`Storage::read`] / [`Storage::write`] は blocking タスクを 1 つ起こし、
//! **その中でトランザクションを開き、クロージャを最後まで回し、閉じる**。
//! トランザクションがスレッドを跨がないことが構造から保証され、`'static` へ延長するための
//! unsafe な自己参照も要らなくなる。
//!
//! クロージャ内の `.await` は `Handle::block_on` で回す。LMDB 側の Future は即座に
//! 完了するので、ブロッキングスレッドを実質的に占有しない。

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
    /// LMDBの環境本体
    pub env: Env<heed::WithoutTls>,

    /// データベースの一覧とメタデータを管理する
    /// Key: データベース名 (`Str`) -> Value: `DatabaseMetadata`
    pub databases: Database<Str, SerdeBincode<DatabaseMetadata>>,

    /// テーブルの一覧とメタデータを管理する
    /// Key: `DatabaseId` と テーブル名 (`DbIdAndName`) -> Value: `TableMetadata`
    pub tables: Database<DbIdAndName, SerdeJson<TableMetadata>>,

    /// `DatabaseId` からデータベース名を引くための逆引きインデックス。
    /// 権限は ID で保存されるため、それを名前へ戻すのに使う。
    /// Key: `DatabaseId` -> Value: データベース名 (`Str`)
    pub database_id_index: Database<SerdeBincode<crate::models::id::DatabaseId>, Str>,

    /// `TableId` の存在確認と、テーブル名の逆引きに用いるインデックス。
    /// Key: `TableId` -> Value: テーブル名 (`Str`)
    pub table_id_index: Database<SerdeBincode<crate::models::id::TableId>, Str>,

    /// 登録済みのユーザーを管理する
    /// Key: ユーザー名 (`Str`) -> Value: `UserMetadata` の JSON (`Str`)
    pub users: Database<Str, Str>,

    /// FlexTree
    /// Key: `TableId` と 空間ID（`FlexId`）(`TableIdAndFlexId`) -> SpatialIdMapのバイト列(rkvy)
    pub tables_data: Database<TableIdAndFlexId, Bytes>,

    /// 値→空間の二次インデックス（値フィルタ用）。
    /// Key 生バイト列: `table_id(16) ‖ 順序保存エンコード値(可変) ‖ flexid.encode(FlexId::ENCODED_LEN)` -> 値なし
    pub value_index: Database<Bytes, Unit>,
}

/// heed のエラーをアプリケーションのエラーへ持ち上げる。
///
/// `AppError` はバックエンド非依存に保ちたいので、具体的なエラー型を知っているのは
/// このモジュールだけにする（feature でバックエンドを差し替える際もここごと入れ替わる）。
impl From<heed::Error> for AppError {
    fn from(error: heed::Error) -> Self {
        AppError::StorageError(error.to_string())
    }
}

/// 読み取りトランザクションと、それが属するストレージ。
pub struct KasaneDbRead<'a> {
    pub read_txn: heed::RoTxn<'a, heed::WithoutTls>,
    pub db: &'a AppDb,
}

impl<'a> KasaneDbRead<'a> {
    pub fn new(read_txn: heed::RoTxn<'a, heed::WithoutTls>, db: &'a AppDb) -> Self {
        Self { read_txn, db }
    }
}

/// 書き込みトランザクションと、それが属するストレージ。
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

/// クエリ 1 回分の読み取り断面。
///
/// LMDB の読み取りトランザクションはそれ自体が MVCC スナップショットなので、
/// 1 つを開いてクエリ中の全ソースで共有すれば断面が固定される。
///
/// `Env` を所有する [`Env::static_read_txn`] を使うのが要点で、これにより
/// トランザクションが `'static` になり、`'static` を要求する
/// [`Source`](kasane_logic::Source) の中へそのまま持ち込める。
///
/// `Mutex` で包むのは共有のため。`RoTxn` は（`WithoutTls` でも）スレッド間の
/// 移動はできるが同時使用はできないので、`Sync` を得るには排他が要る。
/// 読み取りが直列化されるのは TiKV 側（`Mutex<Transaction>`）と同じ性質。
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
            // トランザクションはこの blocking スレッド上で開き、ここで閉じる。
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
            // LMDB は単一ライタなので競合でやり直しになることはない。1 回で確定する。
            // エラー時は commit せずに w を drop すると RwTxn は自動で abort される。
            let out = handle.block_on(f(&mut w))?;
            w.commit()?;
            Ok(out)
        })
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?
    }
}
