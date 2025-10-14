use lmdb::{Cursor, Transaction as _, WriteFlags};
use uuid::Uuid;

use crate::{
    io::{
        StorageTrait,
        full::{Storage, tools::value_entry::ValueEntry},
    },
    json::{
        input::{KeyMode, KeyType},
        output::Output,
    },
    r#type::spacetimeid::SpaceTimeId,
    user_error::UserError,
};

impl StorageTrait for Storage {
    fn insert_value(
        &self,
        space_name: &str,
        key_name: &str,
        ids: Vec<SpaceTimeId>,
        value: ValueEntry,
    ) -> Result<Output, UserError> {
        let location = location!();
        let space_id = Self::get_space_id(&self, space_name)?;

        let key_id = Self::get_key_id(&self, &space_id, key_name)?;
        let txn_ro = self.env.begin_ro_txn()?;
        let mut cursor = txn_ro.open_ro_cursor(self.key)?;

        let mut key_type_opt: Option<KeyType> = None;
        let mut key_mode_opt: Option<KeyMode> = None;

        for (k_bytes, _v_bytes) in cursor.iter_start() {
            if k_bytes.starts_with(&space_id)
                && k_bytes.len() >= space_id.len() + key_name.len() + 2
            {
                let key_name_bytes = &k_bytes[space_id.len()..k_bytes.len() - 2];
                if key_name_bytes == key_name.as_bytes() {
                    key_type_opt = Some(KeyType::from_byte(k_bytes[k_bytes.len() - 2])?);
                    key_mode_opt = Some(KeyMode::from_byte(k_bytes[k_bytes.len() - 1])?);
                    break;
                }
            }
        }

        let key_type = key_type_opt.ok_or(UserError::UnKnown {
            message: format!("Key '{}' not found in space '{}'", key_name, space_name),
            location,
        })?;
        let key_mode = key_mode_opt.ok_or(UserError::UnKnown {
            message: format!("KeyMode not found for key '{}'", key_name),
            location: location!(),
        })?;

        if !value.matches_keytype(&key_type) {
            return Err(UserError::UnKnown {
                message: format!(
                    "Type mismatch: key '{}' expects {:?}, but got {:?}",
                    key_name, key_type, value
                ),
                location: location!(),
            });
        }

        let value_id: [u8; 16] = *Uuid::new_v4().as_bytes();
        let mut txn = self.env.begin_rw_txn()?;

        struct Range {
            f: (i32, i32),
            x: (u32, u32),
            y: (u32, u32),
            t: (u64, u64),
        }

        for id in ids {
            let edge_iter = (0..=id.z()).filter_map(|z| {
                id.top(z).ok().map(|top_id| Range {
                    f: (top_id.f_start(), top_id.f_end()),
                    x: (top_id.x_start(), top_id.x_end()),
                    y: (top_id.y_start(), top_id.y_end()),
                    t: (
                        (top_id.t_start() * id.i()).into(),
                        (top_id.t_end() * id.i()).into(),
                    ),
                })
            });

            let mut n: u8 = 0;
            for z in edge_iter {
                let mut key_base = Vec::new();
                key_base.extend_from_slice(&key_id);
                key_base.push(n);

                // --- 4次元のバイト列を作る
                let dimensions: [(Vec<u8>, Vec<u8>); 4] = [
                    (z.f.0.to_be_bytes().to_vec(), z.f.1.to_be_bytes().to_vec()),
                    (z.x.0.to_be_bytes().to_vec(), z.x.1.to_be_bytes().to_vec()),
                    (z.y.0.to_be_bytes().to_vec(), z.y.1.to_be_bytes().to_vec()),
                    (z.t.0.to_be_bytes().to_vec(), z.t.1.to_be_bytes().to_vec()),
                ];

                // --- 範囲重複チェック（UniqueKey の場合のみ）
                if key_mode == KeyMode::UniqueKey {
                    for (dim_idx, (start_bytes, end_bytes)) in dimensions.iter().enumerate() {
                        let mut key_check = key_base.clone();
                        key_check.push(dim_idx as u8);
                        key_check.extend_from_slice(start_bytes);
                        key_check.extend_from_slice(end_bytes);

                        if let Ok(existing) = txn.get(self.value, &key_check) {
                            // 既存 Range があれば部分重複判定
                            // ここでは既存値は value_id または 0 だが、Range はキー自体に含まれている
                            return Err(UserError::UnKnown {
                                message: "Range overlap detected for UniqueKey".to_string(),
                                location: location!(),
                            });
                        }
                    }
                }

                // --- LMDB に挿入
                for (dim_idx, (start_bytes, end_bytes)) in dimensions.iter().enumerate() {
                    let mut key = key_base.clone();
                    key.push(dim_idx as u8);
                    key.extend_from_slice(start_bytes);
                    key.extend_from_slice(end_bytes);

                    let val_to_insert = if n == id.z() {
                        &value_id.to_vec()
                    } else {
                        &[0].to_vec()
                    };
                    txn.put(self.value, &key, val_to_insert, WriteFlags::empty())?;
                }

                n += 1;
            }
        }

        txn.commit()?;
        Ok(Output::Success)
    }
}
