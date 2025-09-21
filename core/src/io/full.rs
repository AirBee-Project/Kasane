use std::{
    collections::{HashMap, HashSet},
    env,
    path::PathBuf,
};

use crate::{
    io::{
        StorageTrait, ValueEntry,
        tools::keytype_id::{id_keytype, keytype_id},
    },
    json::{
        input::{KeyMode, KeyType},
        output::{InfoKey, InfoSpace, InfoUser, Output, ShowUsers, Showkeys},
    },
};
use argon2::password_hash::PasswordHasher;
use argon2::{Argon2, PasswordHash, PasswordVerifier, password_hash::SaltString};
use kasane_logic::id::SpaceTimeId;
use lmdb::{Cursor, DatabaseFlags, Error as LmdbError, WriteFlags};
use rand::rngs::OsRng;

use super::Error;
use lmdb::{Database, Environment, Transaction};
use uuid::Uuid;

pub struct Storage {
    pub space: Database,
    pub key: Database,
    pub value: Database,
    pub user: Database,
    pub env: Environment,
}

impl From<lmdb::Error> for Error {
    fn from(e: lmdb::Error) -> Self {
        match e {
            lmdb::Error::MapFull => Error::LmdbMapFull {
                attempted_size: 0, // 必要に応じて Environment から取得して渡す
                location: "unknown",
            },
            lmdb::Error::NotFound => Error::LmdbDbNotFound {
                db_name: "unknown",
                location: "unknown",
            },
            _ => Error::LmdbError {
                message: e.to_string(),
                location: "unknown",
            },
        }
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Self {
        Error::ParseError {
            message: format!("Invalid UTF-8: {}", e),
            location: "unknown",
        }
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(e: std::string::FromUtf8Error) -> Self {
        Error::ParseError {
            message: format!("Invalid UTF-8 (from Vec<u8>): {}", e),
            location: "unknown",
        }
    }
}
use std::convert::TryFrom;

impl TryFrom<u8> for KeyMode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(KeyMode::UniqueKey),
            1 => Ok(KeyMode::MultiKey),
            _ => Err(()),
        }
    }
}

impl Storage {
    pub fn new(path: Option<PathBuf>) -> Result<Self, Error> {
        // LMDB 環境を作成
        let env = Environment::new()
            .set_max_dbs(10) // 名前付きDBの上限
            .set_map_size(1024 * 1024 * 1024) // 1GB
            .open(&path.unwrap_or(env::current_dir().unwrap()))?;

        // データベースを開く（なければ作成）
        let space = env.create_db(Some("space"), DatabaseFlags::empty())?;
        let key = env.create_db(Some("key"), DatabaseFlags::empty())?;
        let value = env.create_db(Some("value"), DatabaseFlags::empty())?;
        let user = env.create_db(Some("user"), DatabaseFlags::empty())?;

        let storage = Self {
            space,
            key,
            value,
            user,
            env,
        };

        // === 初回起動時の admin ユーザー作成 ===
        {
            let txn = storage.env.begin_ro_txn()?;
            let admin_exists = txn.get(storage.user, b"admin").is_ok();
            drop(txn);

            if !admin_exists {
                // デフォルトパスワードは "admin" にしておく
                // 必要なら env から読み込むことも可能
                storage.create_user("admin", "nekocute")?;
                println!(
                    "✔ 初回起動: admin ユーザーを作成しました (username=admin, password=admin)"
                );
            }
        }

        Ok(storage)
    }
}

impl StorageTrait for Storage {
    fn create_space(&self, spacename: &str) -> Result<Output, Error> {
        let space_id: [u8; 16] = *Uuid::new_v4().as_bytes();
        let space_bytes = spacename.as_bytes();
        let mut txn = self.env.begin_rw_txn()?;
        txn.put(
            self.space,
            &space_bytes,
            &space_id,
            lmdb::WriteFlags::empty(),
        )
        //既に同じ名前のSpaceが存在する場合にはエラーを返す
        .map_err(|e| match e {
            LmdbError::KeyExist => Error::SpaceAlreadyExists {
                space_name: spacename.to_owned(),
            },
            _ => Error::from(e),
        })?;
        txn.commit()?;
        Ok(Output::Success)
    }

    fn drop_space(&self, spacename: &str) -> Result<crate::json::output::Output, Error> {
        let space_bytes = spacename.as_bytes();
        let mut txn = self.env.begin_rw_txn()?;

        // 1. Space の削除
        txn.del(self.space, &space_bytes, None)
            .map_err(|e| match e {
                LmdbError::NotFound => Error::SpaceNotFound {
                    space_name: spacename.to_string(),
                },
                _ => Error::from(e),
            })?;

        // 2. Space に属するキーを先にコピー
        let mut keys_to_delete = Vec::new();
        {
            let mut cursor = txn.open_ro_cursor(self.key)?; // 読み取り専用カーソルで走査
            let prefix = space_bytes; // space UUID が key のプレフィックス
            for result in cursor.iter_start() {
                let (k, _) = result;
                if k.starts_with(prefix) {
                    keys_to_delete.push(k.to_vec()); // Vec にコピー
                }
            }
        }

        // 3. コピーしたキーを削除
        for k in keys_to_delete {
            txn.del(self.key, &k, None)?;
        }

        txn.commit()?;
        Ok(Output::Success)
    }

    fn info_space(&self, spacename: &str) -> Result<crate::json::output::Output, Error> {
        // 1. Space の存在確認
        let space_bytes = spacename.as_bytes();
        let txn = self.env.begin_ro_txn()?;
        let space_uuid = match txn.get(self.space, &space_bytes) {
            Ok(v) => std::str::from_utf8(v)?,
            Err(LmdbError::NotFound) => {
                return Err(Error::SpaceNotFound {
                    space_name: spacename.to_string(),
                });
            }
            Err(e) => return Err(Error::from(e)),
        };

        // 2. キーを走査してこのスペースに属するものだけ抽出
        let mut cursor = txn.open_ro_cursor(self.key)?;
        let mut keys_info: Vec<InfoKey> = Vec::new();
        let prefix = format!("{}:", space_uuid);

        for result in cursor.iter_start() {
            let (k, _v) = result;
            if k.starts_with(prefix.as_bytes()) {
                let key_str = std::str::from_utf8(k)?;
                let parts: Vec<&str> = key_str.split(':').collect();
                if parts.len() != 4 {
                    return Err(Error::ParseError {
                        message: format!("Invalid key format: {}", key_str),
                        location: "io::info_space",
                    });
                }

                keys_info.push(InfoKey {
                    keyname: parts[1].to_string(),
                    keytype: parts[2].to_string(),
                    keymode: parts[3].to_string(),
                });
            }
        }

        let info = InfoSpace {
            spacename: spacename.to_string(),
            keynames: keys_info,
        };

        Ok(crate::json::output::Output::InfoSpace(info))
    }

    fn show_spaces(&self) -> Result<crate::json::output::Output, Error> {
        let txn = self.env.begin_ro_txn()?; // 読み取り専用トランザクション
        let mut cursor = txn.open_ro_cursor(self.space)?; // space DB のカーソルを開く

        let mut spaces = Vec::new();

        for result in cursor.iter_start() {
            let (key_bytes, _val_bytes) = result;

            // &[u8] -> &str への変換。? で Error に変換可能
            let s: &str = std::str::from_utf8(key_bytes)?;

            // &str -> String
            let string: String = s.to_string();

            spaces.push(string);
        }

        Ok(Output::ShowSpaces(crate::json::output::ShowSpaces {
            spacenames: spaces,
        }))
    }

    fn create_key(
        &self,
        spacename: &str,
        keyname: &str,
        keytype: crate::json::input::KeyType,
        keymode: crate::json::input::KeyMode,
    ) -> Result<crate::json::output::Output, Error> {
        let space_bytes = spacename.as_bytes();
        let mut txn = self.env.begin_rw_txn()?;
        let space_uuid = match txn.get(self.space, &space_bytes) {
            Ok(v) => v,
            Err(LmdbError::NotFound) => {
                return Err(Error::SpaceNotFound {
                    space_name: spacename.to_string(),
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

    fn drop_key(
        &self,
        spacename: &str,
        keyname: &str,
    ) -> Result<crate::json::output::Output, Error> {
        let space_bytes = spacename.as_bytes();
        let mut txn = self.env.begin_rw_txn()?;

        // Space の存在確認
        let space_uuid = match txn.get(self.space, &space_bytes) {
            Ok(v) => v,
            Err(LmdbError::NotFound) => {
                return Err(Error::SpaceNotFound {
                    space_name: spacename.to_string(),
                });
            }
            Err(e) => return Err(Error::from(e)),
        };

        let key_prefix = [space_uuid, keyname.as_bytes()].concat();

        // Cursor を使って削除対象のキーを収集
        let mut keys_to_delete = Vec::new();
        {
            let mut cursor = txn.open_ro_cursor(self.key)?;
            for (k, _) in cursor.iter_start() {
                if k.starts_with(&key_prefix) {
                    keys_to_delete.push(k.to_vec());
                }
            }
        } // <- cursor がここで drop され、txn は再び mutable に

        if keys_to_delete.is_empty() {
            return Err(Error::KeyNotFound {
                space_name: spacename.to_string(),
                key_name: keyname.to_string(),
                location: "drop_key",
            });
        }

        // コピーしたキーを削除
        for k in keys_to_delete {
            txn.del(self.key, &k, None)?;
        }

        txn.commit()?;
        Ok(Output::Success)
    }

    fn show_keys(&self, spacename: &str) -> Result<crate::json::output::Output, Error> {
        let space_bytes = spacename.as_bytes();
        let txn = self.env.begin_ro_txn()?;
        let space_uuid = match txn.get(self.space, &space_bytes) {
            Ok(v) => v,
            Err(LmdbError::NotFound) => {
                return Err(Error::SpaceNotFound {
                    space_name: spacename.to_string(),
                });
            }
            Err(e) => return Err(Error::from(e)),
        };

        let mut cursor = txn.open_ro_cursor(self.key)?;
        let mut keys = Vec::new();
        for (k, _v) in cursor.iter_start() {
            if k.starts_with(space_uuid) {
                let keyname_len = k.len() - space_uuid.len() - 2;
                let keyname_bytes = &k[space_uuid.len()..space_uuid.len() + keyname_len];
                let keyname = std::str::from_utf8(keyname_bytes)?.to_string();
                keys.push(keyname);
            }
        }

        Ok(Output::Showkeys(Showkeys { keynames: keys }))
    }
    fn info_key(
        &self,
        spacename: &str,
        keyname: &str,
    ) -> Result<crate::json::output::Output, Error> {
        let space_bytes = spacename.as_bytes();
        let txn = self.env.begin_ro_txn()?;

        // Space の存在確認
        let space_uuid = match txn.get(self.space, &space_bytes) {
            Ok(v) => v,
            Err(LmdbError::NotFound) => {
                return Err(Error::SpaceNotFound {
                    space_name: spacename.to_string(),
                });
            }
            Err(e) => return Err(Error::from(e)),
        };

        let key_prefix = [space_uuid, keyname.as_bytes()].concat();
        let mut found_key: Option<&[u8]> = None;

        // Cursor のスコープをブロックで限定
        {
            let mut cursor = txn.open_ro_cursor(self.key)?;
            for (k, _v) in cursor.iter_start() {
                if k.starts_with(&key_prefix) {
                    found_key = Some(k);
                    break;
                }
            }
        } // <- cursor がここで drop される

        let k = match found_key {
            Some(k) => k,
            None => {
                return Err(Error::KeyNotFound {
                    space_name: spacename.to_string(),
                    key_name: keyname.to_string(),
                    location: "info_key",
                });
            }
        };

        if k.len() < space_uuid.len() + keyname.len() + 2 {
            return Err(Error::ParseError {
                message: "Invalid key length".to_string(),
                location: "info_key",
            });
        }

        // keytype と keymode を取得
        let keytype = id_keytype(k[k.len() - 2]);
        let keymode = KeyMode::try_from(k[k.len() - 1]).map_err(|_| Error::ParseError {
            message: "Invalid keymode value".to_string(),
            location: "info_key",
        })?;

        Ok(Output::InfoKey(InfoKey {
            keyname: keyname.to_string(),
            keytype: format!("{:?}", keytype),
            keymode: format!("{:?}", keymode),
        }))
    }

    fn insert_value(
        &self,
        spacename: &str,
        keyname: &str,
        ids: Vec<Vec<u8>>,
        value: ValueEntry,
    ) -> Result<Output, Error> {
        let mut txn = self.env.begin_rw_txn()?;

        // 1. Space存在確認
        let space_bytes = spacename.as_bytes();
        let space_uuid_bytes = txn.get(self.space, &space_bytes).map_err(|e| match e {
            LmdbError::NotFound => Error::SpaceNotFound {
                space_name: spacename.to_string(),
            },
            _ => Error::from(e),
        })?;

        // 2. Key存在確認 & key_uuid取得
        let (key_uuid, keytype) = {
            let mut key_cursor = txn.open_ro_cursor(self.key)?;
            let mut found = None;
            for (k, v) in key_cursor.iter_start() {
                if k.starts_with(space_uuid_bytes) && std::str::from_utf8(k)?.contains(keyname) {
                    let keytype_id_byte = k[k.len() - 2];
                    found = Some((v.to_vec(), id_keytype(keytype_id_byte)));
                    break;
                }
            }
            found.ok_or(Error::KeyNotFound {
                space_name: spacename.to_string(),
                key_name: keyname.to_string(),
                location: "insert_value",
            })?
        }; // <- key_cursor drop で txn が再び可変借用可能

        // 3. ValueEntry と Key の型チェック
        let type_matches = match (&keytype, &value) {
            (KeyType::INT, ValueEntry::INT(_)) => true,
            (KeyType::FLOAT, ValueEntry::FLOAT(_)) => true,
            (KeyType::BOOLEAN, ValueEntry::BOOLEAN(_)) => true,
            (KeyType::TEXT, ValueEntry::TEXT(_)) => true,
            _ => false,
        };
        if !type_matches {
            return Err(Error::TypeMismatchFilter {
                expected_type: format!("{:?}", keytype),
                operation: format!("{:?}", value),
                location: "insert_value",
            });
        }

        // 4. すべてのIDを事前チェック（重複が1つでもあればエラー）
        for id in &ids {
            let db_key = [key_uuid.clone(), id.clone()].concat();
            if txn.get(self.value, &db_key).is_ok() {
                return Err(Error::InsertError {
                    space_name: spacename.to_string(),
                    key_name: keyname.to_string(),
                });
            }
        }

        // 5. すべて重複なしならまとめて LMDB に保存
        for id in ids {
            let db_key = [key_uuid.clone(), id].concat();
            txn.put(self.value, &db_key, &value.to_bytes(), WriteFlags::empty())?;
        }

        txn.commit()?;
        Ok(Output::Success)
    }

    fn patch_value(
        &self,
        spacename: &str,
        keyname: &str,
        ids: Vec<Vec<u8>>,
        value: super::ValueEntry,
    ) -> Result<crate::json::output::Output, Error> {
        let mut txn = self.env.begin_rw_txn()?;

        // 1. Space存在確認
        let space_bytes = spacename.as_bytes();
        let space_uuid_bytes = match txn.get(self.space, &space_bytes) {
            Ok(v) => v,
            Err(LmdbError::NotFound) => {
                return Err(Error::SpaceNotFound {
                    space_name: spacename.to_string(),
                });
            }
            Err(e) => return Err(Error::from(e)),
        };

        // 2. Key存在確認（カーソルはスコープで即 drop）
        let (key_bytes, _key_id_bytes) = {
            let mut key_cursor = txn.open_ro_cursor(self.key)?;
            let mut key_found = None;

            for result in key_cursor.iter_start() {
                let (k, v) = result;
                if k.starts_with(space_uuid_bytes) && std::str::from_utf8(k)?.contains(keyname) {
                    key_found = Some((k.to_vec(), v.to_vec()));
                    break;
                }
            }

            key_found.ok_or(Error::KeyNotFound {
                space_name: spacename.to_string(),
                key_name: keyname.to_string(),
                location: "patch_value",
            })?
        };

        // 3. ValueEntry と Key の型チェック
        let keytype_id_byte = key_bytes[key_bytes.len() - 2];
        let keytype = id_keytype(keytype_id_byte);

        let type_matches = match (keytype, &value) {
            (KeyType::INT, ValueEntry::INT(_)) => true,
            (KeyType::FLOAT, ValueEntry::FLOAT(_)) => true,
            (KeyType::BOOLEAN, ValueEntry::BOOLEAN(_)) => true,
            (KeyType::TEXT, ValueEntry::TEXT(_)) => true,
            _ => false,
        };

        if !type_matches {
            return Err(Error::TypeMismatchFilter {
                expected_type: format!("{:?}", keytype),
                operation: format!("{:?}", value),
                location: "patch_value",
            });
        }

        // 4. IDごとに既存値確認 & 新規挿入
        for id in ids {
            let db_key = [key_bytes.clone(), id.clone()].concat();

            // 既に存在する場合はスキップ
            if txn.get(self.value, &db_key).is_ok() {
                continue;
            }

            // 存在しなければ挿入
            txn.put(self.value, &db_key, &value.to_bytes(), WriteFlags::empty())?;
        }

        txn.commit()?;
        Ok(Output::Success)
    }

    // fn update_value(
    //     &self,
    //     spacename: &str,
    //     keyname: &str,
    //     ids: Vec<Vec<u8>>,
    //     value: super::ValueEntry,
    // ) -> Result<crate::json::output::Output, Error> {
    // }
    fn delete_value(
        &self,
        spacename: &str,
        keyname: &str,
        ids: Vec<Vec<u8>>,
    ) -> Result<crate::json::output::Output, Error> {
        let mut txn = self.env.begin_rw_txn()?;

        // 1. Space存在確認
        let space_bytes = spacename.as_bytes();
        let space_uuid_bytes = txn.get(self.space, &space_bytes).map_err(|e| match e {
            LmdbError::NotFound => Error::SpaceNotFound {
                space_name: spacename.to_string(),
            },
            _ => Error::from(e),
        })?;

        // 2. Key存在確認 & key_uuid取得
        let key_uuid = {
            let mut key_cursor = txn.open_ro_cursor(self.key)?;
            let mut found = None;
            for (k, v) in key_cursor.iter_start() {
                if k.starts_with(space_uuid_bytes) && std::str::from_utf8(k)?.contains(keyname) {
                    found = Some(v.to_vec());
                    break;
                }
            }
            found.ok_or(Error::KeyNotFound {
                space_name: spacename.to_string(),
                key_name: keyname.to_string(),
                location: "delete_value",
            })?
        };

        // 3. IDsごとに前方一致で削除
        for id in ids {
            let mut cursor = txn.open_rw_cursor(self.value)?;
            let mut keys_to_delete = Vec::new();

            for (k, _v) in cursor.iter_start() {
                if k.starts_with(&key_uuid) {
                    let id_bytes = &k[key_uuid.len()..]; // key_uuid を除いた部分
                    if id_bytes.starts_with(&id) {
                        keys_to_delete.push(k.to_vec());
                    }
                }
            }
            drop(cursor);

            for k in keys_to_delete {
                txn.del(self.value, &k, None)?;
            }
        }

        txn.commit()?;
        Ok(crate::json::output::Output::Success)
    }

    fn select_value(
        &self,
        spacename: &str,
        keynames: Vec<String>,
        ids: Vec<Vec<u8>>,
    ) -> Result<HashMap<Vec<u8>, Vec<(String, ValueEntry)>>, Error> {
        let txn = self.env.begin_ro_txn()?;

        // 1. Space UUID の取得
        let space_bytes = spacename.as_bytes();
        let space_uuid = txn.get(self.space, &space_bytes).map_err(|e| match e {
            LmdbError::NotFound => Error::SpaceNotFound {
                space_name: spacename.to_string(),
            },
            _ => Error::from(e),
        })?;

        let mut result_map: HashMap<Vec<u8>, Vec<(String, ValueEntry)>> = HashMap::new();

        for keyname in keynames {
            // 2. Key UUID と KeyType の取得
            let (key_uuid, keytype) = {
                let mut key_cursor = txn.open_ro_cursor(self.key)?;
                let mut found = None;
                for (k, v) in key_cursor.iter_start() {
                    if k.starts_with(space_uuid) && std::str::from_utf8(k)?.contains(&keyname) {
                        let keytype_id_byte = k[k.len() - 2];
                        found = Some((v.to_vec(), id_keytype(keytype_id_byte)));
                        break;
                    }
                }
                found.ok_or(Error::KeyNotFound {
                    space_name: spacename.to_string(),
                    key_name: keyname.to_string(),
                    location: "select_value",
                })?
            };

            // 3. value DB を全走査
            let mut cursor = txn.open_ro_cursor(self.value)?;
            for (k, v) in cursor.iter_start() {
                if !k.starts_with(&key_uuid) {
                    continue;
                }

                let id_bytes = k[key_uuid.len()..].to_vec();

                // 入力された ids のいずれかで前方一致するか
                if ids.iter().any(|input_id| id_bytes.starts_with(input_id)) {
                    let value_entry = ValueEntry::from_bytes(keytype, v).ok_or(Error::NnKnown)?;
                    result_map
                        .entry(id_bytes.clone())
                        .or_insert_with(Vec::new)
                        .push((keyname.to_string(), value_entry));
                }
            }
        }

        Ok(result_map)
    }

    fn show_values(
        &self,
        spacename: &str,
        keyname: &str,
    ) -> Result<HashMap<Vec<u8>, Vec<(String, ValueEntry)>>, Error> {
        let txn = self.env.begin_ro_txn()?;

        // 1. SpaceのUUIDを取得
        let space_bytes = spacename.as_bytes();
        let space_uuid = txn.get(self.space, &space_bytes).map_err(|e| match e {
            LmdbError::NotFound => Error::SpaceNotFound {
                space_name: spacename.to_string(),
            },
            _ => Error::from(e),
        })?;

        // 2. KeyのUUIDとKeyTypeを取得
        let (key_uuid, keytype) = {
            let mut key_cursor = txn.open_ro_cursor(self.key)?;
            let mut found = None;
            for (k, v) in key_cursor.iter_start() {
                if k.starts_with(space_uuid) && std::str::from_utf8(k)?.contains(keyname) {
                    let keytype_id_byte = k[k.len() - 2];
                    found = Some((v.to_vec(), id_keytype(keytype_id_byte)));
                    break;
                }
            }
            found.ok_or(Error::KeyNotFound {
                space_name: spacename.to_string(),
                key_name: keyname.to_string(),
                location: "show_values",
            })?
        };

        // 3. value DB から key_uuid で始まる全ての値を取得
        let mut cursor = txn.open_ro_cursor(self.value)?;
        let mut result_map: HashMap<Vec<u8>, Vec<(String, ValueEntry)>> = HashMap::new();

        for result in cursor.iter_start() {
            let (k, v) = result;
            if k.starts_with(&key_uuid) {
                // k の先頭16バイトは key_uuid, 残りが id_bytes
                let id_bytes = k[key_uuid.len()..].to_vec();
                let value_entry = ValueEntry::from_bytes(keytype, v).ok_or(Error::NnKnown)?;

                result_map
                    .entry(id_bytes)
                    .or_insert_with(Vec::new)
                    .push((keyname.to_string(), value_entry));
            }
        }

        Ok(result_map)
    }

    fn create_user(&self, username: &str, password: &str) -> Result<Output, Error> {
        let mut txn = self.env.begin_rw_txn()?;

        // ユーザー名が既に存在するか確認
        if txn.get(self.user, &username.as_bytes()).is_ok() {
            return Err(Error::UserAlreadyExists {
                user_name: username.to_string(),
            });
        }

        // ソルトを生成
        let salt = SaltString::generate(&mut OsRng);

        // Argon2 インスタンスを作成
        let argon2 = Argon2::default();

        // パスワードをハッシュ化
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| Error::LmdbError {
                message: "Password hash failed".to_string(),
                location: "create_user",
            })?
            .to_string();

        // ユーザー名とハッシュを保存
        txn.put(
            self.user,
            &username.as_bytes(),
            &password_hash.as_bytes(),
            lmdb::WriteFlags::empty(),
        )?;
        txn.commit()?;

        Ok(Output::Success)
    }

    fn drop_user(&self, username: &str) -> Result<Output, Error> {
        if username == "admin" {
            return Err(Error::UserNotFound {
                user_name: "admin".to_string(),
            }); // もしくは専用のエラーを作っても良い
        }

        let mut txn = self.env.begin_rw_txn()?;
        match txn.del(self.user, &username.as_bytes(), None) {
            Ok(_) => {
                txn.commit()?;
                Ok(Output::Success)
            }
            Err(LmdbError::NotFound) => Err(Error::UserNotFound {
                user_name: username.to_string(),
            }),
            Err(e) => Err(Error::from(e)),
        }
    }

    fn info_user(&self, username: &str) -> Result<Output, Error> {
        let txn = self.env.begin_ro_txn()?;
        let hash_bytes =
            txn.get(self.user, &username.as_bytes())
                .map_err(|_| Error::UserNotFound {
                    user_name: username.to_string(),
                })?;
        Ok(Output::InfoUser(InfoUser {
            user_name: username.to_string(),
        }))
    }
    fn show_users(&self) -> Result<Output, Error> {
        let txn = self.env.begin_ro_txn()?;
        let mut cursor = txn.open_ro_cursor(self.user)?;

        let mut users = Vec::new();

        for result in cursor.iter_start() {
            let (key_bytes, _value_bytes) = result;
            let username = std::str::from_utf8(key_bytes)?.to_string();
            users.push(username);
        }

        Ok(Output::ShowUsers(ShowUsers { users }))
    }

    fn verify_user(&self, username: &str, password: &str) -> Result<bool, Error> {
        let txn = self.env.begin_ro_txn()?;

        // ユーザー名に対応するハッシュを取得
        let hash_bytes = match txn.get(self.user, &username.as_bytes()) {
            Ok(v) => v,
            Err(_) => return Ok(false), // ユーザーが存在しない場合は false
        };

        // ハッシュ文字列に変換
        let hash_str = std::str::from_utf8(hash_bytes)?;

        // PasswordHash に変換
        let parsed_hash = PasswordHash::new(hash_str).map_err(|_| Error::LmdbError {
            message: "Invalid stored password hash".to_string(),
            location: "verify_user",
        })?;

        // Argon2 インスタンス
        let argon2 = Argon2::default();

        // パスワードを検証
        let valid = argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok();

        Ok(valid)
    }
}
