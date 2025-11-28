use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;

use crate::interface::output::Output;
use crate::location;
use crate::{command::space, io::wasm::Storage, user_error::UserError};

impl Storage {
    pub fn drop_space(&mut self, space_name: String) -> Result<Output, UserError> {
        match self.inner.entry(space_name.clone()) {
            Vacant(entry) => Err(UserError::SpaceNotFound {
                space_name,
                location: location!(),
            }),
            Occupied(entry) => {
                entry.remove_entry();
                Ok(Output::Success)
            }
        }
    }
}
