use crate::{
    io::{StorageTrait, full::Storage},
    json::output::{Output, Showkeys},
    user_error::UserError,
};
use lmdb::{Cursor, Transaction as _};

impl StorageTrait for Storage {
    fn show_keys(&self, space_name: &str) -> Result<Output, UserError> {
        // スペースIDを取得
        let space_id = Self::get_space_id(&self, space_name)?;

        // 読み取り専用トランザクション開始
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.key)?;

        let mut key_names = Vec::new();

        for (k_bytes, _v_bytes) in cursor.iter_start() {
            if k_bytes.starts_with(&space_id) && k_bytes.len() >= space_id.len() + 2 {
                // key_typeとkey_modeを除いた部分を key_name とする
                let key_name_bytes = &k_bytes[space_id.len()..k_bytes.len() - 2];

                let key_name_str =
                    std::str::from_utf8(key_name_bytes).map_err(|e| UserError::from(e))?;
                key_names.push(key_name_str.to_string());
            }
        }

        Ok(Output::Showkeys(Showkeys { key_names }))
    }
}
