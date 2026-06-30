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
        let meta = DatabaseMetadata {
            id: crate::models::id::DatabaseId(id),
        };

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
        let db_id = {
            let db = self.db.databases;
            if let Some(meta_data) = db.get(&self.write_txn, name)? {
                meta_data.id
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: name.to_string(),
                });
            }
        };

        // データベース削除前に、関連するユーザー権限をすべて削除する。
        // `user_privileges` は (UserId, DatabaseId) をキーにしているため、全走査して対象の DatabaseId を探す。
        let privs_table = self.db.user_privileges;
        let mut priv_keys_to_delete = Vec::new();
        for item in privs_table.iter(&self.write_txn)? {
            let (k, _) = item?;
            if k.1 == db_id {
                priv_keys_to_delete.push(k);
            }
        }
        for k in priv_keys_to_delete {
            privs_table.delete(&mut self.write_txn, &k)?;
        }

        let db = self.db.databases;
        db.delete(&mut self.write_txn, name)?;
        self.database_caches.remove(name);

        Ok(())
    }
}
