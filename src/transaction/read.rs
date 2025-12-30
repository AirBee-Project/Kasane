use crate::{
    error::Error,
    io::on_disk::OnDiskReadTx,
    transaction::models::{FieldType, Range},
};

use crate::io::on_disk::FIELD_TABLE;
use redb::ReadableTable;

pub struct Field {
    pub name: String,
    pub r#type: FieldType,
}

pub trait ReadTxTrait {
    fn show_fields(&self) -> Result<Box<dyn Iterator<Item = Result<Field, Error>> + '_>, Error>;

    fn show_values(&self) -> Result<(), Error>;
    fn select_value(&self, key_name: &str, range: Range) -> Result<Error, ()>;
    fn close(self) -> Result<(), Error>;
}

#[cfg(feature = "on_disk")]
impl ReadTxTrait for OnDiskReadTx {
    fn show_fields(&self) -> Result<Box<dyn Iterator<Item = Result<Field, Error>> + '_>, Error> {
        let table = self.inner.open_table(FIELD_TABLE)?;

        let mut fields = Vec::new();

        for entry in table.iter()? {
            let (name, field_def) = entry?;

            let field_type = FieldType::try_from(field_def.value().type_u8)?;

            fields.push(Ok(Field {
                name: name.value().to_owned(),
                r#type: field_type,
            }));
        }

        Ok(Box::new(fields.into_iter()))
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
