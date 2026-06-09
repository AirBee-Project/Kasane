use crate::{
    db_init::{DATABASES, TABLES},
    error::AppError,
    models::database::table::Table,
    repositories::KasaneDbRead,
};

impl KasaneDbRead {
    /// Tableの情報を取得する
    pub fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        let db_meta = {
            let redb_dbs = self.read_txn.open_table(DATABASES)?;
            if let Some(m) = redb_dbs.get(db_name)? {
                m.value()
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        let redb_tables = self.read_txn.open_table(TABLES)?;
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

    /// Tableの一覧を取得する
    pub fn table_list(&self, db_name: &str) -> Result<Vec<Table>, AppError> {
        let db_meta = {
            let redb_dbs = self.read_txn.open_table(DATABASES)?;
            if let Some(m) = redb_dbs.get(db_name)? {
                m.value()
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        let db_id_bytes = db_meta.id.into_bytes();

        self.read_txn
            .open_table(TABLES)?
            .range((db_id_bytes, "")..)?
            .take_while(|res| match res {
                Ok((k, _)) => k.value().0 == db_id_bytes,
                Err(_) => true,
            })
            .map(|res| {
                let (k, v) = res.map_err(AppError::from)?;
                let m = v.value();
                Ok(Table {
                    id: m.id,
                    name: k.value().1.to_owned(),
                    data_type: m.data_type,
                    max_zoom_level: m.max_zoom_level,
                })
            })
            .collect()
    }
}
