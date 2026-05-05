use kasane_logic::{FlexId, SpatialIdSet};
use redb::ReadTransaction;

use crate::{db_init::TABLES, error::AppError, models::table::entity::TableMetadata};

pub struct SpatialDbRead {
    read_txn: ReadTransaction,
}

impl SpatialDbRead {
    /// [SpatialDbRead]のインスタンスを作成する
    pub fn new(read_txn: ReadTransaction) -> Self {
        Self { read_txn: read_txn }
    }

    /// Tableの情報を取得する
    pub fn table_info(&self, name: &str) -> Result<Option<TableMetadata>, AppError> {
        let redb_tables = self.read_txn.open_table(TABLES)?;
        if let Some(meta_data) = redb_tables.get(name)? {
            Ok(Some(meta_data.value().clone()))
        } else {
            Ok(None)
        }
    }

    //Todo
    pub fn spatial_get(
        &self,
        table_name: &str,
        ids: SpatialIdSet,
    ) -> Result<impl Iterator<Item = (FlexId, &[u8])>, AppError> {
        Ok(std::iter::empty::<(FlexId, &'static [u8])>())
    }
}
