use kasane_logic::{SingleId, SpatialIdSet};
use redb::{ReadTransaction, ReadableTable};

use crate::{db_init::TABLES, error::AppError, models::table::Table};

pub struct SpatialDbRead {
    read_txn: ReadTransaction,
}

impl SpatialDbRead {
    /// [SpatialDbRead]のインスタンスを作成する
    pub fn new(read_txn: ReadTransaction) -> Self {
        Self { read_txn }
    }

    /// Tableの情報を取得する
    pub fn table_info(&self, name: &str) -> Result<Option<Table>, AppError> {
        let redb_tables = self.read_txn.open_table(TABLES)?;
        if let Some(meta_data) = redb_tables.get(name)? {
            let m = meta_data.value();
            Ok(Some(Table {
                id: m.id,
                name: name.to_string(),
                data_type: m.data_type,
                max_zoom_level: m.max_zoom_level,
            }))
        } else {
            Ok(None)
        }
    }

    /// Tableの一覧を取得する
    pub fn table_list(&self) -> Result<Vec<Table>, AppError> {
        self.read_txn
            .open_table(TABLES)?
            .iter()?
            .map(|res| {
                let (k, v) = res.map_err(AppError::from)?;
                let m = v.value();
                Ok(Table {
                    id: m.id,
                    name: k.value().to_owned(),
                    data_type: m.data_type,
                    max_zoom_level: m.max_zoom_level,
                })
            })
            .collect()
    }

    //Todo
    pub fn value_get(
        &self,
        _table_id: u64,
        _ids: SpatialIdSet,
    ) -> Result<Vec<(SingleId, &[u8])>, AppError> {
        Ok(vec![])
    }
}
