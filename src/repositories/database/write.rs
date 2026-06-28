use uuid::Uuid;

use crate::{
    error::AppError,
    models::database::{DatabaseInfoResponse, DatabaseMetadata},
    repositories::KasaneDbWrite,
};

impl<'a> KasaneDbWrite<'a> {
    /// Databaseの情報を取得する
    pub fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError> {
        if self.database_caches.contains_key(name) {
            return Ok(Some(DatabaseInfoResponse {
                name: name.to_string(),
            }));
        }

        let db = self.db.databases;
        if db.get(&self.write_txn, name)?.is_some() {
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
        let meta = DatabaseMetadata { id: crate::models::id::DatabaseId(id) };

        let db = self.db.databases;
        db.put(&mut self.write_txn, name, &meta)?;

        self.database_caches.insert(name.to_string(), meta);

        Ok(DatabaseInfoResponse {
            name: name.to_string(),
        })
    }

    /// Databaseを削除する
    pub fn database_remove(&mut self, name: &str) -> Result<(), AppError> {
        if name.is_empty() {
            return Err(AppError::DatabaseNotFound {
                name: name.to_string(),
            });
        }
        let _meta = {
            let db = self.db.databases;
            if let Some(meta_data) = db.get(&self.write_txn, name)? {
                meta_data
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: name.to_string(),
                });
            }
        };

        // Note: The caller is expected to delete all tables under this database
        // before deleting the database itself, or we do it here.
        // For simplicity, we just remove the database metadata here.

        let db = self.db.databases;
        db.delete(&mut self.write_txn, name)?;
        self.database_caches.remove(name);

        Ok(())
    }
}
