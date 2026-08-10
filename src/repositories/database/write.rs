use uuid::Uuid;

use crate::{
    error::AppError,
    models::database::{DatabaseInfoResponse, DatabaseMetadata},
    repositories::{KasaneDbWrite, meta::MetaRead},
};

impl<'a> KasaneDbWrite<'a> {
    /// Databaseの情報を取得する
    #[tracing::instrument(skip_all)]
    pub fn database_info_impl(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError> {
        let db = self.db.databases;
        if let Some(meta) = db.get(&self.write_txn, name)? {
            Ok(Some(DatabaseInfoResponse {
                name: name.to_string(),
                description: meta.description,
            }))
        } else {
            Ok(None)
        }
    }

    /// Databaseを作成する
    #[tracing::instrument(skip_all)]
    pub fn database_create_impl(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseInfoResponse, AppError> {
        if self.database_info_impl(name)?.is_some() {
            return Err(AppError::DatabaseAlreadyExists {
                name: name.to_string(),
            });
        }

        let id = Uuid::now_v7();
        let meta = DatabaseMetadata {
            id: crate::models::id::DatabaseId(id),
            description: description.clone(),
        };

        let db_id = crate::models::id::DatabaseId(id);
        self.db.databases.put(&mut self.write_txn, name, &meta)?;
        self.db
            .database_id_index
            .put(&mut self.write_txn, &db_id, name)?;

        Ok(DatabaseInfoResponse {
            name: name.to_string(),
            description,
        })
    }

    /// Databaseを削除する
    #[tracing::instrument(skip_all)]
    pub fn database_remove_impl(&mut self, name: &str) -> Result<(), AppError> {
        if name.is_empty() {
            return Err(AppError::DatabaseNotFound {
                name: name.to_string(),
            });
        }
        let Some(meta) = self.db.databases.get(&self.write_txn, name)? else {
            return Err(AppError::DatabaseNotFound {
                name: name.to_string(),
            });
        };

        // 配下テーブルの列挙と削除は、この 1 つの書き込みトランザクション内で行う。
        // 列挙だけを別の読み取りトランザクションで済ませると、その隙間に作られた
        // テーブルが削除対象から漏れ、親を失って到達不能なまま残ってしまう。
        for table_name in self.table_names(meta.id)? {
            self.table_remove_impl(name, &table_name)?;
        }

        // ユーザーが持つ権限はデータベース ID で保存されており、削除したデータベースの
        // ID が再利用されることはない。よって残った権限ルールが後から作られた同名の
        // データベースに効くことはなく、ここで掃除する必要はない
        // （逆引きインデックスから消えるので、表示上は解決できないルールとして隠れる）。
        self.db.databases.delete(&mut self.write_txn, name)?;
        self.db
            .database_id_index
            .delete(&mut self.write_txn, &meta.id)?;

        Ok(())
    }

    /// Databaseの名前や説明を変更する
    #[tracing::instrument(skip_all)]
    pub fn database_update_impl(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<(), AppError> {
        let final_new_name = new_name.as_deref().unwrap_or(name);

        if name != final_new_name {
            // new_nameの妥当性を検証
            crate::services::helpers::name_valid::name_valid(final_new_name)?;
        }

        // 変更元の存在確認
        let mut meta = {
            let db = self.db.databases;
            if let Some(meta) = db.get(&self.write_txn, name)? {
                meta
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: name.to_string(),
                });
            }
        };

        if name != final_new_name {
            // コピー先が既に存在するか確認
            let db = self.db.databases;
            if db.get(&self.write_txn, final_new_name)?.is_some() {
                return Err(AppError::DatabaseAlreadyExists {
                    name: final_new_name.to_string(),
                });
            }
        }

        // 説明の更新
        if let Some(desc) = description {
            meta.description = desc;
        }

        let db = self.db.databases;
        if name != final_new_name {
            // lmdbから古いエントリを削除し、新しいエントリを追加
            db.delete(&mut self.write_txn, name)?;
            self.db
                .database_id_index
                .put(&mut self.write_txn, &meta.id, final_new_name)?;
        }
        db.put(&mut self.write_txn, final_new_name, &meta)?;
        Ok(())
    }

    /// Databaseをコピーする。
    #[tracing::instrument(skip_all)]
    pub fn database_copy_impl(
        &mut self,
        src_db_name: &str,
        copy_name: &str,
    ) -> Result<DatabaseInfoResponse, AppError> {
        // コピー先データベース名の妥当性検証
        crate::services::helpers::name_valid::name_valid(copy_name)?;

        // 1. コピー元データベースの存在確認
        let src_db_meta = {
            let db = self.db.databases;
            db.get(&self.write_txn, src_db_name)?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: src_db_name.to_string(),
                })?
        };

        // 2. コピー先データベースがすでに存在するかチェック
        if self.database_info_impl(copy_name)?.is_some() {
            return Err(AppError::DatabaseAlreadyExists {
                name: copy_name.to_string(),
            });
        }

        // 3. コピー先データベースを作成
        let copy_db_id = crate::models::id::DatabaseId(Uuid::now_v7());
        let copy_meta = DatabaseMetadata {
            id: copy_db_id,
            description: src_db_meta.description.clone(),
        };

        self.db
            .databases
            .put(&mut self.write_txn, copy_name, &copy_meta)?;
        self.db
            .database_id_index
            .put(&mut self.write_txn, &copy_db_id, copy_name)?;

        // 4. コピー元データベース内の全テーブル名を取得
        let table_names = self.table_names(src_db_meta.id)?;

        // 5. 各テーブルをコピー
        for table_name in table_names {
            self.table_copy_impl(src_db_name, &table_name, copy_name, &table_name)?;
        }

        Ok(DatabaseInfoResponse {
            name: copy_name.to_string(),
            description: src_db_meta.description,
        })
    }
}
