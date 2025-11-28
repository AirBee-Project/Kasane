use crate::io::wasm::Storage;

use kasane_logic::space_time_id::encode;

use crate::{
    interface::{
        input::{Range, ValueEntry},
        output::Output,
    },
    location,
    user_error::UserError,
};

use std::collections::hash_map::Entry::{Occupied, Vacant};

impl Storage {
    pub fn show_values(&mut self, key_name: String) -> Result<Output, UserError> {
        todo!()
    }
}
