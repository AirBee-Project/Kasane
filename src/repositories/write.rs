use kasane_logic::{IntoSingleIds, SpatialIdSet};
use redb::{ReadableTable, WriteTransaction};

use crate::{
    db_init::{TABLE_IDS, TABLE_IDS_KEY, TABLES},
    error::AppError,
    models::table::{Table, TableDataType, TableMetadata},
};

pub struct SpatialDbWrite {
    write_txn: WriteTransaction,
}

impl SpatialDbWrite {
    /// [SpatialDbWrite]のインスタンスを作成する
    pub fn new(write_txn: WriteTransaction) -> Self {
        Self { write_txn }
    }

    ///Tableの情報を取得する
    pub fn table_info(&self, name: &str) -> Result<Option<Table>, AppError> {
        let redb_tables = self.write_txn.open_table(TABLES)?;
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

    ///KasaneのTableを作成する
    ///既存のTableとの重複確認は行わない
    pub fn table_create(
        &self,
        name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
    ) -> Result<Table, AppError> {
        let id = self.increment_table_id()?;
        let meta = TableMetadata {
            id,
            data_type: data_type.clone(),
            max_zoom_level,
        };
        let mut redb_tables = self.write_txn.open_table(TABLES)?;
        let _ = redb_tables.insert(name, meta)?;

        Ok(Table {
            id,
            name: name.to_string(),
            data_type,
            max_zoom_level,
        })
    }

    /// Tableを削除する
    /// Tableの存在確認は行わない
    pub fn table_remove(&self, name: &str) -> Result<(), AppError> {
        let mut redb_tables = self.write_txn.open_table(TABLES)?;
        let removed = redb_tables.remove(name)?;
        if removed.is_none() {
            return Err(AppError::TableNotFound {
                name: name.to_string(),
            });
        }
        Ok(())
    }

    /// 空間IDに対して値を割り当てる
    /// そこに値がある場合は上書きされる
    pub fn value_insert(
        &self,
        table_id: u64,
        ids: SpatialIdSet,
        value: &[u8],
    ) -> Result<(), AppError> {
        for ele in ids.into_single_ids() {
            println!("{},", ele,)
        }
        println!("Value Insert Request");
        Ok(())
    }

    ///次のTableに対して割り当てるIDを返す
    fn increment_table_id(&self) -> Result<u64, AppError> {
        let mut redb_ids = self.write_txn.open_table(TABLE_IDS)?;
        let current_id = match redb_ids.get(TABLE_IDS_KEY)? {
            Some(id) => id.value(),
            None => 0,
        };
        let _ = redb_ids.insert(TABLE_IDS_KEY, current_id + 1)?;
        Ok(current_id)
    }

    /// 変更の内容を永続化する
    pub fn commit(self) -> Result<(), AppError> {
        self.write_txn.commit()?;
        Ok(())
    }
}
