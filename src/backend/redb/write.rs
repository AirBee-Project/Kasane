use kasane_logic::{SetOnMemory, TableOnMemory};
use redb::ReadableTable;

use crate::{
    backend::{
        redb::{RedbWriteTx, FIELD, META},
        ReadTransaction, WriteTransaction,
    },
    Error,
};

impl WriteTransaction for RedbWriteTx {
    fn create_field(&mut self, field_name: String) -> Result<(), Error> {
        let mut field_table = self.0.open_table(FIELD)?;
        let mut meta = self.0.open_table(META)?;
        if field_table.get(&field_name)?.is_some() {
            return Err(Error::FieldAlreadyExists(field_name));
        }
        let next_id = meta
            .get("next_field_id")?
            .ok_or_else(|| Error::Serialization("missing next_field_id".into()))?
            .value();
        let new_id = next_id;
        meta.insert("next_field_id", new_id + 1)?;
        field_table.insert(field_name, new_id)?;
        Ok(())
    }

    fn drop_field(&mut self, field_name: String) -> Result<(), Error> {
        todo!()
    }
    fn insert(
        &mut self,
        field_name: String,
        range: SetOnMemory,
        value: &[u8],
    ) -> Result<(), Error> {
        todo!()
    }

    fn commit(self) -> Result<(), Error>
    where
        Self: Sized,
    {
        self.0.commit()?;
        Ok(())
    }

    fn rollback(self) -> Result<(), Error>
    where
        Self: Sized,
    {
        self.0.abort()?;
        Ok(())
    }
}

impl ReadTransaction for RedbWriteTx {
    fn show_fields(&self) -> Result<Vec<String>, Error> {
        let field_table = self.0.open_table(FIELD)?;
        let mut fields = Vec::new();
        for entry in field_table.iter()? {
            let (name, _id) = entry?;
            fields.push(name.value().to_owned());
        }
        Ok(fields)
    }

    fn get(&self, field_name: String, range: SetOnMemory) -> TableOnMemory<&[u8]> {
        todo!()
    }
}
