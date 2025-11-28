use std::collections::hash_map::Entry::{Occupied, Vacant};

use kasane_logic::encode_id_map::EncodeIDMap;

use crate::{
    interface::{input::KeyType, output::Output},
    io::wasm::Storage,
    location,
    user_error::UserError,
};

impl Storage {
    pub fn create_key(&mut self, key_name: String, key_type: KeyType) -> Result<Output, UserError> {
        match self.inner.entry(key_name.clone()) {
            Occupied(_) => Err(UserError::KeyAlreadyExists {
                key_name: key_name,
                location: location!(),
            }),
            Vacant(entry) => {
                entry.insert((key_type, EncodeIDMap::new()));
                return Ok(Output::Success);
            }
        }
    }
}
