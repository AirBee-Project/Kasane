use kasane_logic::{FlexId, SingleId};
use redb::{ReadableTable, WriteTransaction};

use crate::{
    db_init::{RANKS, RANKS_KEY_TABLE, TABLES},
    error::AppError,
    models::table::{TableDataType, entity::TableMetadata},
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
    pub fn table_info(&self, name: &str) -> Result<Option<TableMetadata>, AppError> {
        let redb_tables = self.write_txn.open_table(TABLES)?;
        if let Some(meta_data) = redb_tables.get(name)? {
            Ok(Some(meta_data.value().clone()))
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
    ) -> Result<(), AppError> {
        let rank = self.increment_table_rank()?;
        let meta_data = TableMetadata {
            rank,
            r#type: data_type,
            max_zoom_level,
        };
        let mut redb_tables = self.write_txn.open_table(TABLES)?;
        let _ = redb_tables.insert(name, meta_data)?;
        Ok(())
    }

    /// Tableを削除する
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

    /// 時空間IDに対して値を割り当てる
    ///
    /// そこに値がある場合は上書きされる
    pub fn spatial_insert(&self, table_rank: u64, ids: &[SingleId], value: &[u8]) {
        for single_id in ids {
            //まず自分の親が値を持つかを調べる
            for parent in single_id.spatial_parents() {}
        }
    }

    ///次のTableに対して割り当てるRankを返す
    fn increment_table_rank(&self) -> Result<u64, AppError> {
        let mut redb_ranks = self.write_txn.open_table(RANKS)?;
        let current_rank = match redb_ranks.get(RANKS_KEY_TABLE)? {
            Some(rank) => rank.value(),
            None => 0,
        };
        let _ = redb_ranks.insert(RANKS_KEY_TABLE, current_rank + 1)?;
        Ok(current_rank)
    }

    /// 変更の内容を永続化する
    pub fn commit(self) -> Result<(), AppError> {
        self.write_txn.commit()?;
        Ok(())
    }
}
