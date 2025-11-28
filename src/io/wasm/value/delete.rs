use crate::{interface::input::Range, io::wasm::Storage};
use std::collections::hash_map::Entry::{Occupied, Vacant};

use crate::{interface::output::Output, location, user_error::UserError};

impl Storage {
    pub fn delete_value(&mut self, key_name: String, range: Range) -> Result<Output, UserError> {
        match self.inner.entry(key_name.clone()) {
            Occupied(mut entry) => {
                let encode_ids = Self::process_range(range)?;

                let (_, set) = entry.get_mut();

                for encode_id in encode_ids.iter() {
                    set.remove(encode_id);
                }
                Ok(Output::Success)
            }
            Vacant(_) => {
                return Err(UserError::KeyNotFound {
                    key_name,
                    location: location!(),
                });
            }
        }
    }
}
