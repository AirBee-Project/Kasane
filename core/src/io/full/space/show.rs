use redb::{ReadableDatabase, ReadableTable};

use crate::{
    io::full::{Storage, SPACE_TABLE},
    json::output::{Output, ShowSpaces},
    user_error::UserError,
};

impl Storage {
    /// 登録されているすべての space 名を取得
    pub fn show_spaces(&self) -> Result<Output, UserError> {
        let mut spaces = Vec::new();

        let read_txn = self.db.begin_read()?;

        {
            let table_space = match read_txn.open_table(SPACE_TABLE) {
                Ok(v) => v,
                Err(e) => match e {
                    redb::TableError::TableDoesNotExist(_) => {
                        return Ok(Output::ShowSpaces(ShowSpaces {
                            space_names: spaces,
                        }));
                    }
                    e => return Err(e.into()),
                },
            };

            for space in table_space.iter()? {
                let (key, _) = space?;
                spaces.push(key.value().to_string());
            }
        }

        Ok(Output::ShowSpaces(ShowSpaces {
            space_names: spaces,
        }))
    }
}
