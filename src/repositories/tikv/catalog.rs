//! データベースとテーブルのカタログ操作。

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::keys::{self, LockScope};
use super::kv::{Reader, Readers};
use super::{TikvRead, TikvWrite, kv};
use crate::error::{AppError, Resource, Stored};
use crate::models::database::table::{
    Table, TableConstraints, TableDataType, TableMetadata, UpdateTableConstraints,
};
use crate::models::database::{DatabaseInfoResponse, DatabaseMetadata};
use crate::models::id::{DatabaseId, TableId};
use crate::repositories::ResolvedTable;
use crate::repositories::encoding::OwnedName;

/// `stored` は失敗をどの語彙で報告するかだけに使う（LMDB 側と同じ語彙）。
pub(super) fn encode<T: serde::Serialize>(stored: Stored, meta: &T) -> Result<Vec<u8>, AppError> {
    serde_json::to_vec(meta).map_err(|e| AppError::corrupt(stored, e))
}

pub(super) fn decode<T: serde::de::DeserializeOwned>(
    stored: Stored,
    bytes: &[u8],
) -> Result<T, AppError> {
    serde_json::from_slice(bytes).map_err(|e| AppError::corrupt(stored, e))
}

pub(super) async fn database_meta<R: Reader>(
    txn: &Readers<R>,
    name: &str,
) -> Result<Option<DatabaseMetadata>, AppError> {
    if name.is_empty() {
        return Ok(None);
    }
    match kv::get(txn, keys::database(name)).await? {
        Some(bytes) => Ok(Some(decode(Stored::DatabaseEntry, &bytes)?)),
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
        Some(bytes) => Ok(Some(decode(Stored::TableEntry, &bytes)?)),
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

/// **1 回の `batch_get` で引く。** 権限の描画は保持数ぶん名前を要求するので、
/// 1 件ずつ往復すると権限を多く持つ利用者ほど一覧が重くなる。
pub(super) async fn database_names<R: Reader>(
    txn: &Readers<R>,
    ids: &[DatabaseId],
) -> Result<HashMap<DatabaseId, String>, AppError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let by_key: BTreeMap<Vec<u8>, DatabaseId> = ids
        .iter()
        .map(|&db_id| (keys::database_id_index(db_id), db_id))
        .collect();

    let mut out = HashMap::with_capacity(ids.len());
    for (key, bytes) in kv::batch_get(txn, by_key.keys().cloned().collect()).await? {
        if let Some(&db_id) = by_key.get(&key) {
            out.insert(
                db_id,
                String::from_utf8(bytes)
                    .map_err(|e| AppError::corrupt(Stored::DatabaseEntry, e))?,
            );
        }
    }
    Ok(out)
}

pub(super) async fn table_entries<R: Reader>(
    txn: &Readers<R>,
    ids: &[TableId],
) -> Result<HashMap<TableId, (DatabaseId, String)>, AppError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let by_key: BTreeMap<Vec<u8>, TableId> = ids
        .iter()
        .map(|&table_id| (keys::table_id_index(table_id), table_id))
        .collect();

    let mut out = HashMap::with_capacity(ids.len());
    for (key, bytes) in kv::batch_get(txn, by_key.keys().cloned().collect()).await? {
        if let Some(&table_id) = by_key.get(&key) {
            out.insert(
                table_id,
                OwnedName::try_from(bytes.as_slice())?.into_parts(),
            );
        }
    }
    Ok(out)
}

/// ACL 側から辿った ID でデータベースを引く。
///
/// 逆引きで名前を得てから本体を引くので 2 段になるが、どちらの段も 1 回の
/// `batch_get` に束ねられる（往復は**件数によらず 2 回**）。
#[tracing::instrument(skip_all, fields(ids = ids.len()))]
pub(super) async fn databases_by_id<R: Reader>(
    txn: &Readers<R>,
    ids: &[DatabaseId],
) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError> {
    let names = database_names(txn, ids).await?;
    if names.is_empty() {
        return Ok(Vec::new());
    }

    let by_key: BTreeMap<Vec<u8>, DatabaseId> = names
        .iter()
        .map(|(&db_id, name)| (keys::database(name), db_id))
        .collect();

    let mut out = Vec::with_capacity(names.len());
    for (key, bytes) in kv::batch_get(txn, by_key.keys().cloned().collect()).await? {
        let Some(&db_id) = by_key.get(&key) else {
            continue;
        };
        let meta: DatabaseMetadata = decode(Stored::DatabaseEntry, &bytes)?;
        // 名前が別のデータベースへ付け替わっていたら採らない。
        if meta.id != db_id {
            continue;
        }
        out.push((
            db_id,
            DatabaseInfoResponse {
                name: names[&db_id].clone(),
                description: meta.description,
            },
        ));
    }
    Ok(out)
}

/// このデータベース配下の、指定した ID のテーブルだけ。配下全件は舐めない。
#[tracing::instrument(skip_all, fields(db_id = %db_id, ids = ids.len()))]
pub(super) async fn tables_by_id<R: Reader>(
    txn: &Readers<R>,
    db_id: DatabaseId,
    ids: &[TableId],
) -> Result<Vec<Table>, AppError> {
    let entries = table_entries(txn, ids).await?;

    let by_key: BTreeMap<Vec<u8>, TableId> = entries
        .iter()
        .filter(|(_, (owner, _))| *owner == db_id)
        .map(|(&table_id, (_, name))| (keys::table(db_id, name), table_id))
        .collect();
    if by_key.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::with_capacity(by_key.len());
    for (key, bytes) in kv::batch_get(txn, by_key.keys().cloned().collect()).await? {
        let Some(table_id) = by_key.get(&key) else {
            continue;
        };
        let meta: TableMetadata = decode(Stored::TableEntry, &bytes)?;
        if meta.id != *table_id {
            continue;
        }
        out.push(Table::from_meta(&entries[table_id].1, meta));
    }
    Ok(out)
}

#[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
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
        .ok_or_else(|| Resource::Database.not_found(db_name.to_string()))?;
    Ok(table_meta(txn, db_meta.id, table_name)
        .await?
        .map(|meta| Table::from_meta(table_name, meta)))
}

/// 往復は**参照数によらず 2 回**。データベース名の解決なしにテーブルキーを組み立てられない
/// ので段は分かれるが、同じ段の中では 1 回の `batch_get` に束ねられる。
#[tracing::instrument(skip_all, fields(refs = refs.len()))]
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
            db_ids.insert(
                name,
                decode::<DatabaseMetadata>(Stored::DatabaseEntry, &bytes)?.id,
            );
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
        metas.insert(key, decode(Stored::TableEntry, &bytes)?);
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

#[tracing::instrument(skip_all, fields(db_name = %name))]
pub(super) async fn database_info<R: Reader>(
    txn: &Readers<R>,
    name: &str,
) -> Result<Option<(DatabaseId, DatabaseInfoResponse)>, AppError> {
    Ok(database_meta(txn, name).await?.map(|meta| {
        (
            meta.id,
            DatabaseInfoResponse {
                name: name.to_string(),
                description: meta.description,
            },
        )
    }))
}

#[tracing::instrument(skip_all, fields(db_id = %db_id))]
pub(super) async fn table_list_by_id<R: Reader>(
    txn: &Readers<R>,
    db_id: DatabaseId,
) -> Result<Vec<Table>, AppError> {
    let entries = kv::scan_prefix(txn, &keys::tables_of(db_id)).await?;
    entries
        .iter()
        .map(|(key, value)| {
            let name = keys::table_name_from_key(key)?;
            Ok(Table::from_meta(name, decode(Stored::TableEntry, value)?))
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
                let meta: DatabaseMetadata = decode(Stored::DatabaseEntry, value)?;
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
        let db_id = database_id(&self.txn, db_name)
            .await?
            .ok_or_else(|| Resource::Database.not_found(db_name.to_string()))?;
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
            return Err(Resource::Database.already_exists(name.to_string()));
        }

        let meta = DatabaseMetadata {
            id,
            description: description.clone(),
        };
        kv::put(
            &self.txn,
            keys::database(name),
            encode(Stored::DatabaseEntry, &meta)?,
        )
        .await;
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

    #[tracing::instrument(skip_all, fields(db_name = %name))]
    pub(super) async fn database_remove_impl(&mut self, name: &str) -> Result<(), AppError> {
        if name.is_empty() {
            return Err(Resource::Database.not_found(name.to_string()));
        }

        self.require_lock(LockScope::Database, name.as_bytes())?;

        let Some(meta) = database_meta(&self.txn, name).await? else {
            return Err(Resource::Database.not_found(name.to_string()));
        };

        // データベーススコープを保持しているので、この一覧が途中で増えることはない。
        let tables = table_list_by_id(&self.txn, meta.id).await?;
        for table in tables {
            self.retire_table(meta.id, &table.name, table.id).await?;
        }

        // データベーススコープの行を落とす（配下テーブルの行は上のループで消えている）。
        // ID は再利用されないので取り残しても誤爆はしないが、残すと持ち主に
        // 「配下のどれかに届く」と見え続けてしまう。
        self.acl_remove_object_impl(meta.id, None).await?;

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

        // 片方だけでは、逆向きの同時改名で両者が相手の名前へ書き込みうる。
        self.require_locks([
            (LockScope::Database, name.as_bytes()),
            (LockScope::Database, final_new_name.as_bytes()),
        ])?;

        let Some(mut meta) = database_meta(&self.txn, name).await? else {
            return Err(Resource::Database.not_found(name.to_string()));
        };

        if name != final_new_name && database_meta(&self.txn, final_new_name).await?.is_some() {
            return Err(Resource::Database.already_exists(final_new_name.to_string()));
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
            encode(Stored::DatabaseEntry, &meta)?,
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
            .ok_or_else(|| Resource::Database.not_found(src_db_name.to_string()))?;

        if database_meta(&self.txn, copy_name).await?.is_some() {
            return Err(Resource::Database.already_exists(copy_name.to_string()));
        }

        let src_tables = table_list_by_id(&self.txn, src_meta.id).await?;

        let copy_db_id = DatabaseId(uuid::Uuid::now_v7());
        let copy_meta = DatabaseMetadata {
            id: copy_db_id,
            description: src_meta.description.clone(),
        };
        kv::put(
            &self.txn,
            keys::database(copy_name),
            encode(Stored::DatabaseEntry, &copy_meta)?,
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
        is_temporal: bool,
    ) -> Result<Table, AppError> {
        if db_name.is_empty() {
            return Err(Resource::Database.not_found(db_name.to_string()));
        }
        if table_name.is_empty() {
            return Err(AppError::InvalidName {
                reason: "table name cannot be empty".to_string(),
            });
        }

        self.require_lock(LockScope::Database, db_name.as_bytes())?;

        let db_meta = database_meta(&self.txn, db_name)
            .await?
            .ok_or_else(|| Resource::Database.not_found(db_name.to_string()))?;

        if table_meta(&self.txn, db_meta.id, table_name)
            .await?
            .is_some()
        {
            return Err(Resource::Table.already_exists(table_name.to_string()));
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
            is_temporal,
        };
        kv::put(
            &self.txn,
            keys::table(db_meta.id, table_name),
            encode(Stored::TableEntry, &meta)?,
        )
        .await;
        kv::put(
            &self.txn,
            keys::table_id_index(id),
            OwnedName::new(db_meta.id, table_name).into_bytes(),
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
            is_temporal,
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
        is_temporal: Option<bool>,
    ) -> Result<Table, AppError> {
        // 改名はテーブル集合の変更なので排他する（既存データ検証のほうは排他しない）。
        self.require_lock(LockScope::Database, db_name.as_bytes())?;

        let db_meta = database_meta(&self.txn, db_name)
            .await?
            .ok_or_else(|| Resource::Database.not_found(db_name.to_string()))?;
        let mut table = table_meta(&self.txn, db_meta.id, table_name)
            .await?
            .map(|meta| Table::from_meta(table_name, meta))
            .ok_or_else(|| Resource::Table.not_found(table_name.to_string()))?;

        let changed_name = match new_name {
            Some(nn) if nn != table_name => {
                if table_meta(&self.txn, db_meta.id, nn).await?.is_some() {
                    return Err(Resource::Table.already_exists(nn.to_string()));
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

            self.validate_existing_data(table.id, table.data_type, table.constraints.as_ref())
                .await?;
        }

        if let Some(desc) = description {
            table.description = desc;
        }

        if let Some(v) = is_temporal {
            table.is_temporal = v;
        }

        let meta = TableMetadata {
            id: table.id,
            data_type: table.data_type,
            max_zoom_level: table.max_zoom_level,
            constraints: table.constraints.clone(),
            description: table.description.clone(),
            value_index: table.value_index,
            is_temporal: table.is_temporal,
        };

        if changed_name {
            kv::delete(&self.txn, keys::table(db_meta.id, table_name)).await;
            kv::put(
                &self.txn,
                keys::table_id_index(table.id),
                OwnedName::new(db_meta.id, &table.name).into_bytes(),
            )
            .await;
        }
        kv::put(
            &self.txn,
            keys::table(db_meta.id, &table.name),
            encode(Stored::TableEntry, &meta)?,
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
        // シャード実体は論理削除して回収に回すので、テーブル全体の排他は要らない。
        self.require_lock(LockScope::Database, db_name.as_bytes())?;

        let db_meta = database_meta(&self.txn, db_name)
            .await?
            .ok_or_else(|| Resource::Database.not_found(db_name.to_string()))?;
        let table = table_meta(&self.txn, db_meta.id, table_name)
            .await?
            .ok_or_else(|| Resource::Table.not_found(table_name.to_string()))?;

        self.retire_table(db_meta.id, table_name, table.id).await?;
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
            .ok_or_else(|| Resource::Database.not_found(src_db_name.to_string()))?;
        // 存在確認だけ。実体の読み出しは `copy_table_into` が行う。
        table_meta(&self.txn, src_db.id, src_table_name)
            .await?
            .ok_or_else(|| Resource::Table.not_found(src_table_name.to_string()))?;

        let copy_db = database_meta(&self.txn, copy_db_name)
            .await?
            .ok_or_else(|| Resource::Database.not_found(copy_db_name.to_string()))?;

        if table_meta(&self.txn, copy_db.id, copy_table_name)
            .await?
            .is_some()
        {
            return Err(Resource::Table.already_exists(copy_table_name.to_string()));
        }

        self.copy_table_into(src_db.id, src_table_name, copy_db.id, copy_table_name)
            .await
    }

    /// コピー元のシャードは**スナップショットから読む**（ロックを取らない）。
    ///
    /// 得られるのはトランザクション開始時点の一貫したコピーで、コピー中に走った書き込みは
    /// 含まれない。テーブル全体を排他する意味論もありうるが、複数インスタンス前提では代償が
    /// 大きすぎる。
    async fn copy_table_into(
        &mut self,
        src_db_id: DatabaseId,
        src_table_name: &str,
        dst_db_id: DatabaseId,
        dst_table_name: &str,
    ) -> Result<Table, AppError> {
        let src_meta = table_meta(&self.txn, src_db_id, src_table_name)
            .await?
            .ok_or_else(|| Resource::Table.not_found(src_table_name.to_string()))?;

        let copy_id = TableId(uuid::Uuid::now_v7());
        let copy_meta = TableMetadata {
            id: copy_id,
            data_type: src_meta.data_type,
            max_zoom_level: src_meta.max_zoom_level,
            constraints: src_meta.constraints.clone(),
            description: src_meta.description.clone(),
            value_index: src_meta.value_index,
            is_temporal: src_meta.is_temporal,
        };

        kv::put(
            &self.txn,
            keys::table(dst_db_id, dst_table_name),
            encode(Stored::TableEntry, &copy_meta)?,
        )
        .await;
        kv::put(
            &self.txn,
            keys::table_id_index(copy_id),
            OwnedName::new(dst_db_id, dst_table_name).into_bytes(),
        )
        .await;

        // 一括で決めさせるのは、名前空間が増えたとき複製だけが追随し損ねるのを防ぐため。
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
            is_temporal: src_meta.is_temporal,
        })
    }

    /// テーブルを**論理削除**する。実体の削除は [`gc`](super::gc) が後から行う。
    ///
    /// `TableId` は UUIDv7 なので、回収前に飛び込んだ書き込みが残骸を作っても次の掃除で回収
    /// される。
    async fn retire_table(
        &mut self,
        db_id: DatabaseId,
        table_name: &str,
        table_id: TableId,
    ) -> Result<(), AppError> {
        // このテーブルを指す ACL 行を保持者を問わず落とす。実体の回収は後からでよいが、
        // 権限は「消えたテーブルに届いたまま」にしない。
        self.acl_remove_object_impl(db_id, Some(table_id)).await?;

        kv::delete_many(
            &self.txn,
            [
                keys::table(db_id, table_name),
                keys::table_id_index(table_id),
            ],
        )
        .await;
        kv::put_many(&self.txn, [super::gc::retire(table_id)]).await;
        Ok(())
    }
}
