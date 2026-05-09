use redb::{AccessGuard, ReadableTable};

use crate::{
    db_init::{ID_TO_VALUE, VALUE_TO_ID},
    error::AppError,
    repositories::layer::write::SpatialDbWrite,
};

impl SpatialDbWrite {
    ///valueを挿入し、ポインタとなるIDを出力する
    pub(self) fn value_insert(&self, layer_id: uuid::Uuid, value: &[u8]) -> Result<u64, AppError> {
        let mut redb_value_to_id = self.write_txn.open_table(VALUE_TO_ID)?;

        // 既に同じ値が存在した場合
        if let Some(access_guard) = redb_value_to_id.get((layer_id.into_bytes(), value))? {
            return Ok(access_guard.value());
        }

        let mut redb_id_to_value = self.write_txn.open_table(ID_TO_VALUE)?;

        // 新規の値の場合
        let id = self.increment_value_id(layer_id)?;
        redb_value_to_id.insert((layer_id.into_bytes(), value), id)?;
        redb_id_to_value.insert((layer_id.into_bytes(), id), value)?;
        Ok(id)
    }

    ///valueを削除し、value_idを開放する
    pub fn value_remove(&self, layer_id: uuid::Uuid, value_id: u64) -> Result<Option<Vec<u8>>, AppError> {
        let mut redb_id_to_value = self.write_txn.open_table(ID_TO_VALUE)?;
        let mut redb_value_to_id = self.write_txn.open_table(VALUE_TO_ID)?;

        // 値が存在している場合
        if let Some(access_guard) = redb_id_to_value.remove((layer_id.into_bytes(), value_id))? {
            let value = access_guard.value();
            redb_value_to_id.remove((layer_id.into_bytes(), value))?;
            return Ok(Some(value.to_vec()));
        }

        return Ok(None);
    }

    /// value_idからvalueの実体を取得する
    pub fn value_get(&self, layer_id: uuid::Uuid, value_id: u64) -> Result<Option<Vec<u8>>, AppError> {
        let redb_id_to_value = self.write_txn.open_table(ID_TO_VALUE)?;
        if let Some(access_guard) = redb_id_to_value.get((layer_id.into_bytes(), value_id))? {
            return Ok(Some(access_guard.value().to_vec()));
        }
        Ok(None)
    }

    /// 新規のValueに割り当てるIDを返す
    pub fn increment_value_id(&self, layer_id: uuid::Uuid) -> Result<u64, AppError> {
        todo!()
    }
}
