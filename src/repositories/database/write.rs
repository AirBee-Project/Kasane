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

        // Todo:全探索を防ぐべき。PrefixにするとKeyが長くなってしまうので、逆引きのTableを整備すればよい。
        // データベースとTableはそんなに変化がないので逆引きがあっても問題ない。いつかやる
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

    /// Databaseの名前を変更する
    pub fn database_rename(&mut self, name: &str, new_name: &str) -> Result<(), AppError> {
        if name == new_name {
            return Ok(());
        }

        // new_nameの妥当性を検証
        crate::services::helpers::name_valid::name_valid(new_name)?;

        // コピー元の存在確認
        let meta = {
            let db = self.db.databases;
            if let Some(meta) = db.get(&self.write_txn, name)? {
                meta
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: name.to_string(),
                });
            }
        };

        // コピー先が既に存在するか確認
        let db = self.db.databases;
        if db.get(&self.write_txn, new_name)?.is_some() {
            return Err(AppError::DatabaseAlreadyExists {
                name: new_name.to_string(),
            });
        }

        // lmdbから古いエントリを削除し、新しいエントリを追加
        db.delete(&mut self.write_txn, name)?;
        db.put(&mut self.write_txn, new_name, &meta)?;

        // キャッシュの更新
        self.database_caches.remove(name);
        self.database_caches.insert(new_name.to_string(), meta);

        Ok(())
    }

    /// Databaseをコピーする。
    pub fn database_copy(
        &mut self,
        src_db_name: &str,
        dest_db_name: &str,
        user_id: Option<crate::models::id::UserId>,
    ) -> Result<DatabaseInfoResponse, AppError> {
        // コピー先データベース名の妥当性検証
        crate::services::helpers::name_valid::name_valid(dest_db_name)?;

        // 1. コピー元データベースの存在確認
        let src_db_meta = {
            let db = self.db.databases;
            db.get(&self.write_txn, src_db_name)?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: src_db_name.to_string(),
                })?
        };

        // 2. コピー先データベースがすでに存在するかチェック
        if self.database_info(dest_db_name)?.is_some() {
            return Err(AppError::DatabaseAlreadyExists {
                name: dest_db_name.to_string(),
            });
        }

        // 3. コピー先データベースを作成
        let dest_db_id = crate::models::id::DatabaseId(Uuid::now_v7());
        let dest_meta = DatabaseMetadata { id: dest_db_id };

        let db = self.db.databases;
        db.put(&mut self.write_txn, dest_db_name, &dest_meta)?;
        self.database_caches
            .insert(dest_db_name.to_string(), dest_meta);

        // 4. コピー元データベース内の全テーブル名を取得
        let db_tables = self.db.tables;
        let mut table_names = Vec::new();
        let src_db_id_bytes = src_db_meta.id.into_bytes();
        for iter in db_tables
            .remap_types::<heed::types::Bytes, heed::types::Bytes>()
            .prefix_iter(&self.write_txn, src_db_id_bytes.as_slice())?
        {
            let (k_bytes, _) = iter?;
            if k_bytes.len() > 16 {
                let name = std::str::from_utf8(&k_bytes[16..]).map_err(|e| {
                    AppError::InternalError(format!("Invalid table name encoding: {}", e))
                })?;
                table_names.push(name.to_string());
            }
        }

        // 5. 各テーブルをコピー
        for table_name in table_names {
            self.table_copy(src_db_name, &table_name, dest_db_name, &table_name)?;
        }

        // 6. コピー実行ユーザーに対して新しいデータベースの Manage 権限を自動付与
        if let Some(uid) = user_id {
            let privs_table = self.db.user_privileges;
            privs_table.put(
                &mut self.write_txn,
                &(uid, dest_db_id),
                &(crate::models::users::UserRole::Manage as u8),
            )?;
        }

        Ok(DatabaseInfoResponse {
            name: dest_db_name.to_string(),
        })
    }
}
