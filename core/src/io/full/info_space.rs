use crate::{
    io::{StorageTrait, full::Storage},
    json::input::{KeyMode, KeyType},
    json::output::Output,
    user_error::UserError,
};
use lmdb::{Cursor, Transaction as _};

impl StorageTrait for Storage {
    fn info_space(&self, space_name: &str) -> Result<Output, UserError> {
        let space_id = Self::get_space_id(&self, space_name)?;

        // 読み取り専用トランザクション
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.key)?;

        let mut key_infos = Vec::new();

        for (k_bytes, _v_bytes) in cursor.iter_start() {
            if k_bytes.starts_with(&space_id) && k_bytes.len() >= space_id.len() + 2 {
                // key_name は space_id と key_type/key_mode を除いた部分
                let key_name_bytes = &k_bytes[space_id.len()..k_bytes.len() - 2];
                let key_name = std::str::from_utf8(key_name_bytes)
                    .map_err(|e| UserError::from(e))?
                    .to_string();

                // key_type と key_mode は最後の2バイト
                let key_type_byte = k_bytes[k_bytes.len() - 2];
                let key_mode_byte = k_bytes[k_bytes.len() - 1];

                let key_type_str = KeyType::from_byte(key_type_byte)?.as_str().to_string();
                let key_mode_str = KeyMode::from_byte(key_mode_byte)?.as_str().to_string();

                key_infos.push(crate::json::output::InfoKey {
                    key_name,
                    key_type: key_type_str,
                    key_mode: key_mode_str,
                });
            }
        }

        let info_space = crate::json::output::InfoSpace {
            space_name: space_name.to_string(),
            key_names: key_infos,
        };

        Ok(Output::InfoSpace(info_space))
    }
}
