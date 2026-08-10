//! データベースとテーブルのカタログ操作。
//!
//! ロックの粒度については [`LockScope`] とモジュール冒頭のコメントを参照。
//! テーブル集合を変更・走査する操作はデータベース単位のロックを、
//! シャード集合を触る操作はテーブル単位のロックを宣言する。
//!
//! ロックを**複数宣言する順序は問わない**。取得順は保持集合（`BTreeSet`）が
//! バイト昇順で決めるので、宣言側で並べ替える必要はない（`super::mod` を参照）。

use tokio::sync::Mutex;

use super::keys::{self, LockScope};
use crate::error::AppError;
use crate::models::database::table::{
    Table, TableConstraints, TableDataType, TableMetadata, UpdateTableConstraints,
};
use crate::models::database::{DatabaseInfoResponse, DatabaseMetadata};
use crate::models::id::{DatabaseId, TableId};
use crate::repositories::encoding::shard_entry::ShardEntry;

use super::kv::Reader;
use super::{TikvRead, TikvWrite, kv};

// --- 直列化 ---

fn encode_database(meta: &DatabaseMetadata) -> Result<Vec<u8>, AppError> {
    serde_json::to_vec(meta)
        .map_err(|e| AppError::InternalError(format!("failed to encode database metadata: {e}")))
}

fn decode_database(bytes: &[u8]) -> Result<DatabaseMetadata, AppError> {
    serde_json::from_slice(bytes)
        .map_err(|e| AppError::InternalError(format!("failed to decode database metadata: {e}")))
}

fn encode_table(meta: &TableMetadata) -> Result<Vec<u8>, AppError> {
    serde_json::to_vec(meta)
        .map_err(|e| AppError::InternalError(format!("failed to encode table metadata: {e}")))
}

fn decode_table(bytes: &[u8]) -> Result<TableMetadata, AppError> {
    serde_json::from_slice(bytes)
        .map_err(|e| AppError::InternalError(format!("failed to decode table metadata: {e}")))
}

// --- 読み書き共通の参照系（トランザクションだけを受け取る） ---

pub(super) async fn database_meta<R: Reader>(
    txn: &Mutex<R>,
    name: &str,
) -> Result<Option<DatabaseMetadata>, AppError> {
    if name.is_empty() {
        return Ok(None);
    }
    match kv::get(txn, keys::database(name)).await? {
        Some(bytes) => Ok(Some(decode_database(&bytes)?)),
        None => Ok(None),
    }
}

pub(super) async fn database_id<R: Reader>(
    txn: &Mutex<R>,
    name: &str,
) -> Result<Option<DatabaseId>, AppError> {
    Ok(database_meta(txn, name).await?.map(|meta| meta.id))
}

pub(super) async fn table_meta<R: Reader>(
    txn: &Mutex<R>,
    db_id: DatabaseId,
    table_name: &str,
) -> Result<Option<TableMetadata>, AppError> {
    if table_name.is_empty() {
        return Ok(None);
    }
    match kv::get(txn, keys::table(db_id, table_name)).await? {
        Some(bytes) => Ok(Some(decode_table(&bytes)?)),
        None => Ok(None),
    }
}

pub(super) async fn table_id<R: Reader>(
    txn: &Mutex<R>,
    db_id: DatabaseId,
    table_name: &str,
) -> Result<Option<TableId>, AppError> {
    Ok(table_meta(txn, db_id, table_name).await?.map(|m| m.id))
}

pub(super) async fn database_name<R: Reader>(
    txn: &Mutex<R>,
    db_id: DatabaseId,
) -> Result<Option<String>, AppError> {
    match kv::get(txn, keys::database_id_index(db_id)).await? {
        Some(bytes) => Ok(Some(String::from_utf8(bytes).map_err(|e| {
            AppError::InternalError(format!("database name is not valid utf-8: {e}"))
        })?)),
        None => Ok(None),
    }
}

pub(super) async fn table_name<R: Reader>(
    txn: &Mutex<R>,
    table_id: TableId,
) -> Result<Option<String>, AppError> {
    match kv::get(txn, keys::table_id_index(table_id)).await? {
        Some(bytes) => Ok(Some(String::from_utf8(bytes).map_err(|e| {
            AppError::InternalError(format!("table name is not valid utf-8: {e}"))
        })?)),
        None => Ok(None),
    }
}

pub(super) async fn table_names<R: Reader>(
    txn: &Mutex<R>,
    db_id: DatabaseId,
) -> Result<Vec<String>, AppError> {
    let keys = kv::scan_prefix_keys(txn, &keys::tables_of(db_id)).await?;
    keys.iter()
        .map(|key| keys::table_name_from_key(key).map(str::to_string))
        .collect()
}

pub(super) async fn table_info<R: Reader>(
    txn: &Mutex<R>,
    db_name: &str,
    table_name: &str,
) -> Result<Option<Table>, AppError> {
    if db_name.is_empty() {
        return Ok(None);
    }
    let db_meta = database_meta(txn, db_name)
        .await?
        .ok_or_else(|| AppError::DatabaseNotFound {
            name: db_name.to_string(),
        })?;
    Ok(table_meta(txn, db_meta.id, table_name)
        .await?
        .map(|meta| Table::from_meta(table_name, meta)))
}

pub(super) async fn database_info<R: Reader>(
    txn: &Mutex<R>,
    name: &str,
) -> Result<Option<DatabaseInfoResponse>, AppError> {
    Ok(database_meta(txn, name)
        .await?
        .map(|meta| DatabaseInfoResponse {
            name: name.to_string(),
            description: meta.description,
        }))
}

pub(super) async fn table_list_by_id<R: Reader>(
    txn: &Mutex<R>,
    db_id: DatabaseId,
) -> Result<Vec<Table>, AppError> {
    let entries = kv::scan_prefix(txn, &keys::tables_of(db_id)).await?;
    entries
        .iter()
        .map(|(key, value)| {
            let name = keys::table_name_from_key(key)?;
            Ok(Table::from_meta(name, decode_table(value)?))
        })
        .collect()
}

// --- 読み取り側 ---

impl<R: Reader> TikvRead<'_, R> {
    pub(super) async fn database_list_impl(
        &self,
    ) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        let entries = kv::scan_prefix(&self.txn, &keys::Ns::Databases.prefix()).await?;
        entries
            .iter()
            .map(|(key, value)| {
                let name = keys::database_name_from_key(key)?;
                let meta = decode_database(value)?;
                Ok((
                    meta.id,
                    DatabaseInfoResponse {
                        name: name.to_string(),
                        description: meta.description,
                    },
                ))
            })
            .collect()
    }

    pub(super) async fn table_list_impl(&self, db_name: &str) -> Result<Vec<Table>, AppError> {
        let db_id =
            database_id(&self.txn, db_name)
                .await?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                })?;
        table_list_by_id(&self.txn, db_id).await
    }

    pub(super) async fn table_count_impl(&self, table_id: TableId) -> Result<u64, AppError> {
        let entries = kv::scan_shard_prefix(&self.txn, &keys::shards_of(table_id)).await?;
        let mut total = 0u64;
        for (_, value) in entries {
            if let Some(count) = ShardEntry::leaf_count(value.entry())? {
                total += count as u64;
            }
        }
        Ok(total)
    }
}

// --- 書き込み側 ---

impl TikvWrite<'_> {
    /// データベースを作成する。名前そのものが排他の単位になる。
    pub(super) async fn database_create_impl(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseInfoResponse, AppError> {
        // 「存在しないことを確認してから作る」ので、その名前を指すデータベーススコープを取る。
        let id = DatabaseId(uuid::Uuid::now_v7());
        self.require_lock(LockScope::Database, name.as_bytes())?;

        if database_meta(&self.txn, name).await?.is_some() {
            return Err(AppError::DatabaseAlreadyExists {
                name: name.to_string(),
            });
        }

        let meta = DatabaseMetadata {
            id,
            description: description.clone(),
        };
        kv::put(&self.txn, keys::database(name), encode_database(&meta)?).await?;
        kv::put(
            &self.txn,
            keys::database_id_index(id),
            name.as_bytes().to_vec(),
        )
        .await?;

        Ok(DatabaseInfoResponse {
            name: name.to_string(),
            description,
        })
    }

    /// データベースを配下のテーブルごと削除する。
    ///
    /// テーブル集合の走査と削除を安全に行うため、まずデータベーススコープを取り、
    /// 判明した各テーブルのスコープも宣言する（足りなければフレームワークがやり直す）。
    pub(super) async fn database_remove_impl(&mut self, name: &str) -> Result<(), AppError> {
        if name.is_empty() {
            return Err(AppError::DatabaseNotFound {
                name: name.to_string(),
            });
        }

        self.require_lock(LockScope::Database, name.as_bytes())?;

        let Some(meta) = database_meta(&self.txn, name).await? else {
            return Err(AppError::DatabaseNotFound {
                name: name.to_string(),
            });
        };

        // 配下テーブルを列挙し、それぞれのテーブルスコープを宣言する。
        // データベーススコープを保持しているのでこの一覧は変化しない。
        let tables = table_list_by_id(&self.txn, meta.id).await?;
        let table_ids: Vec<[u8; 16]> = tables.iter().map(|t| t.id.into_bytes()).collect();
        self.require_locks(table_ids.iter().map(|id| (LockScope::Table, id.as_slice())))?;

        for table in tables {
            self.remove_table_contents(meta.id, &table.name, table.id)
                .await?;
        }

        kv::delete(&self.txn, keys::database(name)).await?;
        kv::delete(&self.txn, keys::database_id_index(meta.id)).await?;
        Ok(())
    }

    pub(super) async fn database_update_impl(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<(), AppError> {
        let final_new_name = new_name.as_deref().unwrap_or(name);
        if name != final_new_name {
            crate::services::helpers::name_valid::name_valid(final_new_name)?;
        }

        // 旧名と新名の両方を排他する（片方だけでは、逆向きの同時改名で両者が
        // 相手の名前へ書き込みうる）。取得順は保持集合が決めるので、ここでは並べない。
        self.require_locks([
            (LockScope::Database, name.as_bytes()),
            (LockScope::Database, final_new_name.as_bytes()),
        ])?;

        let Some(mut meta) = database_meta(&self.txn, name).await? else {
            return Err(AppError::DatabaseNotFound {
                name: name.to_string(),
            });
        };

        if name != final_new_name && database_meta(&self.txn, final_new_name).await?.is_some() {
            return Err(AppError::DatabaseAlreadyExists {
                name: final_new_name.to_string(),
            });
        }

        if let Some(desc) = description {
            meta.description = desc;
        }

        if name != final_new_name {
            kv::delete(&self.txn, keys::database(name)).await?;
            kv::put(
                &self.txn,
                keys::database_id_index(meta.id),
                final_new_name.as_bytes().to_vec(),
            )
            .await?;
        }
        kv::put(
            &self.txn,
            keys::database(final_new_name),
            encode_database(&meta)?,
        )
        .await?;
        Ok(())
    }

    pub(super) async fn database_copy_impl(
        &mut self,
        src_db_name: &str,
        copy_name: &str,
    ) -> Result<DatabaseInfoResponse, AppError> {
        crate::services::helpers::name_valid::name_valid(copy_name)?;

        self.require_locks([
            (LockScope::Database, src_db_name.as_bytes()),
            (LockScope::Database, copy_name.as_bytes()),
        ])?;

        let src_meta = database_meta(&self.txn, src_db_name)
            .await?
            .ok_or_else(|| AppError::DatabaseNotFound {
                name: src_db_name.to_string(),
            })?;

        if database_meta(&self.txn, copy_name).await?.is_some() {
            return Err(AppError::DatabaseAlreadyExists {
                name: copy_name.to_string(),
            });
        }

        // コピー元テーブルのシャードを読むため、各テーブルのスコープも宣言する。
        let src_tables = table_list_by_id(&self.txn, src_meta.id).await?;
        let src_table_ids: Vec<[u8; 16]> = src_tables.iter().map(|t| t.id.into_bytes()).collect();
        self.require_locks(
            src_table_ids
                .iter()
                .map(|id| (LockScope::Table, id.as_slice())),
        )?;

        let copy_db_id = DatabaseId(uuid::Uuid::now_v7());
        let copy_meta = DatabaseMetadata {
            id: copy_db_id,
            description: src_meta.description.clone(),
        };
        kv::put(
            &self.txn,
            keys::database(copy_name),
            encode_database(&copy_meta)?,
        )
        .await?;
        kv::put(
            &self.txn,
            keys::database_id_index(copy_db_id),
            copy_name.as_bytes().to_vec(),
        )
        .await?;

        for table in src_tables {
            self.copy_table_into(src_meta.id, &table.name, copy_db_id, &table.name)
                .await?;
        }

        Ok(DatabaseInfoResponse {
            name: copy_name.to_string(),
            description: src_meta.description,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn table_create_impl(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
        description: Option<String>,
    ) -> Result<Table, AppError> {
        if db_name.is_empty() {
            return Err(AppError::DatabaseNotFound {
                name: db_name.to_string(),
            });
        }
        if table_name.is_empty() {
            return Err(AppError::InternalError(
                "Table name cannot be empty".to_string(),
            ));
        }

        // データベースのテーブル集合を変更するのでデータベーススコープを取る。
        self.require_lock(LockScope::Database, db_name.as_bytes())?;

        let db_meta =
            database_meta(&self.txn, db_name)
                .await?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                })?;

        if table_meta(&self.txn, db_meta.id, table_name)
            .await?
            .is_some()
        {
            return Err(AppError::TableAlreadyExists {
                name: table_name.to_string(),
            });
        }

        let id = TableId(uuid::Uuid::now_v7());
        let actual_constraints = TableConstraints::with_enum_ids(data_type, constraints)
            .map_err(|reason| AppError::ConstraintViolation { reason })?;
        if let Some(c) = &actual_constraints
            && let Err(msg) = c.validate()
        {
            return Err(AppError::ConstraintViolation { reason: msg });
        }

        let meta = TableMetadata {
            id,
            data_type,
            max_zoom_level,
            constraints: actual_constraints.clone(),
            description: description.clone(),
        };
        kv::put(
            &self.txn,
            keys::table(db_meta.id, table_name),
            encode_table(&meta)?,
        )
        .await?;
        kv::put(
            &self.txn,
            keys::table_id_index(id),
            table_name.as_bytes().to_vec(),
        )
        .await?;

        Ok(Table {
            id,
            name: table_name.to_string(),
            data_type,
            max_zoom_level,
            constraints: actual_constraints,
            description,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn table_update_impl(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<UpdateTableConstraints>>,
        description: Option<Option<String>>,
        validate_existing_data: bool,
    ) -> Result<Table, AppError> {
        // 改名はテーブル集合の変更、既存データ検証はシャード集合の走査なので両方を取る。
        self.require_lock(LockScope::Database, db_name.as_bytes())?;

        let db_meta =
            database_meta(&self.txn, db_name)
                .await?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                })?;
        let mut table = table_meta(&self.txn, db_meta.id, table_name)
            .await?
            .map(|meta| Table::from_meta(table_name, meta))
            .ok_or_else(|| AppError::TableNotFound {
                name: table_name.to_string(),
            })?;

        if validate_existing_data {
            self.require_lock(LockScope::Table, &table.id.into_bytes())?;
        }

        let changed_name = match new_name {
            Some(nn) if nn != table_name => {
                if table_meta(&self.txn, db_meta.id, nn).await?.is_some() {
                    return Err(AppError::TableAlreadyExists {
                        name: nn.to_string(),
                    });
                }
                table.name = nn.to_string();
                true
            }
            _ => false,
        };

        if let Some(nc_opt) = new_constraints {
            let new_c = match nc_opt {
                None => None,
                Some(nc) => {
                    TableConstraints::merged_with(table.data_type, table.constraints.as_ref(), nc)
                        .map_err(|reason| AppError::ConstraintViolation { reason })?
                }
            };
            if let Some(c) = &new_c
                && let Err(msg) = c.validate()
            {
                return Err(AppError::ConstraintViolation { reason: msg });
            }
            table.constraints = new_c;

            if validate_existing_data {
                self.validate_existing_data(table.id, table.data_type, table.constraints.as_ref())
                    .await?;
            }
        }

        if let Some(desc) = description {
            table.description = desc;
        }

        let meta = TableMetadata {
            id: table.id,
            data_type: table.data_type,
            max_zoom_level: table.max_zoom_level,
            constraints: table.constraints.clone(),
            description: table.description.clone(),
        };

        if changed_name {
            kv::delete(&self.txn, keys::table(db_meta.id, table_name)).await?;
            kv::put(
                &self.txn,
                keys::table_id_index(table.id),
                table.name.as_bytes().to_vec(),
            )
            .await?;
        }
        kv::put(
            &self.txn,
            keys::table(db_meta.id, &table.name),
            encode_table(&meta)?,
        )
        .await?;

        Ok(table)
    }

    pub(super) async fn table_remove_impl(
        &mut self,
        db_name: &str,
        table_name: &str,
    ) -> Result<(), AppError> {
        // テーブル集合からの除去（DB スコープ）とシャード集合の削除（テーブルスコープ）の両方。
        self.require_lock(LockScope::Database, db_name.as_bytes())?;

        let db_meta =
            database_meta(&self.txn, db_name)
                .await?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                })?;
        let table = table_meta(&self.txn, db_meta.id, table_name)
            .await?
            .ok_or_else(|| AppError::TableNotFound {
                name: table_name.to_string(),
            })?;

        self.require_lock(LockScope::Table, &table.id.into_bytes())?;

        self.remove_table_contents(db_meta.id, table_name, table.id)
            .await
    }

    pub(super) async fn table_copy_impl(
        &mut self,
        src_db_name: &str,
        src_table_name: &str,
        copy_db_name: &str,
        copy_table_name: &str,
    ) -> Result<Table, AppError> {
        crate::services::helpers::name_valid::name_valid(copy_table_name)?;

        self.require_locks([
            (LockScope::Database, src_db_name.as_bytes()),
            (LockScope::Database, copy_db_name.as_bytes()),
        ])?;

        let src_db = database_meta(&self.txn, src_db_name)
            .await?
            .ok_or_else(|| AppError::DatabaseNotFound {
                name: src_db_name.to_string(),
            })?;
        let src_table = table_meta(&self.txn, src_db.id, src_table_name)
            .await?
            .ok_or_else(|| AppError::TableNotFound {
                name: src_table_name.to_string(),
            })?;

        // コピー元のシャードを一貫して読むため、その集合も排他する。
        self.require_lock(LockScope::Table, &src_table.id.into_bytes())?;

        let copy_db = database_meta(&self.txn, copy_db_name)
            .await?
            .ok_or_else(|| AppError::DatabaseNotFound {
                name: copy_db_name.to_string(),
            })?;

        if table_meta(&self.txn, copy_db.id, copy_table_name)
            .await?
            .is_some()
        {
            return Err(AppError::TableAlreadyExists {
                name: copy_table_name.to_string(),
            });
        }

        self.copy_table_into(src_db.id, src_table_name, copy_db.id, copy_table_name)
            .await
    }

    /// テーブル 1 つを複製する。呼び出し側が必要なロックを取得済みであること。
    async fn copy_table_into(
        &mut self,
        src_db_id: DatabaseId,
        src_table_name: &str,
        dst_db_id: DatabaseId,
        dst_table_name: &str,
    ) -> Result<Table, AppError> {
        let src_meta = table_meta(&self.txn, src_db_id, src_table_name)
            .await?
            .ok_or_else(|| AppError::TableNotFound {
                name: src_table_name.to_string(),
            })?;

        let copy_id = TableId(uuid::Uuid::now_v7());
        let copy_meta = TableMetadata {
            id: copy_id,
            data_type: src_meta.data_type,
            max_zoom_level: src_meta.max_zoom_level,
            constraints: src_meta.constraints.clone(),
            description: src_meta.description.clone(),
        };

        kv::put(
            &self.txn,
            keys::table(dst_db_id, dst_table_name),
            encode_table(&copy_meta)?,
        )
        .await?;
        kv::put(
            &self.txn,
            keys::table_id_index(copy_id),
            dst_table_name.as_bytes().to_vec(),
        )
        .await?;

        // シャードと値インデックスを、キー先頭の table_id だけ差し替えて複製する。
        // 値はそのまま持ち回る（シャード値のフレームはキーに依存しないので、
        // 検証済みのバイト列を再フレームせずそのまま置ける）。
        let shards = kv::scan_prefix(&self.txn, &keys::shards_of(src_meta.id)).await?;
        for (key, value) in shards {
            let mut dst = key;
            keys::replace_leading_id(&mut dst, copy_id)?;
            kv::put(&self.txn, dst, value).await?;
        }

        let index_keys =
            kv::scan_prefix_keys(&self.txn, &keys::value_index_of(src_meta.id)).await?;
        for key in index_keys {
            let mut dst = key;
            keys::replace_leading_id(&mut dst, copy_id)?;
            kv::put(&self.txn, dst, Vec::new()).await?;
        }

        Ok(Table {
            id: copy_id,
            name: dst_table_name.to_string(),
            data_type: src_meta.data_type,
            max_zoom_level: src_meta.max_zoom_level,
            constraints: src_meta.constraints,
            description: src_meta.description,
        })
    }

    /// テーブルのシャード・値インデックス・カタログ項目をまとめて削除する。
    /// 呼び出し側が該当スコープのロックを取得済みであること。
    async fn remove_table_contents(
        &mut self,
        db_id: DatabaseId,
        table_name: &str,
        table_id: TableId,
    ) -> Result<(), AppError> {
        for key in kv::scan_prefix_keys(&self.txn, &keys::shards_of(table_id)).await? {
            kv::delete(&self.txn, key).await?;
        }
        for key in kv::scan_prefix_keys(&self.txn, &keys::value_index_of(table_id)).await? {
            kv::delete(&self.txn, key).await?;
        }
        kv::delete(&self.txn, keys::table(db_id, table_name)).await?;
        kv::delete(&self.txn, keys::table_id_index(table_id)).await?;
        Ok(())
    }
}
