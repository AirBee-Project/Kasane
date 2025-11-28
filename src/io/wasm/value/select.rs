use kasane_logic::space_time_id::encode;

use crate::{
    interface::{
        input::{Range, SelectValue, ValueEntry},
        output::{
            Output::{self},
            Value,
        },
    },
    io::wasm::Storage,
    location,
    user_error::UserError,
};

use std::collections::hash_map::Entry::{Occupied, Vacant};

impl Storage {
    pub fn select_value(
        &mut self,
        key_names: Vec<String>,
        range: Range,
    ) -> Result<Output, UserError> {
        todo!()
    }
}
