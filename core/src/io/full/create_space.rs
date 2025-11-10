use std::collections::HashSet;

use redb::{ReadableMultimapTable, ReadableTable};

use crate::{
    io::full::{kv_type::uuid::UuidKey, SpaceKeyTableValue, Storage, SPACE_TABLE},
    json::output::Output,
    location,
    user_error::UserError,
};

impl Storage {
    pub fn create_space(&self, space_name: &str) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table_space = write_txn.open_table(SPACE_TABLE)?;

            if table_space.get(space_name)?.is_some() {
                return Err(UserError::SpaceAlreadyExists {
                    space_name: space_name.to_string(),
                    location: location!(),
                });
            }

            let space_id = UuidKey::new();

            table_space.insert(space_name, space_id);
        }
        write_txn.commit()?;
        Ok(Output::Success)
    }
}
