pub mod database;
pub mod users;

use crate::models::{database::DatabaseMetadata, database::table::TableMetadata};

use std::collections::HashMap;

pub struct KasaneDbRead<'a> {
    pub read_txn: heed::RoTxn<'a, heed::WithoutTls>,
    pub db: &'a crate::db_init::AppDb,
}

impl<'a> KasaneDbRead<'a> {
    pub fn new(read_txn: heed::RoTxn<'a, heed::WithoutTls>, db: &'a crate::db_init::AppDb) -> Self {
        Self { read_txn, db }
    }
}

pub struct KasaneDbWrite<'a> {
    pub write_txn: heed::RwTxn<'a>,
    pub db: &'a crate::db_init::AppDb,
    pub database_caches: HashMap<String, DatabaseMetadata>,
    pub table_caches: HashMap<(crate::models::id::DatabaseId, String), TableMetadata>,
}

impl<'a> KasaneDbWrite<'a> {
    pub fn new(write_txn: heed::RwTxn<'a>, db: &'a crate::db_init::AppDb) -> Self {
        Self {
            write_txn,
            db,
            database_caches: HashMap::new(),
            table_caches: std::collections::HashMap::new(),
        }
    }

    pub fn commit(self) -> Result<(), crate::error::AppError> {
        self.write_txn.commit()?;
        Ok(())
    }

    pub fn abort(self) -> Result<(), crate::error::AppError> {
        self.write_txn.abort();
        Ok(())
    }
}

use crate::error::AppError;
use crate::models::database::table::TableDataType;
use crate::models::id::TableId;
use crate::repositories::database::table::data::batch::WriteOp;
use crate::repositories::users::{KasaneUsersRead, KasaneUsersWrite};
use kasane_logic::SpatialIdSet;

impl crate::db_init::AppDb {
    /// 読み取りトランザクションを開き、リポジトリ越しに処理を実行して閉じる。
    /// heed の txn 型を上位（サービス/ハンドラ）へ露出させないための境界。
    pub fn read<T>(
        &self,
        f: impl FnOnce(&KasaneDbRead<'_>) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let txn = self.env.read_txn()?;
        f(&KasaneDbRead::new(txn, self))
    }

    /// 書き込みトランザクションを開き、処理が成功したら commit、失敗したら abort する。
    ///
    /// 注意：データ書き込み（insert/upsert/remove）は [`Self::batch_data_insert`] 等の
    /// バッチャー経由を使うこと。本メソッドはメタデータ書き込み（DB/テーブル/ユーザー）向け。
    pub fn write<T>(
        &self,
        f: impl FnOnce(&mut KasaneDbWrite<'_>) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let mut w = KasaneDbWrite::new(self.env.write_txn()?, self);
        match f(&mut w) {
            Ok(out) => {
                w.commit()?;
                Ok(out)
            }
            // エラー時は commit せず w を drop すると、RwTxn は自動で abort される。
            Err(e) => Err(e),
        }
    }

    /// ユーザー用リポジトリの読み取り版。
    pub fn read_users<T>(
        &self,
        f: impl FnOnce(&KasaneUsersRead<'_>) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let txn = self.env.read_txn()?;
        f(&KasaneUsersRead::new(txn, self))
    }

    /// ユーザー用リポジトリの書き込み版。成功時 commit、失敗時 abort。
    pub fn write_users<T>(
        &self,
        f: impl FnOnce(&mut KasaneUsersWrite<'_>) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let mut w = KasaneUsersWrite::new(self.env.write_txn()?, self);
        match f(&mut w) {
            Ok(out) => {
                w.commit()?;
                Ok(out)
            }
            // エラー時は commit せず w を drop すると、RwTxn は自動で abort される。
            Err(e) => Err(e),
        }
    }

    async fn enqueue_write(
        &self,
        op: WriteOp,
        rx: tokio::sync::oneshot::Receiver<Result<(), AppError>>,
    ) -> Result<(), AppError> {
        self.write_sender
            .send(op)
            .await
            .map_err(|_| AppError::InternalError("WriteBatcher channel is closed".to_string()))?;
        rx.await
            .map_err(|_| AppError::InternalError("WriteBatcher failed to reply".to_string()))?
    }

    /// 検証済みの insert をバッチャーへ投入し、コミット完了まで待つ。
    pub async fn batch_data_insert(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        value: Vec<u8>,
    ) -> Result<(), AppError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.enqueue_write(
            WriteOp::Insert {
                table_id,
                data_type,
                ids,
                value,
                reply: tx,
            },
            rx,
        )
        .await
    }

    /// 検証済みの upsert をバッチャーへ投入し、コミット完了まで待つ。
    pub async fn batch_data_upsert(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        value: Vec<u8>,
    ) -> Result<(), AppError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.enqueue_write(
            WriteOp::Upsert {
                table_id,
                data_type,
                ids,
                value,
                reply: tx,
            },
            rx,
        )
        .await
    }

    /// 検証済みの remove をバッチャーへ投入し、コミット完了まで待つ。
    pub async fn batch_data_remove(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.enqueue_write(
            WriteOp::Remove {
                table_id,
                data_type,
                ids,
                reply: tx,
            },
            rx,
        )
        .await
    }
}
