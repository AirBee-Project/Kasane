use crate::{interface::output::Output, io::wasm::Storage, user_error::UserError};

impl Storage {
    pub fn info_space(&self, space_name: String) -> Result<Output, UserError> {
        todo!()
    }
}
