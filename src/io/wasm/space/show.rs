use crate::{
    command::space,
    interface::output::{Output, ShowSpaces},
    io::wasm::Storage,
    user_error::UserError,
};

impl Storage {
    pub fn show_spaces(&self) -> Result<Output, UserError> {
        let mut space_names = vec![];
        for (space_name, _) in &self.inner {
            space_names.push(space_name.to_string());
        }
        Ok(Output::ShowSpaces(ShowSpaces { space_names }))
    }
}
