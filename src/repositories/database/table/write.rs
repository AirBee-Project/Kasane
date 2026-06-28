use crate::{
    error::AppError,
    models::database::table::{Table, TableDataType, TableMetadata},
    repositories::KasaneDbWrite,
};

impl<'a> KasaneDbWrite<'a> {
    /// Tableの情報を取得する
    pub fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        if db_name.is_empty() {
            return Ok(None);
        }
        let db_meta = {
            let db = self.db.databases;
            if let Some(m) = db.get(&self.write_txn, db_name)? {
                m
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        if let Some(meta_data) = self.table_caches.get(&(db_meta.id, table_name.to_string())) {
            return Ok(Some(Table {
                id: meta_data.id,
                name: table_name.to_string(),
                data_type: meta_data.data_type,
                max_zoom_level: meta_data.max_zoom_level,
            }));
        }

        let db = self.db.tables;
        if let Some(m) = db.get(&self.write_txn, &(db_meta.id, table_name))? {
            Ok(Some(Table {
                id: m.id,
                name: table_name.to_string(),
                data_type: m.data_type,
                max_zoom_level: m.max_zoom_level,
            }))
        } else {
            Ok(None)
        }
    }

    /// Tableを作成する
    pub fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
    ) -> Result<Table, AppError> {
        if db_name.is_empty() {
            return Err(AppError::DatabaseNotFound {
                name: db_name.to_string(),
            });
        }
        if table_name.is_empty() {
            return Err(AppError::InternalError(
                "Table name cannot be empty".to_string(),
            ));
        }
        let db_meta = {
            let db = self.db.databases;
            if let Some(m) = db.get(&self.write_txn, db_name)? {
                m
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        if self.table_info(db_name, table_name)?.is_some() {
            return Err(AppError::TableAlreadyExists {
                name: table_name.to_string(),
            });
        }

        let db_index = self.db.table_id_index;

        let mut id = crate::models::id::TableId(uuid::Uuid::now_v7());
        loop {
            if db_index.get(&self.write_txn, &id)?.is_none() {
                break;
            }
            id = crate::models::id::TableId(uuid::Uuid::now_v7());
        }

        let meta = TableMetadata {
            id,
            data_type,
            max_zoom_level,
        };

        let db = self.db.tables;
        db.put(&mut self.write_txn, &(db_meta.id, table_name), &meta)?;
        db_index.put(&mut self.write_txn, &id, &())?;

        self.table_caches
            .insert((db_meta.id, table_name.to_string()), meta);

        Ok(Table {
            id,
            name: table_name.to_string(),
            data_type,
            max_zoom_level,
        })
    }

    /// Tableを削除する（メタデータ・IDインデックス・シャードデータをすべて削除）。
    pub fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError> {
        let table = match self.table_info(db_name, table_name)? {
            Some(t) => t,
            None => {
                return Err(AppError::TableNotFound {
                    name: table_name.to_string(),
                });
            }
        };

        let db_meta = {
            let db = self.db.databases;
            db.get(&self.write_txn, db_name)?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                })?
        };

        // 1. シャードデータを全削除（tables_data の table_id プレフィックス）。
        //    反復中に削除できないため、キーを集めてから削除する。
        let tables_data = self
            .db
            .tables_data
            .remap_types::<heed::types::Bytes, heed::types::Bytes>();
        let prefix = table.id.into_bytes();
        let keys: Vec<Vec<u8>> = {
            let mut ks = Vec::new();
            for iter in tables_data.prefix_iter(&self.write_txn, prefix.as_slice())? {
                let (k_bytes, _) = iter?;
                ks.push(k_bytes.to_vec());
            }
            ks
        };
        for k in keys {
            tables_data.delete(&mut self.write_txn, &k)?;
        }

        // 2. テーブルメタデータと ID インデックスを削除。
        self.db
            .tables
            .delete(&mut self.write_txn, &(db_meta.id, table_name))?;
        self.db
            .table_id_index
            .delete(&mut self.write_txn, &table.id)?;
        self.db
            .table_counts
            .delete(&mut self.write_txn, &table.id)?;

        // 3. キャッシュから除去。
        self.table_caches
            .remove(&(db_meta.id, table_name.to_string()));

        Ok(())
    }
}
