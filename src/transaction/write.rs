#[cfg(feature = "on_disk")]
use redb::ReadableTable;

use crate::{
    error::{DataCorruptionKind, Error},
    io::{
        models::FieldDef,
        on_disk::{OnDiskWriteTx, FIELD_TABLE, META_FIELD_ID, META_TABLE},
    },
    location,
    transaction::models::{FieldType, Range},
};

pub trait WriteTxTrait {
    fn create_field(&mut self, field_name: &str, key_type: FieldType) -> Result<(), Error>;
    fn drop_field(&mut self, field_name: &str) -> Result<(), Error>;
    fn delete_value(&mut self, field_name: &str, range: Range);
    fn insert_value<T>(&mut self, field_name: &str, value: T, range: Range);
    fn commit(self) -> Result<(), Error>;
    fn rollback(self) -> Result<(), Error>;
}

#[cfg(feature = "on_disk")]
impl WriteTxTrait for OnDiskWriteTx {
    fn create_field(&mut self, field_name: &str, field_type: FieldType) -> Result<(), Error> {
        // WriteTransaction は self.inner
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

        // 次に割り当てる FieldID を取得
        let next_field_id = match meta_table.get(META_FIELD_ID)? {
            Some(v) => v.value() + 1,
            None => {
                return Err(Error::DataCorruption {
                    location: location!(),
                    kind: DataCorruptionKind::MissingMetadata,
                })
            }
        };

        // META_TABLE を更新（次の ID を保存）
        meta_table.insert(META_FIELD_ID, next_field_id)?;

        // FieldDef を作成して FIELD_TABLE に挿入
        let field_info = FieldDef {
            type_u8: field_type.into(),
            id: next_field_id,
        };
        field_table.insert(field_name.to_string(), field_info)?;

        Ok(())
    }

    fn drop_field(&mut self, field_name: &str) -> Result<(), Error> {
        todo!()
    }

    fn delete_value(&mut self, field_name: &str, range: Range) {
        todo!()
    }

    fn insert_value<T>(&mut self, field_name: &str, value: T, range: Range) {
        todo!()
    }

    fn commit(self) -> Result<(), Error> {
        Ok(self.inner.commit()?)
    }

    fn rollback(self) -> Result<(), Error> {
        let _ = self.inner.abort();
        Ok(())
    }
}
