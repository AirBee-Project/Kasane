use crate::{
    error::Error,
    io::on_disk::OnDiskWriteTx,
    transaction::models::{KeyType, Range},
};

pub trait WriteTxTrait {
    fn create_key(&mut self, key_name: &str, key_type: KeyType) -> Result<(), Error>;
    fn drop_key(&mut self, key_name: &str) -> Result<(), Error>;
    // fn delete_value(&mut self, key_name: &str, range: Range);
    // fn insert_value<T>(&mut self, key_name: &str, value: T, range: Range);
    fn commit(self) -> Result<(), Error>;
    fn rollback(self) -> Result<(), Error>;
}

impl WriteTxTrait for OnDiskWriteTx {
    fn create_key(&mut self, key_name: &str, key_type: KeyType) -> Result<(), Error> {
        todo!()
    }

    fn drop_key(&mut self, key_name: &str) -> Result<(), Error> {
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
