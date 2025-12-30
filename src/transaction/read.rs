use crate::{error::Error, io::on_disk::OnDiskReadTx, transaction::models::Range};

pub trait ReadTxTrait {
    fn show_keys(&self) -> Result<(), Error>;
    fn show_values(&self) -> Result<(), Error>;
    fn select_value(&self, key_name: &str, range: Range) -> Result<Error, ()>;
    fn close(self) -> Result<(), Error>;
}

impl ReadTxTrait for OnDiskReadTx {
    fn show_keys(&self) -> Result<(), Error> {
        todo!()
    }

    fn close(self) -> Result<(), Error> {
        Ok(self.inner.close()?)
    }

    fn show_values(&self) -> Result<(), Error> {
        todo!()
    }

    fn select_value(&self, key_name: &str, range: Range) -> Result<Error, ()> {
        todo!()
    }
}
