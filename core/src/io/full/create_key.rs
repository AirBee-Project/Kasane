use crate::{
    io::full::Storage,
    json::{
        input::{KeyMode, KeyType},
        output::Output,
    },
    user_error::UserError,
};
use sled::transaction::{Transactional, abort};

impl Storage {
    pub fn create_key(
        &self,
        space_name: &str,
        key_name: &str,
        key_type: KeyType,
        key_mode: KeyMode,
    ) -> Result<Output, UserError> {
        // space key を作成
        let space_bytes = space_name.as_bytes().to_vec();

        let result = (&self.space, &self.key).transaction(|(tx_space, tx_key)| {
            let space_id = tx_space
                .get(&space_bytes)?
                .ok_or(UserError::SpaceNotFound {
                    space_name: space_name.to_owned(),
                    location: location!(),
                })?;

            let mut key_bytes = vec![];
            key_bytes.extend_from_slice(&space_id);
            key_bytes.extend_from_slice(key_name.as_bytes());

            if tx_key.get(&key_bytes)?.is_some() {
                abort(UserError::KeyAlreadyExists {
                    space_name: space_name.to_owned(),
                    key_name: key_name.to_owned(),
                    location: location!(),
                })?;
            }

            let key_id: u64 = tx_key.generate_id()?;

            // 値を構築（key_id + key_type + key_mode）
            let mut value_bytes = key_id.to_be_bytes().to_vec();
            value_bytes.extend_from_slice(key_type.as_bytes());
            value_bytes.extend_from_slice(key_mode.as_bytes());

            tx_key.insert(key_bytes, value_bytes)?;

            Ok(())
        });

        match result {
            Ok(_) => Ok(Output::Success),
            Err(e) => Err(e.into()),
        }
    }
}
