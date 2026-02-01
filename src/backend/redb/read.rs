use kasane_logic::{SetOnMemory, TableOnMemory};
use redb::ReadableTable;

use crate::{
    backend::{
        redb::{RedbReadTx, FIELD},
        ReadTransaction,
    },
    Error,
};

impl ReadTransaction for RedbReadTx {
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
