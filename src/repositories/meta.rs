//! LMDB 上のメタデータへの点参照。
//!
//! 読み取り・書き込みどちらのトランザクションからも同じ引き方をするので、ここに 1 度だけ
//! 書いてマクロで両方へ実装する。
//!
//! その上に載る権限ルールの名前 ⇄ ID 変換は、バックエンド非依存な既定実装として
//! [`MetaRepository`](crate::repositories::MetaRepository) が 1 箇所だけ持つ
//! （認可の規則をバックエンドごとに複製しないため）。
//!
//! 全走査に依存する逆引きは持たず、`database_id_index` / `table_id_index` への
//! 点参照だけで済ませている。

use heed::BytesDecode;

use crate::db_init::DbIdAndName;
use crate::error::AppError;
use crate::models::id::{DatabaseId, TableId};
use crate::models::users::UserMetadata;
use crate::repositories::{KasaneDbRead, KasaneDbWrite};

pub trait MetaRead {
    // --- トランザクション種別ごとに実装する点参照 ---

    fn database_id(&self, name: &str) -> Result<Option<DatabaseId>, AppError>;
    fn table_id(&self, db_id: DatabaseId, table_name: &str) -> Result<Option<TableId>, AppError>;
    /// `DatabaseId` からデータベース名を引く（`database_id_index` への点参照）。
    fn database_name(&self, db_id: DatabaseId) -> Result<Option<String>, AppError>;
    /// `TableId` からテーブル名を引く（`table_id_index` への点参照）。
    fn table_name(&self, table_id: TableId) -> Result<Option<String>, AppError>;
    fn user_meta(&self, username: &str) -> Result<Option<UserMetadata>, AppError>;
    /// データベース配下のテーブル名を列挙する。
    fn table_names(&self, db_id: DatabaseId) -> Result<Vec<String>, AppError>;
}

/// 点参照 5 つを、トランザクションを保持するフィールド名を指定して実装する。
macro_rules! impl_meta_read {
    ($target:ty, $txn:ident) => {
        impl MetaRead for $target {
            fn database_id(&self, name: &str) -> Result<Option<DatabaseId>, AppError> {
                if name.is_empty() {
                    return Ok(None);
                }
                Ok(self.db.databases.get(&self.$txn, name)?.map(|meta| meta.id))
            }

            fn table_id(
                &self,
                db_id: DatabaseId,
                table_name: &str,
            ) -> Result<Option<TableId>, AppError> {
                if table_name.is_empty() {
                    return Ok(None);
                }
                Ok(self
                    .db
                    .tables
                    .get(&self.$txn, &(db_id, table_name))?
                    .map(|meta| meta.id))
            }

            fn database_name(&self, db_id: DatabaseId) -> Result<Option<String>, AppError> {
                Ok(self
                    .db
                    .database_id_index
                    .get(&self.$txn, &db_id)?
                    .map(str::to_string))
            }

            fn table_name(&self, table_id: TableId) -> Result<Option<String>, AppError> {
                Ok(self
                    .db
                    .table_id_index
                    .get(&self.$txn, &table_id)?
                    .map(str::to_string))
            }

            fn user_meta(&self, username: &str) -> Result<Option<UserMetadata>, AppError> {
                match self.db.users.get(&self.$txn, username)? {
                    Some(val) => Ok(Some(serde_json::from_str(val).map_err(|_| {
                        AppError::InternalError("Failed to parse user metadata".into())
                    })?)),
                    None => Ok(None),
                }
            }

            fn table_names(&self, db_id: DatabaseId) -> Result<Vec<String>, AppError> {
                let prefix = db_id.into_bytes();
                let mut names = Vec::new();
                for item in self
                    .db
                    .tables
                    .remap_types::<heed::types::Bytes, heed::types::Bytes>()
                    .prefix_iter(&self.$txn, prefix.as_slice())?
                {
                    let (k_bytes, _) = item?;
                    let (_, name) = DbIdAndName::bytes_decode(k_bytes).map_err(|e| {
                        AppError::InternalError(format!("Failed to decode table key: {e}"))
                    })?;
                    names.push(name.to_string());
                }
                Ok(names)
            }
        }
    };
}

impl_meta_read!(KasaneDbRead<'_>, read_txn);
impl_meta_read!(KasaneDbWrite<'_>, write_txn);
