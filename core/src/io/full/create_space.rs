use crate::{io::full::Storage, json::output::Output, user_error::UserError};
use sled::transaction::abort;

impl Storage {
    pub async fn create_space(&self, space_name: &str) -> Result<Output, UserError> {
        let space_bytes = space_name.as_bytes().to_vec();

        let result = self.space.transaction(|tx| {
            if tx.get(&space_bytes)?.is_some() {
                abort(UserError::SpaceAlreadyExists {
                    space_name: space_name.to_string(),
                    location: location!(),
                })?;
            }

            tx.insert(
                space_bytes.clone(),
                tx.generate_id()?.to_be_bytes().to_vec(),
            )?;
            Ok(())
        });

        match result {
            Ok(_) => Ok(Output::Success),
            Err(e) => Err(e.into()),
        }
    }
}
