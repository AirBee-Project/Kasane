use crate::interface::output::Output;
use crate::location;
use crate::{io::wasm::Storage, user_error::UserError};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::f32::consts::E;

impl Storage {
    pub fn drop_key(&mut self, key_name: String) -> Result<Output, UserError> {
        match self.inner.entry(key_name.clone()) {
            Occupied(entry) => {
                entry.remove();
                return Ok(Output::Success);
            }
            Vacant(entry) => {
                return Err(UserError::KeyNotFound {
                    key_name,
                    location: location!(),
                });
            }
        }
    }
}
