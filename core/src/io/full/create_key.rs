use lmdb::Error;

use crate::{
    io::{StorageTrait, full::Storage},
    json::input::{KeyMode, KeyType},
    user_error::UserError,
};

impl StorageTrait for Storage {
    fn create_key(
        &self,
        spacename: &str,
        keyname: &str,
        keytype: KeyType,
        keymode: KeyMode,
    ) -> Result<crate::json::output::Output, UserError> {
        let space_bytes = spacename.as_bytes();
        let mut txn = self.env.begin_rw_txn()?;
        let space_uuid = match txn.get(self.space, &space_bytes) {
            Ok(v) => v,
            Err(LmdbError::NotFound) => {
                return Err(UserError::SpaceNotFound {
                    space_name: todo!(),
                    location: todo!(),
                });
            }
            Err(e) => return Err(Error::from(e)),
        };

        let key_id: [u8; 16] = *Uuid::new_v4().as_bytes();

        // バイト列形式: [space_uuid][keyname][keytype][keymode]
        let key_bytes = [
            &space_uuid[..],
            keyname.as_bytes(),
            &[keytype_id(keytype)],
            &[keymode as u8],
        ]
        .concat();

        txn.put(self.key, &key_bytes, &key_id, lmdb::WriteFlags::empty())
            .map_err(|e| match e {
                LmdbError::KeyExist => Error::KeyAlreadyExists {
                    space_name: spacename.to_string(),
                    key_name: keyname.to_string(),
                    location: "io::create_key",
                },
                _ => Error::from(e),
            })?;

        txn.commit()?;
        Ok(Output::Success)
    }
}
