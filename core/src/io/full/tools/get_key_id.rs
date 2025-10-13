use crate::{io::full::Storage, user_error::UserError};
use lmdb::{Cursor, Error as LmdbError, Transaction as _};

impl Storage {
    pub fn get_key_id(&self, space_id: Vec<u8>, key_name: &str) -> Result<Vec<u8>, UserError> {
        let location = location!();
        let txn = self.env.begin_ro_txn()?; // 読み取り専用

        let mut cursor = txn.open_ro_cursor(self.key)?;

        for (k_bytes, v_bytes) in cursor.iter_start() {
            // キーが対象の space_id で始まり、key_name が一致するかチェック
            if k_bytes.starts_with(&space_id)
                && k_bytes.len() >= space_id.len() + key_name.len() + 2
            {
                let key_name_bytes = &k_bytes[space_id.len()..k_bytes.len() - 2]; // key_typeとkey_modeを除く
                if key_name_bytes == key_name.as_bytes() {
                    return Ok(v_bytes.to_vec()); // LMDB の value を返す
                }
            }
        }

        Err(UserError::UnKnown {
            message: format!("Key '{}' not found in the specified space", key_name),
            location,
        })
    }
}
