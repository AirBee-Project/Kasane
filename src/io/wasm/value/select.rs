use crate::{
    interface::{
        input::Range,
        output::{KeyValues, Output, SelectValue, SpaceTimeIDOutput, Value},
    },
    io::wasm::Storage,
    location,
    user_error::UserError,
};

impl Storage {
    pub fn select_value(
        &self,
        key_names: Vec<String>,
        range: Range,
    ) -> Result<Output, UserError> {
        let encode_ids = Self::process_range(range)?;
        let mut key_values = vec![];

        for key_name in key_names {
            match self.inner.get(&key_name) {
                Some((_, id_map)) => {
                    let mut values = vec![];

                    for encode_id in encode_ids.iter() {
                        // Get all values matching this encode_id
                        for (matched_id, value_entry) in id_map.get(&encode_id) {
                            values.push(Value {
                                id: SpaceTimeIDOutput::from(matched_id.decode()),
                                value: value_entry,
                            });
                        }
                    }

                    key_values.push(KeyValues {
                        key_name,
                        values,
                    });
                }
                None => {
                    return Err(UserError::KeyNotFound {
                        key_name,
                        location: location!(),
                    });
                }
            }
        }

        Ok(Output::SelectValue(SelectValue { key_values }))
    }
}
