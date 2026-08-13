use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::keys::{self, LockScope};
use super::kv::{Reader, Readers};
use super::{TikvRead, TikvWrite, kv};
use crate::error::AppError;
use crate::models::database::table::{
    Table, TableConstraints, TableDataType, TableMetadata, UpdateTableConstraints,
};
use crate::models::database::{DatabaseInfoResponse, DatabaseMetadata};
use crate::models::id::{DatabaseId, TableId};
use crate::repositories::ResolvedTable;

/// メタデータ（データベース・テーブル・ユーザー）は JSON で保存する。
///
/// LMDB 側は heed の [`SerdeJson`](heed::types::SerdeJson) が同じ役割を担うので、
/// 対応するのはこの 2 つだけ。何を扱っているかは失敗時の文言のためだけに受け取る。
pub(super) fn encode<T: serde::Serialize>(what: &str, meta: &T) -> Result<Vec<u8>, AppError> {
    serde_json::to_vec(meta)
        .map_err(|e| AppError::InternalError(format!("failed to encode {what} metadata: {e}")))
}

pub(super) fn decode<T: serde::de::DeserializeOwned>(
    what: &str,
    bytes: &[u8],
) -> Result<T, AppError> {
    serde_json::from_slice(bytes)
        .map_err(|e| AppError::InternalError(format!("failed to decode {what} metadata: {e}")))
}

pub(super) async fn database_meta<R: Reader>(
    txn: &Readers<R>,
    name: &str,
) -> Result<Option<DatabaseMetadata>, AppError> {
    if name.is_empty() {
        return Ok(None);
    }
    match kv::get(txn, keys::database(name)).await? {
        Some(bytes) => Ok(Some(decode("database", &bytes)?)),
        None => Ok(None),
    }
}

pub(super) async fn database_id<R: Reader>(
    txn: &Readers<R>,
    name: &str,
) -> Result<Option<DatabaseId>, AppError> {
    Ok(database_meta(txn, name).await?.map(|meta| meta.id))
}

pub(super) async fn table_meta<R: Reader>(
    txn: &Readers<R>,
    db_id: DatabaseId,
    table_name: &str,
) -> Result<Option<TableMetadata>, AppError> {
    if table_name.is_empty() {
        return Ok(None);
    }
    match kv::get(txn, keys::table(db_id, table_name)).await? {
        Some(bytes) => Ok(Some(decode("table", &bytes)?)),
        None => Ok(None),
    }
}

pub(super) async fn table_id<R: Reader>(
    txn: &Readers<R>,
    db_id: DatabaseId,
    table_name: &str,
) -> Result<Option<TableId>, AppError> {
    Ok(table_meta(txn, db_id, table_name).await?.map(|m| m.id))
}

pub(super) async fn database_name<R: Reader>(
    txn: &Readers<R>,
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
    txn: &Readers<R>,
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
    txn: &Readers<R>,
    db_id: DatabaseId,
) -> Result<Vec<String>, AppError> {
    let keys = kv::scan_prefix_keys(txn, &keys::tables_of(db_id)).await?;
    keys.iter()
        .map(|key| keys::table_name_from_key(key).map(str::to_string))
        .collect()
}

pub(super) async fn table_info<R: Reader>(
    txn: &Readers<R>,
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

/// `(データベース名, テーブル名)` の並びをまとめて解決する。
///
/// 往復は**参照数によらず 2 回**。データベース名の解決なしにテーブルキーを組み立てられない
/// ので段は分かれるが、同じ段の中では 1 回の `batch_get` に束ねられる。1 件ずつ
/// `database_meta` → `table_meta` と辿ると、参照数の 2 倍の往復が直列に並ぶ。
pub(super) async fn resolve_tables<R: Reader>(
    txn: &Readers<R>,
    refs: &[(String, String)],
) -> Result<Vec<ResolvedTable>, AppError> {
    if refs.is_empty() {
        return Ok(Vec::new());
    }

    // 1 段目: データベース名 → ID。重複と空名を落としてから 1 回で引く。
    let db_keys: BTreeMap<Vec<u8>, &str> = refs
        .iter()
        .map(|(db_name, _)| db_name.as_str())
        .filter(|name| !name.is_empty())
        .map(|name| (keys::database(name), name))
        .collect();

    let mut db_ids: HashMap<&str, DatabaseId> = HashMap::new();
    for (key, bytes) in kv::batch_get(txn, db_keys.keys().cloned().collect()).await? {
        if let Some(name) = db_keys.get(&key) {
            db_ids.insert(name, decode::<DatabaseMetadata>("database", &bytes)?.id);
        }
    }

    // 2 段目: 解決できたデータベース配下のテーブルを 1 回で引く。
    let mut table_keys: BTreeSet<Vec<u8>> = BTreeSet::new();
    for (db_name, table_name) in refs {
        if table_name.is_empty() {
            continue;
        }
        if let Some(db_id) = db_ids.get(db_name.as_str()) {
            table_keys.insert(keys::table(*db_id, table_name));
        }
    }

    let mut metas: HashMap<Vec<u8>, TableMetadata> = HashMap::new();
    for (key, bytes) in kv::batch_get(txn, table_keys.into_iter().collect()).await? {
        metas.insert(key, decode("table", &bytes)?);
    }

    // 入力と同じ並びで組み立てる。同じ参照が複数回現れても、引いたのは 1 度きり。
    Ok(refs
        .iter()
        .map(|(db_name, table_name)| {
            let Some(&db_id) = db_ids.get(db_name.as_str()) else {
                return ResolvedTable {
                    db_id: None,
                    table: None,
                };
            };
            let table = (!table_name.is_empty())
                .then(|| metas.get(&keys::table(db_id, table_name)))
                .flatten()
                .map(|meta| Table::from_meta(table_name, meta.clone()));
            ResolvedTable {
                db_id: Some(db_id),
                table,
            }
        })
        .collect())
}

pub(super) async fn database_info<R: Reader>(
    txn: &Readers<R>,
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
    txn: &Readers<R>,
    db_id: DatabaseId,
) -> Result<Vec<Table>, AppError> {
    let entries = kv::scan_prefix(txn, &keys::tables_of(db_id)).await?;
    entries
        .iter()
        .map(|(key, value)| {
            let name = keys::table_name_from_key(key)?;
            Ok(Table::from_meta(name, decode("table", value)?))
        })
        .collect()
}

impl<R: Reader> TikvRead<'_, R> {
    #[tracing::instrument(skip_all)]
    pub(super) async fn database_list_impl(
        &self,
    ) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        let entries = kv::scan_prefix(&self.txn, &keys::Ns::Databases.prefix()).await?;
        entries
            .iter()
            .map(|(key, value)| {
                let name = keys::database_name_from_key(key)?;
                let meta: DatabaseMetadata = decode("database", value)?;
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

    #[tracing::instrument(skip_all, fields(db_name = %db_name))]
    pub(super) async fn table_list_impl(&self, db_name: &str) -> Result<Vec<Table>, AppError> {
        let db_id =
            database_id(&self.txn, db_name)
                .await?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                })?;
        table_list_by_id(&self.txn, db_id).await
    }

    #[tracing::instrument(skip_all)]
    pub(super) async fn table_count_impl(&self, table_id: TableId) -> Result<u64, AppError> {
        kv::table_flex_id_count(&self.txn, table_id).await
    }
}

impl TikvWrite<'_> {
    #[tracing::instrument(skip_all, fields(db_name = %name))]
    pub(super) async fn database_create_impl(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseInfoResponse, AppError> {
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
        kv::put(&self.txn, keys::database(name), encode("database", &meta)?).await;
        kv::put(
            &self.txn,
            keys::database_id_index(id),
            name.as_bytes().to_vec(),
        )
        .await;

        Ok(DatabaseInfoResponse {
            name: name.to_string(),
            description,
        })
    }

    /// データベースを配下のテーブルごと削除する。
    #[tracing::instrument(skip_all, fields(db_name = %name))]
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

        // 配下テーブルを列挙する。データベーススコープを保持しているので、
        // この一覧が途中で増えることはない（`table_create` も同じロックを取る）。
        let tables = table_list_by_id(&self.txn, meta.id).await?;
        for table in tables {
            self.retire_table(meta.id, &table.name, table.id).await;
        }

        kv::delete_many(
            &self.txn,
            [keys::database(name), keys::database_id_index(meta.id)],
        )
        .await;
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(db_name = %name))]
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
            kv::delete(&self.txn, keys::database(name)).await;
            kv::put(
                &self.txn,
                keys::database_id_index(meta.id),
                final_new_name.as_bytes().to_vec(),
            )
            .await;
        }
        kv::put(
            &self.txn,
            keys::database(final_new_name),
            encode("database", &meta)?,
        )
        .await;
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(src_db_name = %src_db_name, copy_name = %copy_name))]
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

        // コピー元テーブルのシャードはスナップショットから読む（`copy_table_into`）。
        let src_tables = table_list_by_id(&self.txn, src_meta.id).await?;

        let copy_db_id = DatabaseId(uuid::Uuid::now_v7());
        let copy_meta = DatabaseMetadata {
            id: copy_db_id,
            description: src_meta.description.clone(),
        };
        kv::put(
            &self.txn,
            keys::database(copy_name),
            encode("database", &copy_meta)?,
        )
        .await;
        kv::put(
            &self.txn,
            keys::database_id_index(copy_db_id),
            copy_name.as_bytes().to_vec(),
        )
        .await;

        for table in src_tables {
            self.copy_table_into(src_meta.id, &table.name, copy_db_id, &table.name)
                .await?;
        }

        Ok(DatabaseInfoResponse {
            name: copy_name.to_string(),
            description: src_meta.description,
        })
    }

    #[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn table_create_impl(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
        description: Option<String>,
        value_index: bool,
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
            value_index,
        };
        kv::put(
            &self.txn,
            keys::table(db_meta.id, table_name),
            encode("table", &meta)?,
        )
        .await;
        kv::put(
            &self.txn,
            keys::table_id_index(id),
            table_name.as_bytes().to_vec(),
        )
        .await;

        Ok(Table {
            id,
            name: table_name.to_string(),
            data_type,
            max_zoom_level,
            constraints: actual_constraints,
            description,
            value_index,
        })
    }

    #[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
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
        // 改名はデータベースのテーブル集合の変更なので、その名前を排他する。
        // 既存データ検証はスナップショット走査で、テーブル単位の排他は取らない
        // （`validate_existing_data` の注記を参照）。
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
            value_index: table.value_index,
        };

        if changed_name {
            kv::delete(&self.txn, keys::table(db_meta.id, table_name)).await;
            kv::put(
                &self.txn,
                keys::table_id_index(table.id),
                table.name.as_bytes().to_vec(),
            )
            .await;
        }
        kv::put(
            &self.txn,
            keys::table(db_meta.id, &table.name),
            encode("table", &meta)?,
        )
        .await;

        Ok(table)
    }

    #[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
    pub(super) async fn table_remove_impl(
        &mut self,
        db_name: &str,
        table_name: &str,
    ) -> Result<(), AppError> {
        // 変えるのはデータベースのテーブル集合だけ。シャード実体は論理削除して
        // 回収に回すので、テーブル全体を排他する必要はない（`retire_table` を参照）。
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

        self.retire_table(db_meta.id, table_name, table.id).await;
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(src_db_name = %src_db_name, src_table_name = %src_table_name, copy_db_name = %copy_db_name, copy_table_name = %copy_table_name))]
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
        // 存在確認だけ。実体の読み出しは `copy_table_into` がスナップショットから行う。
        table_meta(&self.txn, src_db.id, src_table_name)
            .await?
            .ok_or_else(|| AppError::TableNotFound {
                name: src_table_name.to_string(),
            })?;

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

    /// テーブル 1 つを複製する。
    ///
    /// コピー元のシャードは**スナップショットから読む**（ロックを取らない）。
    /// したがって得られるのは「このトランザクションの開始時点における一貫したコピー」で、
    /// コピー中に走った書き込みは含まれない。テーブル全体を排他して「コピー中は
    /// 書き込みを止める」意味論も考えられるが、複数インスタンスが同じクラスタへ
    /// 書き込む前提ではそちらの代償が大きすぎる。
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
            value_index: src_meta.value_index,
        };

        kv::put(
            &self.txn,
            keys::table(dst_db_id, dst_table_name),
            encode("table", &copy_meta)?,
        )
        .await;
        kv::put(
            &self.txn,
            keys::table_id_index(copy_id),
            dst_table_name.as_bytes().to_vec(),
        )
        .await;

        // テーブルのデータ実体を、キー先頭の table_id だけ差し替えて複製する。
        // 対象は `keys::table_data_prefixes` が一括で決める。ここで自前に並べると、
        // 名前空間が増えたときに回収側だけが追随して複製が取りこぼす。
        // 値はそのまま持ち回る（シャード値のフレームはキーに依存しないので、
        // 検証済みのバイト列を再フレームせずそのまま置ける）。
        for prefix in keys::table_data_prefixes(src_meta.id) {
            let entries = kv::scan_prefix(&self.txn, &prefix).await?;
            let mut copied = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                let mut dst = key;
                keys::replace_leading_id(&mut dst, copy_id)?;
                copied.push((dst, value));
            }
            kv::put_many(&self.txn, copied).await;
        }

        Ok(Table {
            id: copy_id,
            name: dst_table_name.to_string(),
            data_type: src_meta.data_type,
            max_zoom_level: src_meta.max_zoom_level,
            constraints: src_meta.constraints,
            description: src_meta.description,
            value_index: src_meta.value_index,
        })
    }

    /// テーブルを**論理削除**する。
    ///
    /// カタログ項目を消して回収待ち行列へ積むだけで、シャード実体には触れない。
    /// 実体の削除は [`gc`](super::gc) が後から分割して行う。こうする理由は 2 つある。
    ///
    /// - **テーブル全体の排他が要らなくなる。** データ書き込みはリーフ単位でしか
    ///   ロックを取らないので（`tree/write.rs`）、実体をここで消そうとすると並行中の
    ///   書き込みと衝突しないことを保証できない。カタログから辿れなくしてしまえば、
    ///   取り残されたキーは誰にも見えず、回収するだけでよい。
    /// - **1 トランザクションのサイズ上限に縛られない。** 大きなテーブルの全キーを
    ///   1 つのトランザクションへ詰めると TiKV の上限に当たる。回収は冪等なので
    ///   分割コミットしてよく、原子性を失わない。
    ///
    /// `TableId` は UUIDv7 なので再利用されない。回収前に飛び込んだ書き込みが
    /// 残骸を作っても、次の掃除で回収される。
    async fn retire_table(&mut self, db_id: DatabaseId, table_name: &str, table_id: TableId) {
        kv::delete_many(
            &self.txn,
            [
                keys::table(db_id, table_name),
                keys::table_id_index(table_id),
            ],
        )
        .await;
        kv::put_many(&self.txn, [super::gc::retire(table_id)]).await;
    }
}
