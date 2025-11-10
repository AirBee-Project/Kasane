use std::collections::HashSet;

use redb::{ReadableMultimapTable, ReadableTable};

use crate::{
    io::full::{SpaceKeyTableValue, Storage, SPACE_KEY_TABLE, SPACE_TABLE},
    json::{
        input::{KeyMode, KeyType},
        output::Output,
    },
    location,
    r#type::uuid::UuidKey,
    user_error::UserError,
};

impl Storage {
    pub fn create_key(
        &self,
        space_name: &str,
        key_name: &str,
        key_type: KeyType,
        key_mode: KeyMode,
    ) -> Result<Output, UserError> {
        let write_txn = self.db.begin_write()?;

        {
            let mut table_space = write_txn.open_table(SPACE_TABLE)?;
            let mut table_space_key = write_txn.open_table(SPACE_KEY_TABLE)?;

            //既存のSpaceのチェック
            if table_space.get_mut(space_name)?.is_some() {
                return Err(UserError::SpaceAlreadyExists {
                    space_name: space_name.to_string(),
                    location: location!(),
                });
            }

            let space_id = loop {
                let id = UuidKey::new_v4();
                // パスワードテーブルで既にこのIDが使われていないか確認
                if table_space_key.get(id)?.is_none() {
                    break id;
                }
            };

            let hash_set: SpaceKeyTableValue = SpaceKeyTableValue(HashSet::new());

            table_space.insert(space_name, space_id);
            table_space_key.insert(space_id, hash_set);
        }
        write_txn.commit()?;
        Ok(Output::Success)
    }
}
