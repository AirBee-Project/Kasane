use crate::{
    io::{StorageTrait, full::Storage},
    json::{input::DatabaseCommand, output::Output},
    user_error::UserError,
};

impl StorageTrait for Storage {
    fn grant_database(
        &self,
        user_name: &str,
        command: DatabaseCommand,
    ) -> Result<Output, UserError> {
    }
}
