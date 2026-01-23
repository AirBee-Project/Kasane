use redb::ReadableTable;

use crate::{
    error::{DataCorruptionKind, Error},
    io::on_disk::{OnDiskWriteTx, FIELD_TABLE, META_FIELD_ID, META_TABLE},
    location,
    transaction::models::Range,
};

pub trait WriteTxTrait {
    fn create_field(&mut self, field_name: &str) -> Result<(), Error>;
    fn drop_field(&mut self, field_name: &str) -> Result<(), Error>;
    fn delete_value(&mut self, field_name: &str, range: Range);
    fn insert_value<T>(&mut self, field_name: &str, value: &[u8], range: Range);
    fn commit(self) -> Result<(), Error>;
    fn rollback(self) -> Result<(), Error>;
}

impl WriteTxTrait for OnDiskWriteTx {
    fn create_field(&mut self, field_name: &str) -> Result<(), Error> {
        let write_txn = &self.inner;

        // FIELD_TABLE を開く
        let mut field_table = write_txn.open_table(FIELD_TABLE)?;

        // すでに存在するか確認
        if field_table.get(field_name.to_string())?.is_some() {
            return Err(Error::FieldAlreadyExists {
                field_name: field_name.to_owned(),
                location: location!(),
            });
        }

        // META_TABLE を開く
        let mut meta_table = write_txn.open_table(META_TABLE)?;

        // 次に割り当てる FieldID を取得（初期値 0）
        let next_field_id = match meta_table.get(META_FIELD_ID)? {
            Some(v) => v.value(),
            None => {
                return Err(Error::DataCorruption {
                    location: location!(),
                    kind: DataCorruptionKind::MissingMetadata,
                })
            }
        };

        // フィールドに ID を割り当て
        field_table.insert(field_name.to_string(), next_field_id)?;

        // 次の ID を保存
        meta_table.insert(META_FIELD_ID, next_field_id + 1)?;

        Ok(())
    }

    fn drop_field(&mut self, field_name: &str) -> Result<(), Error> {
        todo!()
    }

    fn delete_value(&mut self, field_name: &str, range: Range) {
        todo!()
    }

    fn commit(self) -> Result<(), Error> {
        Ok(self.inner.commit()?)
    }

    fn rollback(self) -> Result<(), Error> {
        let _ = self.inner.abort();
        Ok(())
    }

    fn insert_value<T>(&mut self, field_name: &str, value: &[u8], range: Range) {
        todo!()
    }
}
