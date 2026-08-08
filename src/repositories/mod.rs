pub mod database;
pub mod meta;
pub mod users;

pub use meta::MetaRead;

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
}

impl<'a> KasaneDbWrite<'a> {
    pub fn new(write_txn: heed::RwTxn<'a>, db: &'a crate::db_init::AppDb) -> Self {
        Self { write_txn, db }
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

    /// 検証済みの insert を実行し、コミット完了まで待つ。
    pub async fn batch_data_insert(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        value: Vec<u8>,
    ) -> Result<(), AppError> {
        let db = self.clone();
        tokio::task::spawn_blocking(move || {
            db.write(|w| w.data_insert(table_id, data_type, ids, &value))
        })
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?
    }

    /// 検証済みの upsert を実行し、コミット完了まで待つ。
    pub async fn batch_data_upsert(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        value: Vec<u8>,
    ) -> Result<(), AppError> {
        let db = self.clone();
        tokio::task::spawn_blocking(move || {
            db.write(|w| w.data_upsert(table_id, data_type, ids, &value))
        })
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?
    }

    /// 検証済みの remove を実行し、コミット完了まで待つ。
    pub async fn batch_data_remove(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        let db = self.clone();
        tokio::task::spawn_blocking(move || db.write(|w| w.data_remove(table_id, data_type, ids)))
            .await
            .map_err(|e| AppError::InternalError(e.to_string()))?
    }
}
