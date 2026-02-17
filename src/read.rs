use redb::ReadableTable;

use crate::{FILED_DICTIONARY, error::Error};

pub struct ReadTx {
    pub tx: redb::ReadTransaction,
}

impl ReadTx {
    pub fn show_fields(&self) -> Result<Vec<String>, Error> {
        let filed_dictonary = self.tx.open_table(FILED_DICTIONARY)?;

        let mut result = Vec::new();

        for entry in filed_dictonary.iter()? {
            let (name, _) = entry?;
            result.push(name.value().to_string());
        }

        Ok(result)
    }
}
