use crate::{
    interface::output::{Key, Output, Showkeys},
    io::wasm::Storage,
    user_error::UserError,
};

impl Storage {
    pub fn show_keys(&self) -> Result<Output, UserError> {
        let mut result = vec![];

        for (key_name, (key_type, _)) in &self.inner {
            result.push(Key {
                key_name: key_name.to_string(),
                key_type: key_type.clone(),
            });
        }

        return Ok(Output::Showkeys(Showkeys { key_names: result }));
    }
}
