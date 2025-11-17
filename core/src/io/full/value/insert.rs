use crate::{
    io::full::{command_helpers::value_entry::ValueEntry, Storage},
    json::output::Output,
    user_error::UserError,
};

impl Storage {
    pub fn insert_value(
        &self,
        space_name: &str,
        key_name: &str,
        // ids: Vec<SpaceTimeId>,
        value: ValueEntry,
    ) -> Result<Output, UserError> {
        todo!()
    }
}
