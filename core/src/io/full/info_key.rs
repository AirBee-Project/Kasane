use crate::{
    io::{StorageTrait, full::Storage},
    json::{
        input::{KeyMode, KeyType},
        output::{InfoKey, Output},
    },
    user_error::UserError,
};
use lmdb::{Cursor, Transaction as _};

impl StorageTrait for Storage {
    fn info_key(&self, space_name: &str, key_name: &str) -> Result<Output, UserError> {
        let space_id = Self::get_space_id(&self, space_name)?;
        let _key_id = Self::get_key_id(&self, space_id.clone(), key_name)?;

        // 読み取り専用トランザクション
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.key)?;

        for (k_bytes, _v_bytes) in cursor.iter_start() {
            if k_bytes.starts_with(&space_id)
                && k_bytes.len() >= space_id.len() + key_name.len() + 2
            {
                let key_name_bytes = &k_bytes[space_id.len()..k_bytes.len() - 2];
                if key_name_bytes == key_name.as_bytes() {
                    let key_type_byte = k_bytes[k_bytes.len() - 2];
                    let key_mode_byte = k_bytes[k_bytes.len() - 1];

                    let key_type_str = KeyType::from_byte(key_type_byte)?.as_str().to_string();
                    let key_mode_str = KeyMode::from_byte(key_mode_byte)?.as_str().to_string();

                    let info_key = InfoKey {
                        key_name: key_name.to_string(),
                        key_type: key_type_str,
                        key_mode: key_mode_str,
                    };

                    return Ok(Output::InfoKey(info_key));
                }
            }
        }

        Err(UserError::UnKnown {
            message: format!("Key '{}' not found in space '{}'", key_name, space_name),
            location: location!(),
        })
    }
}
