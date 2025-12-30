use redb::ReadableTable;

use crate::{
    error::Error,
    io::on_disk::{OnDiskReadTx, FIELD_TABLE},
    transaction::models::Range,
};

pub trait ReadTxTrait {
    fn show_fields(&self) -> Result<Box<dyn Iterator<Item = Result<String, Error>> + '_>, Error>;

    fn show_values(&self) -> Result<(), Error>;
    fn select_value(&self, key_name: &str, range: Range) -> Result<Error, ()>;
    fn close(self) -> Result<(), Error>;
}

impl ReadTxTrait for OnDiskReadTx {
    fn show_fields(&self) -> Result<Box<dyn Iterator<Item = Result<String, Error>> + '_>, Error> {
        let field_table = self.inner.open_table(FIELD_TABLE)?;

        //todo

        todo!()
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
