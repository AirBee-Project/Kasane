use crate::io::wasm::Storage;

use crate::{
    interface::output::{Output, ShowValues, SpaceTimeIDOutput, Value},
    location,
    user_error::UserError,
};

impl Storage {
    pub fn show_values(&self, key_name: String) -> Result<Output, UserError> {
        match self.inner.get(&key_name) {
            Some((_, id_map)) => {
                let mut values = vec![];

                for (encode_id, value_entry) in id_map.iter() {
                    values.push(Value {
                        id: SpaceTimeIDOutput::from(encode_id.decode()),
                        value: value_entry,
                    });
                }

                Ok(Output::ShowValues(ShowValues { values }))
            }
            None => Err(UserError::KeyNotFound {
                key_name,
                location: location!(),
            }),
        }
    }
}
