//! 抽象 trait への適合。委譲だけを行う。
//!
//! trait の実装は 1 ブロックにまとめる必要があるので、操作の定義（モジュール分割）と
//! 適合（ここ）を分けている（LMDB・TiKV 実装も同じ構成）。

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use kasane_logic::{FlexId, SpatialIdSet};

use crate::error::AppError;
use crate::models::database::DatabaseInfoResponse;
use crate::models::database::table::{
    Table, TableConstraints, TableDataType, UpdateTableConstraints,
};
use crate::models::id::{DataTarget, DatabaseId, PrincipalId, TableId};
use crate::models::users::{AclEntry, Grant, Scope, UserRecord};
use crate::repositories::{
    CatalogRepository, ReadRepository, ResolvedTable, ValueGroups, WriteRepository,
};

use super::State;

// 読み取り（`Arc<State>`）と書き込み（`State`）の両方に要る。読み取り側を `State` に
// 揃えると、両 trait にある `database_info` などの呼び出しがサービス層で曖昧になる。
macro_rules! impl_catalog_repository {
    ($target:ty) => {
        impl CatalogRepository for $target {
            async fn database_id(&self, name: &str) -> Result<Option<DatabaseId>, AppError> {
                Ok(self.database_id_impl(name))
            }

            async fn table_id(
                &self,
                db_id: DatabaseId,
                table_name: &str,
            ) -> Result<Option<TableId>, AppError> {
                Ok(self.table_id_impl(db_id, table_name))
            }

            async fn user_record(&self, username: &str) -> Result<Option<UserRecord>, AppError> {
                Ok(self.user_record_impl(username))
            }

            async fn database_names(
                &self,
                ids: &[DatabaseId],
            ) -> Result<HashMap<DatabaseId, String>, AppError> {
                Ok(self.database_names_impl(ids))
            }

            async fn table_entries(
                &self,
                ids: &[TableId],
            ) -> Result<HashMap<TableId, (DatabaseId, String)>, AppError> {
                Ok(self.table_entries_impl(ids))
            }

            async fn grant_for(
                &self,
                principal: PrincipalId,
                scope: Scope,
            ) -> Result<Grant, AppError> {
                Ok(self.grant_for_impl(principal, scope))
            }

            async fn acl_entries(&self, principal: PrincipalId) -> Result<Vec<AclEntry>, AppError> {
                Ok(self.acl_entries_impl(principal))
            }

            async fn acl_databases(
                &self,
                principal: PrincipalId,
            ) -> Result<BTreeSet<DatabaseId>, AppError> {
                Ok(self.acl_databases_impl(principal))
            }

            async fn acl_tables_in(
                &self,
                principal: PrincipalId,
                db_id: DatabaseId,
            ) -> Result<BTreeSet<TableId>, AppError> {
                Ok(self.acl_tables_in_impl(principal, db_id))
            }
        }
    };
}

impl_catalog_repository!(State);
impl_catalog_repository!(Arc<State>);

impl ReadRepository for Arc<State> {
    async fn database_info(
        &self,
        name: &str,
    ) -> Result<Option<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        Ok(self.database_info_impl(name))
    }

    async fn database_list(&self) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        Ok(self.database_list_impl())
    }

    async fn databases_by_id(
        &self,
        ids: &[DatabaseId],
    ) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        Ok(self.databases_by_id_impl(ids))
    }

    async fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        self.table_info_impl(db_name, table_name)
    }

    async fn resolve_tables(
        &self,
        refs: &[(String, String)],
    ) -> Result<Vec<ResolvedTable>, AppError> {
        Ok(self.resolve_tables_impl(refs))
    }

    async fn table_list(&self, db_name: &str) -> Result<Vec<Table>, AppError> {
        self.table_list_impl(db_name)
    }

    async fn table_list_by_id(&self, db_id: DatabaseId) -> Result<Vec<Table>, AppError> {
        Ok(self.table_list_by_id_impl(db_id))
    }

    async fn tables_by_id(
        &self,
        db_id: DatabaseId,
        ids: &[TableId],
    ) -> Result<Vec<Table>, AppError> {
        Ok(self.tables_by_id_impl(db_id, ids))
    }

    async fn table_count(&self, table_id: TableId) -> Result<u64, AppError> {
        Ok(self.table_count_impl(table_id))
    }

    async fn data_get(
        &self,
        table_id: TableId,
        ids: SpatialIdSet,
    ) -> Result<ValueGroups, AppError> {
        Ok(self.data_get_impl(table_id, ids))
    }

    async fn data_filter_eq(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        value: &[u8],
    ) -> Result<Vec<FlexId>, AppError> {
        Ok(self.data_filter_eq_impl(table_id, data_type, value))
    }

    async fn data_filter_range(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        lo: &[u8],
        hi: &[u8],
    ) -> Result<Vec<FlexId>, AppError> {
        Ok(self.data_filter_range_impl(table_id, data_type, lo, hi))
    }

    async fn list_users(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, UserRecord)>, AppError> {
        Ok(self.list_users_impl(after, limit))
    }
}

impl WriteRepository for State {
    async fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError> {
        Ok(self.database_info_impl(name).map(|(_, info)| info))
    }

    async fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        self.table_info_impl(db_name, table_name)
    }

    async fn database_create(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseInfoResponse, AppError> {
        self.database_create_impl(name, description)
    }

    async fn database_remove(&mut self, name: &str) -> Result<(), AppError> {
        self.database_remove_impl(name)
    }

    async fn database_update(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<(), AppError> {
        self.database_update_impl(name, new_name, description)
    }

    async fn database_copy(
        &mut self,
        src_db_name: &str,
        copy_name: &str,
    ) -> Result<DatabaseInfoResponse, AppError> {
        self.database_copy_impl(src_db_name, copy_name)
    }

    async fn table_create(
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
        self.table_create_impl(
            db_name,
            table_name,
            data_type,
            max_zoom_level,
            constraints,
            description,
            value_index,
            is_temporal,
        )
    }

    async fn table_update(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<UpdateTableConstraints>>,
        description: Option<Option<String>>,
        is_temporal: Option<bool>,
    ) -> Result<Table, AppError> {
        self.table_update_impl(
            db_name,
            table_name,
            new_name,
            new_constraints,
            description,
            is_temporal,
        )
    }

    async fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError> {
        self.table_remove_impl(db_name, table_name)
    }

    async fn table_copy(
        &mut self,
        src_db_name: &str,
        src_table_name: &str,
        copy_db_name: &str,
        copy_table_name: &str,
    ) -> Result<Table, AppError> {
        self.table_copy_impl(src_db_name, src_table_name, copy_db_name, copy_table_name)
    }

    /// 値インデックスを持たないので `index` は使わない。
    async fn data_insert(
        &mut self,
        table_id: TableId,
        _index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        self.data_insert_impl(table_id, ids, data);
        Ok(())
    }

    async fn data_insert_many(
        &mut self,
        table_id: TableId,
        _index: Option<TableDataType>,
        entries: Vec<(SpatialIdSet, Vec<u8>)>,
    ) -> Result<(), AppError> {
        for (ids, value) in entries {
            self.data_insert_impl(table_id, ids, &value);
        }
        Ok(())
    }

    async fn data_upsert(
        &mut self,
        table_id: TableId,
        _index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        self.data_upsert_impl(table_id, ids, data);
        Ok(())
    }

    async fn data_remove(
        &mut self,
        table_id: TableId,
        _index: Option<TableDataType>,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        self.data_remove_impl(table_id, ids);
        Ok(())
    }

    /// 書き込みはロックの下で直列に走るので、排他は要らない。
    async fn lock_user(&mut self, _username: &str) -> Result<(), AppError> {
        Ok(())
    }

    async fn put_user_record(
        &mut self,
        username: &str,
        record: &UserRecord,
    ) -> Result<(), AppError> {
        self.put_user_record_impl(username, record);
        Ok(())
    }

    async fn remove_user_record(&mut self, username: &str) -> Result<(), AppError> {
        self.remove_user_record_impl(username);
        Ok(())
    }

    async fn acl_put(&mut self, entry: AclEntry, principal: PrincipalId) -> Result<(), AppError> {
        self.acl_put_impl(entry, principal);
        Ok(())
    }

    async fn acl_remove(
        &mut self,
        principal: PrincipalId,
        target: DataTarget,
    ) -> Result<bool, AppError> {
        Ok(self.acl_remove_impl(principal, target))
    }

    async fn acl_remove_principal(&mut self, principal: PrincipalId) -> Result<(), AppError> {
        self.acl_remove_principal_impl(principal);
        Ok(())
    }

    async fn acl_count(&self, principal: PrincipalId) -> Result<u32, AppError> {
        Ok(self.acl_count_impl(principal))
    }
}
