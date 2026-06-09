use redb::ReadableTable;
use uuid::Uuid;

use crate::{
    db_init::DATABASES,
    error::AppError,
    models::database::{DatabaseInfoResponse, DatabaseMetadata},
    repositories::KasaneDbWrite,
};

impl KasaneDbWrite {
    /// Databaseの情報を取得する
    pub fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError> {
        if self.database_caches.contains_key(name) {
            return Ok(Some(DatabaseInfoResponse {
                name: name.to_string(),
            }));
        }

        let redb_dbs = self.write_txn.open_table(DATABASES)?;
        if redb_dbs.get(name)?.is_some() {
            Ok(Some(DatabaseInfoResponse {
                name: name.to_string(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Databaseを作成する
    pub fn database_create(&mut self, name: &str) -> Result<DatabaseInfoResponse, AppError> {
        if self.database_info(name)?.is_some() {
            return Err(AppError::DatabaseAlreadyExists {
                name: name.to_string(),
            });
        }

        let id = Uuid::now_v7();
        let meta = DatabaseMetadata { id };

        let mut redb_dbs = self.write_txn.open_table(DATABASES)?;
        redb_dbs.insert(name, meta.clone())?;

        self.database_caches.insert(name.to_string(), meta);

        Ok(DatabaseInfoResponse {
            name: name.to_string(),
        })
    }

    /// Databaseを削除する
    pub fn database_remove(&mut self, name: &str) -> Result<(), AppError> {
        let _meta = {
            let redb_dbs = self.write_txn.open_table(DATABASES)?;
            if let Some(meta_data) = redb_dbs.get(name)? {
                meta_data.value()
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: name.to_string(),
                });
            }
        };

        // Note: The caller is expected to delete all tables under this database
        // before deleting the database itself, or we do it here.
        // For simplicity, we just remove the database metadata here.

        let mut redb_dbs = self.write_txn.open_table(DATABASES)?;
        redb_dbs.remove(name)?;
        self.database_caches.remove(name);

        Ok(())
    }
}
