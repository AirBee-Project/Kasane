use redb::ReadableTable;
use uuid::Uuid;

use crate::{
    db_init::{DATABASES, SPATIALID_TO_VALUE, TABLE_ID_INDEX, TABLES, VALUE_TO_SPATIALID},
    error::AppError,
    models::database::table::{Table, TableDataType, TableMetadata},
    repositories::KasaneDbWrite,
};

impl KasaneDbWrite {
    /// Tableの情報を取得する
    pub fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        // DBメタデータ取得
        let db_meta = {
            let redb_dbs = self.write_txn.open_table(DATABASES)?;
            if let Some(m) = redb_dbs.get(db_name)? {
                m.value()
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        // キャッシュ検索
        if let Some(meta_data) = self.table_caches.get(&(db_meta.id, table_name.to_string())) {
            return Ok(Some(Table {
                id: meta_data.id,
                name: table_name.to_string(),
                data_type: meta_data.data_type,
                max_zoom_level: meta_data.max_zoom_level,
            }));
        }

        let redb_tables = self.write_txn.open_table(TABLES)?;
        if let Some(meta_data) = redb_tables.get((db_meta.id.into_bytes(), table_name))? {
            let m = meta_data.value();
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
        let db_meta = {
            let redb_dbs = self.write_txn.open_table(DATABASES)?;
            if let Some(m) = redb_dbs.get(db_name)? {
                m.value()
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

        let mut redb_table_ids = self.write_txn.open_table(TABLE_ID_INDEX)?;

        let mut id = Uuid::now_v7();
        loop {
            if redb_table_ids.get(id.into_bytes())?.is_none() {
                break;
            }
            id = Uuid::now_v7();
        }

        let meta = TableMetadata {
            id,
            data_type,
            max_zoom_level,
        };

        let mut redb_tables = self.write_txn.open_table(TABLES)?;
        redb_tables.insert((db_meta.id.into_bytes(), table_name), meta)?;
        redb_table_ids.insert(id.into_bytes(), ())?;

        self.table_caches
            .insert((db_meta.id, table_name.to_string()), meta);

        Ok(Table {
            id,
            name: table_name.to_string(),
            data_type,
            max_zoom_level,
        })
    }

    /// Tableを削除する
    pub fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError> {
        let db_meta = {
            let redb_dbs = self.write_txn.open_table(DATABASES)?;
            if let Some(m) = redb_dbs.get(db_name)? {
                m.value()
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        let table_meta = if let Some(meta) = self.table_info(db_name, table_name)? {
            meta
        } else {
            return Err(AppError::TableNotFound {
                name: table_name.to_string(),
            });
        };

        let mut redb_tables = self.write_txn.open_table(TABLES)?;
        redb_tables.remove((db_meta.id.into_bytes(), table_name))?;

        let mut redb_table_ids = self.write_txn.open_table(TABLE_ID_INDEX)?;
        redb_table_ids.remove(table_meta.id.into_bytes())?;

        let table_id_bytes = table_meta.id.into_bytes();

        let mut redb_spatialid_to_value = self.write_txn.open_table(SPATIALID_TO_VALUE)?;
        let keys: Vec<_> = redb_spatialid_to_value
            .range((table_id_bytes, [0u8; 12])..=(table_id_bytes, [255u8; 12]))?
            .map(|res| res.map(|(k, _)| k.value()))
            .collect::<Result<Vec<_>, _>>()?;
        for key in keys {
            redb_spatialid_to_value.remove(&key)?;
        }

        let mut redb_value_to_spatialid = self.write_txn.open_table(VALUE_TO_SPATIALID)?;
        let keys: Vec<([u8; 16], Vec<u8>, [u8; 12])> = redb_value_to_spatialid
            .range((table_id_bytes, &[] as &[u8], [0u8; 12])..)?
            .map(|res| {
                res.map(|(k, _)| {
                    let (id, val, spatial) = k.value();
                    (id, val.to_vec(), spatial)
                })
            })
            .take_while(|res| match res {
                Ok((id, _, _)) => id[..] == table_id_bytes[..],
                Err(_) => true,
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (id, val, spatial) in keys {
            redb_value_to_spatialid.remove(&(id, val.as_slice(), spatial))?;
        }

        self.table_caches
            .remove(&(db_meta.id, table_name.to_string()));

        Ok(())
    }
}
