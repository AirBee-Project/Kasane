use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;

use crate::interface::output::Output;
use crate::location;
use crate::{command::space, io::wasm::Storage, user_error::UserError};

impl Storage {
    pub fn create_space(&mut self, space_name: String) -> Result<Output, UserError> {
        match self.inner.entry(space_name.clone()) {
            Vacant(entry) => {
                entry.insert(HashMap::new());
                Ok(Output::Success)
            }
            Occupied(_) => {
                // キーが既に存在する場合はエラー
                Err(UserError::SpaceAlreadyExists {
                    space_name,
                    location: location!(),
                })
            }
        }
    }
}
