use crate::{
    io::full::{Storage, tools::value_entry::ValueEntry},
    json::output::Output,
    r#type::space_time_id::SpaceTimeId,
    user_error::UserError,
};

impl Storage {
    pub fn insert_value(
        &self,
        space_name: &str,
        key_name: &str,
        ids: Vec<SpaceTimeId>,
        value: ValueEntry,
    ) -> Result<Output, UserError> {
        todo!()
    }
}
