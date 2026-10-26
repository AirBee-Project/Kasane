//! `_impl` を付けるのは、trait メソッドと同名にすると inherent 側が常に優先されて
//! trait メソッドを呼べなくなるため。

use std::collections::HashMap;

use heed::BytesDecode;
use uuid::Uuid;

use crate::error::{AppError, Resource, Stored};
use crate::models::database::table::{Table, TableConstraints, TableDataType, TableMetadata};
use crate::models::database::{DatabaseInfoResponse, DatabaseMetadata};
use crate::models::id::{DatabaseId, TableId};

use super::keys::DbIdAndName;
use super::{AppDb, KasaneDbRead, KasaneDbWrite};

/// `RwTxn` は `RoTxn` へ Deref するので、読み書きどちらからも同じ関数を呼べる。
pub(super) fn database_id(
    db: &AppDb,
    txn: &heed::RoTxn<heed::WithoutTls>,
    name: &str,
) -> Result<Option<DatabaseId>, AppError> {
    if name.is_empty() {
        return Ok(None);
    }
    Ok(db.databases.get(txn, name)?.map(|meta| meta.id))
}

pub(super) fn table_id(
    db: &AppDb,
    txn: &heed::RoTxn<heed::WithoutTls>,
    db_id: DatabaseId,
    table_name: &str,
) -> Result<Option<TableId>, AppError> {
    if table_name.is_empty() {
        return Ok(None);
    }
    Ok(db
        .tables
        .get(txn, &(db_id, table_name))?
        .map(|meta| meta.id))
}

pub(super) fn database_name(
    db: &AppDb,
    txn: &heed::RoTxn<heed::WithoutTls>,
    db_id: DatabaseId,
) -> Result<Option<String>, AppError> {
    Ok(db.database_id_index.get(txn, &db_id)?.map(str::to_string))
}

/// 所属データベースも返すのは、権限の描画で「本当にそのデータベースの配下か」を
/// 確かめられるようにするため。
pub(super) fn table_entry(
    db: &AppDb,
    txn: &heed::RoTxn<heed::WithoutTls>,
    table_id: TableId,
) -> Result<Option<(DatabaseId, String)>, AppError> {
    Ok(db
        .table_id_index
        .get(txn, &table_id)?
        .map(|(db_id, name)| (db_id, name.to_string())))
}

pub(super) fn database_names(
    db: &AppDb,
    txn: &heed::RoTxn<heed::WithoutTls>,
    ids: &[DatabaseId],
) -> Result<HashMap<DatabaseId, String>, AppError> {
    let mut out = HashMap::with_capacity(ids.len());
    for &db_id in ids {
        if let Some(name) = database_name(db, txn, db_id)? {
            out.insert(db_id, name);
        }
    }
    Ok(out)
}

pub(super) fn table_entries(
    db: &AppDb,
    txn: &heed::RoTxn<heed::WithoutTls>,
    ids: &[TableId],
) -> Result<HashMap<TableId, (DatabaseId, String)>, AppError> {
    let mut out = HashMap::with_capacity(ids.len());
    for &table_id in ids {
        if let Some(entry) = table_entry(db, txn, table_id)? {
            out.insert(table_id, entry);
        }
    }
    Ok(out)
}

/// リーフの件数ヘッダを合算する（リーフ本体は読まない）。
#[tracing::instrument(skip_all)]
pub(super) fn table_count(
    db: &AppDb,
    txn: &heed::RoTxn<heed::WithoutTls>,
    table_id: TableId,
) -> Result<u64, AppError> {
    let mut total = 0u64;
    for item in db
        .tables_data
        .remap_key_type::<heed::types::Bytes>()
        .prefix_iter(txn, table_id.0.as_bytes())?
    {
        let (_, bytes) = item?;
        if let Some(count) = super::shard::ShardEntry::leaf_count(bytes)? {
            total += count as u64;
        }
    }
    Ok(total)
}

/// 全走査に依存する逆引きは持たず、`tables` のプレフィックススキャンで済ませる。
pub(super) fn table_names(
    db: &AppDb,
    txn: &heed::RoTxn<heed::WithoutTls>,
    db_id: DatabaseId,
) -> Result<Vec<String>, AppError> {
    let prefix = db_id.into_bytes();
    let mut names = Vec::new();
    for item in db
        .tables
        .remap_types::<heed::types::Bytes, heed::types::Bytes>()
        .prefix_iter(txn, prefix.as_slice())?
    {
        let (k_bytes, _) = item?;
        let (_, name) = DbIdAndName::bytes_decode(k_bytes)
            .map_err(|e| AppError::corrupt(Stored::TableEntry, e))?;
        names.push(name.to_string());
    }
    Ok(names)
}

impl<'a> KasaneDbRead<'a> {
    #[tracing::instrument(skip_all)]
    pub fn database_info_impl(
        &self,
        name: &str,
    ) -> Result<Option<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        if name.is_empty() {
            return Ok(None);
        }
        Ok(self.db.databases.get(&self.read_txn, name)?.map(|meta| {
            (
                meta.id,
                DatabaseInfoResponse {
                    name: name.to_string(),
                    description: meta.description,
                },
            )
        }))
    }

    /// ACL 側から辿った ID で引く。存在しない ID は結果に現れない。
    #[tracing::instrument(skip_all, fields(ids = ids.len()))]
    pub fn databases_by_id_impl(
        &self,
        ids: &[DatabaseId],
    ) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        let mut out = Vec::with_capacity(ids.len());
        for &db_id in ids {
            // 逆引きで名前を得てから本体を引く。名前が本体の鍵なので 2 段になる。
            let Some(name) = database_name(self.db, &self.read_txn, db_id)? else {
                continue;
            };
            if let Some(meta) = self.db.databases.get(&self.read_txn, name.as_str())? {
                out.push((
                    db_id,
                    DatabaseInfoResponse {
                        name,
                        description: meta.description,
                    },
                ));
            }
        }
        Ok(out)
    }

    /// このデータベース配下の、指定した ID のテーブルだけ。配下全件は舐めない。
    #[tracing::instrument(skip_all, fields(ids = ids.len()))]
    pub fn tables_by_id_impl(
        &self,
        db_id: DatabaseId,
        ids: &[TableId],
    ) -> Result<Vec<Table>, AppError> {
        let mut out = Vec::with_capacity(ids.len());
        for &table_id in ids {
            let Some((owner, name)) = table_entry(self.db, &self.read_txn, table_id)? else {
                continue;
            };
            if owner != db_id {
                continue;
            }
            if let Some(meta) = self
                .db
                .tables
                .get(&self.read_txn, &(db_id, name.as_str()))?
            {
                out.push(Table::from_meta(&name, meta));
            }
        }
        Ok(out)
    }

    /// 呼び出し側が権限の絞り込みに使うので、ID を添えて返す。
    #[tracing::instrument(skip_all)]
    pub fn database_list_impl(&self) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        let db = self.db.databases;
        let mut list = Vec::new();
        for res in db.iter(&self.read_txn)? {
            let (k, meta) = res.map_err(AppError::from)?;
            list.push((
                meta.id,
                DatabaseInfoResponse {
                    name: k.to_string(),
                    description: meta.description,
                },
            ));
        }
        Ok(list)
    }
}

impl<'a> KasaneDbWrite<'a> {
    #[tracing::instrument(skip_all)]
    pub fn database_info_impl(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError> {
        let db = self.db.databases;
        if let Some(meta) = db.get(&self.write_txn, name)? {
            Ok(Some(DatabaseInfoResponse {
                name: name.to_string(),
                description: meta.description,
            }))
        } else {
            Ok(None)
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn database_create_impl(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseInfoResponse, AppError> {
        if self.database_info_impl(name)?.is_some() {
            return Err(Resource::Database.already_exists(name.to_string()));
        }

        let id = Uuid::now_v7();
        let meta = DatabaseMetadata {
            id: crate::models::id::DatabaseId(id),
            description: description.clone(),
        };

        let db_id = crate::models::id::DatabaseId(id);
        self.db.databases.put(&mut self.write_txn, name, &meta)?;
        self.db
            .database_id_index
            .put(&mut self.write_txn, &db_id, name)?;

        Ok(DatabaseInfoResponse {
            name: name.to_string(),
            description,
        })
    }

    #[tracing::instrument(skip_all)]
    pub fn database_remove_impl(&mut self, name: &str) -> Result<(), AppError> {
        if name.is_empty() {
            return Err(Resource::Database.not_found(name.to_string()));
        }
        let Some(meta) = self.db.databases.get(&self.write_txn, name)? else {
            return Err(Resource::Database.not_found(name.to_string()));
        };

        // 別のトランザクションで列挙すると、隙間に作られたテーブルが削除対象から漏れる。
        for table_name in table_names(self.db, &self.write_txn, meta.id)? {
            self.table_remove_impl(name, &table_name)?;
        }

        // データベーススコープの行を落とす（配下テーブルの行は上のループで消えている）。
        // ID は再利用されないので取り残しても誤爆はしないが、残すと持ち主に
        // 「配下のどれかに届く」と見え続けてしまう。
        self.acl_remove_object_impl(meta.id, None)?;

        self.db.databases.delete(&mut self.write_txn, name)?;
        self.db
            .database_id_index
            .delete(&mut self.write_txn, &meta.id)?;

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub fn database_update_impl(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<DatabaseInfoResponse, AppError> {
        let final_new_name = new_name.as_deref().unwrap_or(name);

        if name != final_new_name {
            crate::services::helpers::name_valid::name_valid(final_new_name)?;
        }

        let mut meta = {
            let db = self.db.databases;
            if let Some(meta) = db.get(&self.write_txn, name)? {
                meta
            } else {
                return Err(Resource::Database.not_found(name.to_string()));
            }
        };

        if name != final_new_name {
            let db = self.db.databases;
            if db.get(&self.write_txn, final_new_name)?.is_some() {
                return Err(Resource::Database.already_exists(final_new_name.to_string()));
            }
        }

        if let Some(desc) = description {
            meta.description = desc;
        }

        let db = self.db.databases;
        if name != final_new_name {
            db.delete(&mut self.write_txn, name)?;
            self.db
                .database_id_index
                .put(&mut self.write_txn, &meta.id, final_new_name)?;
        }
        db.put(&mut self.write_txn, final_new_name, &meta)?;
        Ok(DatabaseInfoResponse {
            name: final_new_name.to_string(),
            description: meta.description,
        })
    }
}

impl<'a> KasaneDbRead<'a> {
    /// すべて mmap 上の点参照なので束ねる意味はなく、入口を揃えるためだけにある。
    #[tracing::instrument(skip_all, fields(refs = refs.len()))]
    pub fn resolve_tables_impl(
        &self,
        refs: &[(String, String)],
    ) -> Result<Vec<crate::repositories::ResolvedTable>, AppError> {
        refs.iter()
            .map(|(db_name, table_name)| {
                let db_meta = if db_name.is_empty() {
                    None
                } else {
                    self.db.databases.get(&self.read_txn, db_name)?
                };
                let Some(db_meta) = db_meta else {
                    return Ok(crate::repositories::ResolvedTable {
                        db_id: None,
                        table: None,
                    });
                };
                let table = if table_name.is_empty() {
                    None
                } else {
                    self.db
                        .tables
                        .get(&self.read_txn, &(db_meta.id, table_name.as_str()))?
                        .map(|meta| Table::from_meta(table_name, meta))
                };
                Ok(crate::repositories::ResolvedTable {
                    db_id: Some(db_meta.id),
                    table,
                })
            })
            .collect()
    }

    #[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
    pub fn table_info_impl(
        &self,
        db_name: &str,
        table_name: &str,
    ) -> Result<Option<Table>, AppError> {
        if db_name.is_empty() {
            return Ok(None);
        }
        let db_meta = {
            let db = self.db.databases;
            if let Some(m) = db.get(&self.read_txn, db_name)? {
                m
            } else {
                return Err(Resource::Database.not_found(db_name.to_string()));
            }
        };

        let db_tables = self.db.tables;
        if let Some(m) = db_tables.get(&self.read_txn, &(db_meta.id, table_name))? {
            Ok(Some(Table::from_meta(table_name, m)))
        } else {
            Ok(None)
        }
    }

    #[tracing::instrument(skip_all, fields(db_name = %db_name))]
    pub fn table_list_impl(&self, db_name: &str) -> Result<Vec<Table>, AppError> {
        let db_id = database_id(self.db, &self.read_txn, db_name)?
            .ok_or_else(|| Resource::Database.not_found(db_name.to_string()))?;
        self.table_list_by_id_impl(db_id)
    }

    /// ID を解決済みの呼び出し側が、名前からの引き直しを避けるために使う。
    #[tracing::instrument(skip_all)]
    pub fn table_list_by_id_impl(
        &self,
        db_id: crate::models::id::DatabaseId,
    ) -> Result<Vec<Table>, AppError> {
        let db_id_bytes = db_id.into_bytes();
        let db = self.db.tables;

        let mut tables = Vec::new();
        for iter in db
            .remap_types::<heed::types::Bytes, heed::types::Bytes>()
            .prefix_iter(&self.read_txn, db_id_bytes.as_slice())?
        {
            let (k_bytes, v_bytes) = iter?;
            let (_, name) = super::keys::DbIdAndName::bytes_decode(k_bytes).unwrap();
            let m = heed::types::SerdeJson::<crate::models::database::table::TableMetadata>::bytes_decode(v_bytes).unwrap();
            tables.push(Table {
                id: m.id,
                name: name.to_string(),
                data_type: m.data_type,
                max_zoom_level: m.max_zoom_level,
                constraints: m.constraints,
                description: m.description,
                value_index: m.value_index,
                is_temporal: m.is_temporal,
            });
        }
        Ok(tables)
    }

    pub fn table_count_impl(&self, table_id: TableId) -> Result<u64, AppError> {
        table_count(self.db, &self.read_txn, table_id)
    }
}

impl<'a> KasaneDbWrite<'a> {
    #[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
    pub fn table_info_impl(
        &self,
        db_name: &str,
        table_name: &str,
    ) -> Result<Option<Table>, AppError> {
        if db_name.is_empty() {
            return Ok(None);
        }
        let db_meta = {
            let db = self.db.databases;
            if let Some(m) = db.get(&self.write_txn, db_name)? {
                m
            } else {
                return Err(Resource::Database.not_found(db_name.to_string()));
            }
        };

        let db = self.db.tables;
        if let Some(m) = db.get(&self.write_txn, &(db_meta.id, table_name))? {
            Ok(Some(Table::from_meta(table_name, m)))
        } else {
            Ok(None)
        }
    }

    #[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
    #[allow(clippy::too_many_arguments)]
    pub fn table_create_impl(
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
            return Err(AppError::InvalidArgument {
                reason: "table name cannot be empty".to_string(),
            });
        }
        let db_meta = {
            let db = self.db.databases;
            if let Some(m) = db.get(&self.write_txn, db_name)? {
                m
            } else {
                return Err(Resource::Database.not_found(db_name.to_string()));
            }
        };

        if self.table_info_impl(db_name, table_name)?.is_some() {
            return Err(Resource::Table.already_exists(table_name.to_string()));
        }

        let db_index = self.db.table_id_index;

        let mut id = crate::models::id::TableId(uuid::Uuid::now_v7());
        loop {
            if db_index.get(&self.write_txn, &id)?.is_none() {
                break;
            }
            id = crate::models::id::TableId(uuid::Uuid::now_v7());
        }

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

        let db = self.db.tables;
        db.put(&mut self.write_txn, &(db_meta.id, table_name), &meta)?;
        self.db
            .table_id_index
            .put(&mut self.write_txn, &id, &(db_meta.id, table_name))?;

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
    pub fn table_update_impl(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<TableConstraints>>,
        description: Option<Option<String>>,
        is_temporal: Option<bool>,
    ) -> Result<Table, AppError> {
        let db_meta = {
            let db = self.db.databases;
            db.get(&self.write_txn, db_name)?
                .ok_or_else(|| Resource::Database.not_found(db_name.to_string()))?
        };

        let mut table = {
            self.table_info_impl(db_name, table_name)?
                .ok_or_else(|| Resource::Table.not_found(table_name.to_string()))?
        };

        let changed_name = if let Some(nn) = new_name {
            if nn != table_name {
                if self.table_info_impl(db_name, nn)?.is_some() {
                    return Err(Resource::Table.already_exists(nn.to_string()));
                }
                table.name = nn.to_string();
                true
            } else {
                false
            }
        } else {
            false
        };

        if let Some(nc_opt) = new_constraints {
            let new_c = match nc_opt {
                None => None,
                Some(nc) => {
                    TableConstraints::replaced_with(table.data_type, table.constraints.as_ref(), nc)
                        .map_err(|reason| AppError::ConstraintViolation { reason })?
                }
            };

            if let Some(c) = &new_c
                && let Err(msg) = c.validate()
            {
                return Err(AppError::ConstraintViolation { reason: msg });
            }

            table.constraints = new_c;

            self.validate_table_existing_data(
                table.id,
                table.data_type,
                table.constraints.as_ref(),
            )?;
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

        let db = self.db.tables;
        if changed_name {
            db.delete(&mut self.write_txn, &(db_meta.id, table_name))?;
            self.db.table_id_index.put(
                &mut self.write_txn,
                &table.id,
                &(db_meta.id, table.name.as_str()),
            )?;
        }
        db.put(&mut self.write_txn, &(db_meta.id, &table.name), &meta)?;

        Ok(table)
    }

    #[tracing::instrument(skip_all, fields(table_id = %table_id))]
    fn validate_table_existing_data(
        &self,
        table_id: crate::models::id::TableId,
        data_type: TableDataType,
        constraints: Option<&TableConstraints>,
    ) -> Result<(), AppError> {
        let tables_data = self
            .db
            .tables_data
            .remap_types::<heed::types::Bytes, heed::types::Bytes>();
        let prefix = table_id.into_bytes();

        for iter in tables_data.prefix_iter(&self.write_txn, prefix.as_slice())? {
            let (_, v_bytes) = iter?;
            use super::shard::ShardEntry;
            match ShardEntry::decode(v_bytes)? {
                ShardEntry::Leaf(map_bytes) => {
                    for &bytes in super::shard::LeafView::parse(&map_bytes)?.values() {
                        let restored_json = crate::services::helpers::value::restore_value(
                            data_type,
                            constraints,
                            bytes,
                        )?;
                        crate::services::helpers::value::interpret_value(
                            data_type,
                            constraints,
                            restored_json,
                        )?;
                    }
                }
                ShardEntry::Pointers(_) => {}
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
    pub fn table_remove_impl(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError> {
        let table = match self.table_info_impl(db_name, table_name)? {
            Some(t) => t,
            None => {
                return Err(Resource::Table.not_found(table_name.to_string()));
            }
        };

        let db_meta = {
            let db = self.db.databases;
            db.get(&self.write_txn, db_name)?
                .ok_or_else(|| Resource::Database.not_found(db_name.to_string()))?
        };

        // 反復中に削除できないので、キーを集めてから削除する。
        let tables_data = self
            .db
            .tables_data
            .remap_types::<heed::types::Bytes, heed::types::Bytes>();
        let prefix = table.id.into_bytes();
        let keys: Vec<Vec<u8>> = {
            let mut ks = Vec::new();
            for iter in tables_data.prefix_iter(&self.write_txn, prefix.as_slice())? {
                let (k_bytes, _) = iter?;
                ks.push(k_bytes.to_vec());
            }
            ks
        };
        for k in keys {
            tables_data.delete(&mut self.write_txn, &k)?;
        }

        let value_index = self.db.value_index;
        let vi_keys: Vec<Vec<u8>> = {
            let mut ks = Vec::new();
            for iter in value_index.prefix_iter(&self.write_txn, prefix.as_slice())? {
                let (k_bytes, _) = iter?;
                ks.push(k_bytes.to_vec());
            }
            ks
        };
        for k in vi_keys {
            value_index.delete(&mut self.write_txn, &k)?;
        }

        // このテーブルを指す ACL 行を保持者を問わず落とす。
        self.acl_remove_object_impl(db_meta.id, Some(table.id))?;

        self.db
            .tables
            .delete(&mut self.write_txn, &(db_meta.id, table_name))?;
        self.db
            .table_id_index
            .delete(&mut self.write_txn, &table.id)?;

        Ok(())
    }
}
