//! 抽象 trait への適合。委譲だけを行う。
//!
//! trait の実装は 1 ブロックにまとめる必要があるので、操作の定義（モジュール分割）と
//! 適合（ここ）を分けている（LMDB 実装も同じ構成）。

use kasane_logic::{FlexId, SpatialIdSet};

use crate::error::AppError;
use crate::models::database::DatabaseInfoResponse;
use crate::models::database::table::{
    Table, TableConstraints, TableDataType, UpdateTableConstraints,
};
use std::collections::{BTreeSet, HashMap};

use crate::models::id::{DataTarget, DatabaseId, PrincipalId, TableId};
use crate::models::users::{AclEntry, Grant, Scope, UserRecord};
use crate::repositories::{CatalogRepository, ReadRepository, ValueGroups, WriteRepository};

use super::kv::Reader;
use super::{TikvRead, TikvWrite, catalog, users};

/// 点参照は読み取り側と書き込み側で同一なので、impl ヘッダだけを差し替えて本体を共有する。
macro_rules! impl_catalog_repository {
    ($($header:tt)*) => {
        $($header)* {
            async fn database_id(&self, name: &str) -> Result<Option<DatabaseId>, AppError> {
                catalog::database_id(&self.txn, name).await
            }

            async fn table_id(
                &self,
                db_id: DatabaseId,
                table_name: &str,
            ) -> Result<Option<TableId>, AppError> {
                catalog::table_id(&self.txn, db_id, table_name).await
            }

            async fn user_record(&self, username: &str) -> Result<Option<UserRecord>, AppError> {
                users::user_record(&self.txn, username).await
            }

            async fn database_names(
                &self,
                ids: &[DatabaseId],
            ) -> Result<HashMap<DatabaseId, String>, AppError> {
                catalog::database_names(&self.txn, ids).await
            }

            async fn table_entries(
                &self,
                ids: &[TableId],
            ) -> Result<HashMap<TableId, (DatabaseId, String)>, AppError> {
                catalog::table_entries(&self.txn, ids).await
            }

            async fn grant_for(
                &self,
                principal: PrincipalId,
                scope: Scope,
            ) -> Result<Grant, AppError> {
                users::grant_for(&self.txn, principal, scope).await
            }

            async fn acl_entries(
                &self,
                principal: PrincipalId,
            ) -> Result<Vec<AclEntry>, AppError> {
                users::acl_entries(&self.txn, principal).await
            }

            async fn acl_databases(
                &self,
                principal: PrincipalId,
            ) -> Result<BTreeSet<DatabaseId>, AppError> {
                users::acl_databases(&self.txn, principal).await
            }

            async fn acl_tables_in(
                &self,
                principal: PrincipalId,
                db_id: DatabaseId,
            ) -> Result<BTreeSet<TableId>, AppError> {
                users::acl_tables_in(&self.txn, principal, db_id).await
            }
        }
    };
}

impl_catalog_repository!(impl<R: Reader> CatalogRepository for TikvRead<'_, R>);
impl_catalog_repository!(impl CatalogRepository for TikvWrite<'_>);

impl<R: Reader> ReadRepository for TikvRead<'_, R> {
    async fn database_info(
        &self,
        name: &str,
    ) -> Result<Option<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        catalog::database_info(&self.txn, name).await
    }

    async fn database_list(&self) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        self.database_list_impl().await
    }

    async fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        catalog::table_info(&self.txn, db_name, table_name).await
    }

    async fn resolve_tables(
        &self,
        refs: &[(String, String)],
    ) -> Result<Vec<crate::repositories::ResolvedTable>, AppError> {
        catalog::resolve_tables(&self.txn, refs).await
    }

    async fn table_list(&self, db_name: &str) -> Result<Vec<Table>, AppError> {
        self.table_list_impl(db_name).await
    }

    async fn table_list_by_id(&self, db_id: DatabaseId) -> Result<Vec<Table>, AppError> {
        catalog::table_list_by_id(&self.txn, db_id).await
    }

    async fn databases_by_id(
        &self,
        ids: &[DatabaseId],
    ) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        catalog::databases_by_id(&self.txn, ids).await
    }

    async fn tables_by_id(
        &self,
        db_id: DatabaseId,
        ids: &[TableId],
    ) -> Result<Vec<Table>, AppError> {
        catalog::tables_by_id(&self.txn, db_id, ids).await
    }

    async fn table_count(&self, table_id: TableId) -> Result<u64, AppError> {
        self.table_count_impl(table_id).await
    }

    async fn data_get(
        &self,
        table_id: TableId,
        ids: SpatialIdSet,
        limit: Option<usize>,
    ) -> Result<ValueGroups, AppError> {
        self.data_get_impl(table_id, ids, limit).await
    }

    async fn data_filter_eq(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        value: &[u8],
    ) -> Result<Vec<FlexId>, AppError> {
        self.data_filter_eq_impl(table_id, data_type, value).await
    }

    async fn data_filter_range(
        &self,
        table_id: TableId,
        data_type: TableDataType,
        lo: &[u8],
        hi: &[u8],
    ) -> Result<Vec<FlexId>, AppError> {
        self.data_filter_range_impl(table_id, data_type, lo, hi)
            .await
    }

    async fn list_users(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, UserRecord)>, AppError> {
        self.list_users_impl(after, limit).await
    }
}

impl WriteRepository for TikvWrite<'_> {
    async fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError> {
        Ok(catalog::database_info(&self.txn, name)
            .await?
            .map(|(_, info)| info))
    }

    async fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        catalog::table_info(&self.txn, db_name, table_name).await
    }

    async fn database_create(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseInfoResponse, AppError> {
        self.database_create_impl(name, description).await
    }

    async fn database_remove(&mut self, name: &str) -> Result<(), AppError> {
        self.database_remove_impl(name).await
    }

    async fn database_update(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<(), AppError> {
        self.database_update_impl(name, new_name, description).await
    }

    async fn database_copy(
        &mut self,
        src_db_name: &str,
        copy_name: &str,
    ) -> Result<DatabaseInfoResponse, AppError> {
        self.database_copy_impl(src_db_name, copy_name).await
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
            .await
    }

    async fn table_update(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<UpdateTableConstraints>>,
        description: Option<Option<String>>,
    ) -> Result<Table, AppError> {
        self.table_update_impl(db_name, table_name, new_name, new_constraints, description)
            .await
    }

    async fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError> {
        self.table_remove_impl(db_name, table_name).await
    }

    async fn table_copy(
        &mut self,
        src_db_name: &str,
        src_table_name: &str,
        copy_db_name: &str,
        copy_table_name: &str,
    ) -> Result<Table, AppError> {
        self.table_copy_impl(src_db_name, src_table_name, copy_db_name, copy_table_name)
            .await
    }

    async fn data_insert(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        self.data_insert_impl(table_id, index, ids, data).await
    }

    async fn data_insert_many(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        entries: Vec<(SpatialIdSet, Vec<u8>)>,
    ) -> Result<(), AppError> {
        self.data_insert_many_impl(table_id, index, entries).await
    }

    async fn data_upsert(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        self.data_upsert_impl(table_id, index, ids, data).await
    }

    async fn data_remove(
        &mut self,
        table_id: TableId,
        index: Option<TableDataType>,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        self.data_remove_impl(table_id, index, ids).await
    }

    async fn lock_user(&mut self, username: &str) -> Result<(), AppError> {
        self.lock_user_impl(username)
    }

    async fn put_user_record(
        &mut self,
        username: &str,
        record: &UserRecord,
    ) -> Result<(), AppError> {
        self.put_user_record_impl(username, record).await
    }

    async fn remove_user_record(&mut self, username: &str) -> Result<(), AppError> {
        self.remove_user_record_impl(username).await
    }

    async fn acl_put(&mut self, entry: AclEntry, principal: PrincipalId) -> Result<(), AppError> {
        self.acl_put_impl(entry, principal).await
    }

    async fn acl_remove(
        &mut self,
        principal: PrincipalId,
        target: DataTarget,
    ) -> Result<bool, AppError> {
        self.acl_remove_impl(principal, target).await
    }

    async fn acl_remove_principal(&mut self, principal: PrincipalId) -> Result<(), AppError> {
        self.acl_remove_principal_impl(principal).await
    }

    async fn acl_count(&self, principal: PrincipalId) -> Result<u32, AppError> {
        users::acl_count(&self.txn, principal).await
    }
}
