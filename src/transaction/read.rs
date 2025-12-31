use redb::ReadableTable;

use crate::{
    error::Error,
    io::on_disk::{OnDiskReadTx, FIELD_TABLE},
    transaction::models::Range,
};

pub trait ReadTxTrait {
    fn show_fields(&self) -> Result<Vec<String>, Error>;
    fn show_values(&self) -> Result<(), Error>;
    fn select_value(&self, key_name: &str, range: Range) -> Result<Error, ()>;
    fn close(self) -> Result<(), Error>;
}

impl ReadTxTrait for OnDiskReadTx {
    fn show_fields(&self) -> Result<Vec<String>, Error> {
        let field_table: redb::ReadOnlyTable<String, u64> = self.inner.open_table(FIELD_TABLE)?;

        field_table
            .iter()?
            .map(|entry| {
                let (key, _) = entry?;
                Ok(key.value().to_string())
            })
            .collect()
    }

    fn show_values(&self) -> Result<(), Error> {
        todo!()
    }

    fn select_value(&self, key_name: &str, range: Range) -> Result<Error, ()> {
        todo!()
    }

    fn close(self) -> Result<(), Error> {
        Ok(self.inner.close()?)
    }
}
