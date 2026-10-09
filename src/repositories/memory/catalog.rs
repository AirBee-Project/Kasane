//! `_impl` を付けるのは、trait メソッドと同名にすると inherent 側が常に優先されて
//! trait メソッドを呼べなくなるため（LMDB・TiKV 実装も同じ規約）。

use std::collections::HashMap;

use uuid::Uuid;

use crate::error::{AppError, Resource};
use crate::models::database::table::{
    Table, TableConstraints, TableDataType, TableMetadata, UpdateTableConstraints,
};
use crate::models::database::{DatabaseInfoResponse, DatabaseMetadata};
use crate::models::id::{DatabaseId, TableId};
use crate::repositories::ResolvedTable;
use crate::services::helpers::name_valid::name_valid;
use crate::services::helpers::value::{interpret_value, restore_value};

use super::State;

impl State {
    pub(super) fn database_id_impl(&self, name: &str) -> Option<DatabaseId> {
        self.databases.get(name).map(|meta| meta.id)
    }

    pub(super) fn table_id_impl(&self, db_id: DatabaseId, table_name: &str) -> Option<TableId> {
        self.tables
            .get(&(db_id, table_name.to_string()))
            .map(|meta| meta.id)
    }

    /// 存在しない ID は結果に現れない。
    pub(super) fn database_names_impl(&self, ids: &[DatabaseId]) -> HashMap<DatabaseId, String> {
        self.databases
            .iter()
            .filter(|(_, meta)| ids.contains(&meta.id))
            .map(|(name, meta)| (meta.id, name.clone()))
            .collect()
    }

    /// テーブル ID から所属データベースと名前を引く。存在しない ID は結果に現れない。
    pub(super) fn table_entries_impl(
        &self,
        ids: &[TableId],
    ) -> HashMap<TableId, (DatabaseId, String)> {
        self.tables
            .iter()
            .filter(|(_, meta)| ids.contains(&meta.id))
            .map(|((db_id, name), meta)| (meta.id, (*db_id, name.clone())))
            .collect()
    }

    pub(super) fn database_info_impl(
        &self,
        name: &str,
    ) -> Option<(DatabaseId, DatabaseInfoResponse)> {
        self.databases.get(name).map(|meta| {
            (
                meta.id,
                DatabaseInfoResponse {
                    name: name.to_string(),
                    description: meta.description.clone(),
                },
            )
        })
    }

    pub(super) fn database_list_impl(&self) -> Vec<(DatabaseId, DatabaseInfoResponse)> {
        self.databases
            .keys()
            .filter_map(|name| self.database_info_impl(name))
            .collect()
    }

    /// 返りは `ids` の並び。存在しない ID は結果に現れない。
    pub(super) fn databases_by_id_impl(
        &self,
        ids: &[DatabaseId],
    ) -> Vec<(DatabaseId, DatabaseInfoResponse)> {
        let all = self.database_list_impl();
        ids.iter()
            .filter_map(|id| all.iter().find(|(db_id, _)| db_id == id).cloned())
            .collect()
    }

    /// `db_name` が空なら `Ok(None)`、データベースが無ければエラー。
    pub(super) fn table_info_impl(
        &self,
        db_name: &str,
        table_name: &str,
    ) -> Result<Option<Table>, AppError> {
        if db_name.is_empty() {
            return Ok(None);
        }
        let db_id = self.database(db_name)?.id;
        Ok(self
            .tables
            .get(&(db_id, table_name.to_string()))
            .map(|meta| Table::from_meta(table_name, meta.clone())))
    }

    /// 返りは `refs` と同じ並び。
    pub(super) fn resolve_tables_impl(&self, refs: &[(String, String)]) -> Vec<ResolvedTable> {
        refs.iter()
            .map(|(db_name, table_name)| {
                let db_id = self.database_id_impl(db_name);
                let table = db_id.and_then(|db_id| {
                    self.tables
                        .get(&(db_id, table_name.clone()))
                        .map(|meta| Table::from_meta(table_name, meta.clone()))
                });
                ResolvedTable { db_id, table }
            })
            .collect()
    }

    pub(super) fn table_list_impl(&self, db_name: &str) -> Result<Vec<Table>, AppError> {
        let db_id = self.database(db_name)?.id;
        Ok(self.table_list_by_id_impl(db_id))
    }

    pub(super) fn table_list_by_id_impl(&self, db_id: DatabaseId) -> Vec<Table> {
        self.tables_in(db_id)
            .map(|(name, meta)| Table::from_meta(name, meta.clone()))
            .collect()
    }

    /// このデータベース配下の、指定した ID のテーブルだけ。
    pub(super) fn tables_by_id_impl(&self, db_id: DatabaseId, ids: &[TableId]) -> Vec<Table> {
        ids.iter()
            .filter_map(|id| {
                self.tables_in(db_id)
                    .find(|(_, meta)| meta.id == *id)
                    .map(|(name, meta)| Table::from_meta(name, meta.clone()))
            })
            .collect()
    }

    pub(super) fn table_count_impl(&self, table_id: TableId) -> u64 {
        self.data.get(&table_id).map_or(0, |map| map.count() as u64)
    }

    pub(super) fn database_create_impl(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseInfoResponse, AppError> {
        if self.databases.contains_key(name) {
            return Err(Resource::Database.already_exists(name));
        }
        self.databases.insert(
            name.to_string(),
            DatabaseMetadata {
                id: DatabaseId(Uuid::now_v7()),
                description: description.clone(),
            },
        );
        Ok(DatabaseInfoResponse {
            name: name.to_string(),
            description,
        })
    }

    pub(super) fn database_remove_impl(&mut self, name: &str) -> Result<(), AppError> {
        let db_id = self.database(name)?.id;
        for table_name in self.table_names(db_id) {
            self.table_remove_impl(name, &table_name)?;
        }
        self.acl_remove_object_impl(db_id, None);
        self.databases.remove(name);
        Ok(())
    }

    pub(super) fn database_update_impl(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<(), AppError> {
        let new_name = new_name.as_deref().unwrap_or(name);
        if new_name != name {
            name_valid(new_name)?;
        }
        let mut meta = self.database(name)?.clone();
        if new_name != name && self.databases.contains_key(new_name) {
            return Err(Resource::Database.already_exists(new_name));
        }
        if let Some(description) = description {
            meta.description = description;
        }
        self.databases.remove(name);
        self.databases.insert(new_name.to_string(), meta);
        Ok(())
    }

    pub(super) fn database_copy_impl(
        &mut self,
        src_db_name: &str,
        copy_name: &str,
    ) -> Result<DatabaseInfoResponse, AppError> {
        name_valid(copy_name)?;
        let src = self.database(src_db_name)?.clone();
        let copy = self.database_create_impl(copy_name, src.description)?;
        for table_name in self.table_names(src.id) {
            self.table_copy_impl(src_db_name, &table_name, copy_name, &table_name)?;
        }
        Ok(copy)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn table_create_impl(
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
        if table_name.is_empty() {
            return Err(AppError::InvalidName {
                reason: "table name cannot be empty".to_string(),
            });
        }
        let db_id = self.database(db_name)?.id;
        let key = (db_id, table_name.to_string());
        if self.tables.contains_key(&key) {
            return Err(Resource::Table.already_exists(table_name));
        }

        let constraints = TableConstraints::with_enum_ids(data_type, constraints)
            .map_err(|reason| AppError::ConstraintViolation { reason })?;
        if let Some(c) = &constraints {
            c.validate()
                .map_err(|reason| AppError::ConstraintViolation { reason })?;
        }

        let meta = TableMetadata {
            id: TableId(Uuid::now_v7()),
            data_type,
            max_zoom_level,
            constraints,
            description,
            value_index,
            is_temporal,
        };
        self.tables.insert(key, meta.clone());
        Ok(Table::from_meta(table_name, meta))
    }

    pub(super) fn table_update_impl(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<UpdateTableConstraints>>,
        description: Option<Option<String>>,
        is_temporal: Option<bool>,
    ) -> Result<Table, AppError> {
        let db_id = self.database(db_name)?.id;
        let mut meta = self
            .tables
            .get(&(db_id, table_name.to_string()))
            .ok_or_else(|| Resource::Table.not_found(table_name))?
            .clone();

        let new_name = new_name.unwrap_or(table_name);
        if new_name != table_name && self.tables.contains_key(&(db_id, new_name.to_string())) {
            return Err(Resource::Table.already_exists(new_name));
        }

        if let Some(update) = new_constraints {
            meta.constraints = match update {
                None => None,
                Some(update) => {
                    TableConstraints::merged_with(meta.data_type, meta.constraints.as_ref(), update)
                        .map_err(|reason| AppError::ConstraintViolation { reason })?
                }
            };
            if let Some(c) = &meta.constraints {
                c.validate()
                    .map_err(|reason| AppError::ConstraintViolation { reason })?;
            }
            self.validate_existing_data(meta.id, meta.data_type, meta.constraints.as_ref())?;
        }
        if let Some(description) = description {
            meta.description = description;
        }
        if let Some(is_temporal) = is_temporal {
            meta.is_temporal = is_temporal;
        }

        self.tables.remove(&(db_id, table_name.to_string()));
        self.tables
            .insert((db_id, new_name.to_string()), meta.clone());
        Ok(Table::from_meta(new_name, meta))
    }

    pub(super) fn table_remove_impl(
        &mut self,
        db_name: &str,
        table_name: &str,
    ) -> Result<(), AppError> {
        let db_id = self.database(db_name)?.id;
        let meta = self
            .tables
            .remove(&(db_id, table_name.to_string()))
            .ok_or_else(|| Resource::Table.not_found(table_name))?;
        self.data.remove(&meta.id);
        self.acl_remove_object_impl(db_id, Some(meta.id));
        Ok(())
    }

    pub(super) fn table_copy_impl(
        &mut self,
        src_db_name: &str,
        src_table_name: &str,
        copy_db_name: &str,
        copy_table_name: &str,
    ) -> Result<Table, AppError> {
        let src_db_id = self.database(src_db_name)?.id;
        let src = self
            .tables
            .get(&(src_db_id, src_table_name.to_string()))
            .ok_or_else(|| Resource::Table.not_found(src_table_name))?
            .clone();
        let copy_db_id = self.database(copy_db_name)?.id;
        let key = (copy_db_id, copy_table_name.to_string());
        if self.tables.contains_key(&key) {
            return Err(Resource::Table.already_exists(copy_table_name));
        }
        name_valid(copy_table_name)?;

        let copy = TableMetadata {
            id: TableId(Uuid::now_v7()),
            ..src.clone()
        };
        if let Some(map) = self.data.get(&src.id).cloned() {
            self.data.insert(copy.id, map);
        }
        self.tables.insert(key, copy.clone());
        Ok(Table::from_meta(copy_table_name, copy))
    }

    fn database(&self, name: &str) -> Result<&DatabaseMetadata, AppError> {
        self.databases
            .get(name)
            .ok_or_else(|| Resource::Database.not_found(name))
    }

    fn tables_in(&self, db_id: DatabaseId) -> impl Iterator<Item = (&String, &TableMetadata)> {
        self.tables
            .range((db_id, String::new())..)
            .take_while(move |((owner, _), _)| *owner == db_id)
            .map(|((_, name), meta)| (name, meta))
    }

    /// 走査しながら書き換えられないので、名前だけを先に集める。
    fn table_names(&self, db_id: DatabaseId) -> Vec<String> {
        self.tables_in(db_id)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// 制約を変えたテーブルの既存値が、新しい制約の下でも読めて書けることを確かめる。
    fn validate_existing_data(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        constraints: Option<&TableConstraints>,
    ) -> Result<(), AppError> {
        let Some(map) = self.data.get(&table_id) else {
            return Ok(());
        };
        for (_, value) in map.iter() {
            let restored = restore_value(data_type, constraints, value)?;
            interpret_value(data_type, constraints, restored)?;
        }
        Ok(())
    }
}
