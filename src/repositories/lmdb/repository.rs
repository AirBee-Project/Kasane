//! 抽象 trait への適合。trait の実装は 1 ブロックにまとめる必要があるので、操作の定義
//! （モジュール分割）と適合（ここ）を分けている。

use std::collections::{BTreeSet, HashMap};

use spatial_id::collection::SpatialIdSet;

use crate::error::AppError;
use crate::models::database::DatabaseDetail;
use crate::models::database::table::{Table, TableConstraints, TableDataType};
use crate::models::id::{DataTarget, DatabaseId, PrincipalId, TableId};
use crate::models::users::{AclEntry, Grant, Scope, UserRecord};
use crate::repositories::{CatalogRepository, ReadRepository, ValueGroups, WriteRepository};

use super::{KasaneDbRead, KasaneDbWrite, catalog, data, users};

/// `RwTxn` は `RoTxn` へ Deref するので、読み書きで同じ自由関数を呼べる。
macro_rules! impl_catalog_repository {
    ($target:ty, $txn:ident) => {
        impl CatalogRepository for $target {
            async fn database_id(&self, name: &str) -> Result<Option<DatabaseId>, AppError> {
                catalog::database_id(self.db, &self.$txn, name)
            }

            async fn table_id(
                &self,
                db_id: DatabaseId,
                table_name: &str,
            ) -> Result<Option<TableId>, AppError> {
                catalog::table_id(self.db, &self.$txn, db_id, table_name)
            }

            async fn user_record(&self, username: &str) -> Result<Option<UserRecord>, AppError> {
                self.db.user_record(&self.$txn, username)
            }

            async fn database_names(
                &self,
                ids: &[DatabaseId],
            ) -> Result<HashMap<DatabaseId, String>, AppError> {
                catalog::database_names(self.db, &self.$txn, ids)
            }

            async fn table_entries(
                &self,
                ids: &[TableId],
            ) -> Result<HashMap<TableId, (DatabaseId, String)>, AppError> {
                catalog::table_entries(self.db, &self.$txn, ids)
            }

            async fn grant_for(
                &self,
                principal: PrincipalId,
                scope: Scope,
            ) -> Result<Grant, AppError> {
                self.db.grant_for(&self.$txn, principal, scope)
            }

            async fn acl_entries(&self, principal: PrincipalId) -> Result<Vec<AclEntry>, AppError> {
                self.db.acl_entries(&self.$txn, principal)
            }

            async fn table_count(&self, table_id: TableId) -> Result<u64, AppError> {
                catalog::table_count(self.db, &self.$txn, table_id)
            }

            async fn acl_databases(
                &self,
                principal: PrincipalId,
            ) -> Result<BTreeSet<DatabaseId>, AppError> {
                self.db.acl_databases(&self.$txn, principal)
            }

            async fn acl_tables_in(
                &self,
                principal: PrincipalId,
                db_id: DatabaseId,
            ) -> Result<BTreeSet<TableId>, AppError> {
                self.db.acl_tables_in(&self.$txn, principal, db_id)
            }
        }
    };
}

impl_catalog_repository!(KasaneDbRead<'_>, read_txn);
impl_catalog_repository!(KasaneDbWrite<'_>, write_txn);

impl ReadRepository for KasaneDbRead<'_> {
    async fn database_info(
        &self,
        name: &str,
    ) -> Result<Option<(DatabaseId, DatabaseDetail)>, AppError> {
        catalog::database_info(self, name)
    }

    async fn database_list(&self) -> Result<Vec<(DatabaseId, DatabaseDetail)>, AppError> {
        catalog::database_list(self)
    }

    async fn databases_by_id(
        &self,
        ids: &[DatabaseId],
    ) -> Result<Vec<(DatabaseId, DatabaseDetail)>, AppError> {
        catalog::databases_by_id(self, ids)
    }

    async fn tables_by_id(
        &self,
        db_id: DatabaseId,
        ids: &[TableId],
    ) -> Result<Vec<Table>, AppError> {
        catalog::tables_by_id(self, db_id, ids)
    }

    async fn resolve_tables(
        &self,
        refs: &[(String, String)],
    ) -> Result<Vec<crate::repositories::ResolvedTable>, AppError> {
        catalog::resolve_tables(self, refs)
    }

    async fn table_list_by_id(&self, db_id: DatabaseId) -> Result<Vec<Table>, AppError> {
        catalog::table_list_by_id(self, db_id)
    }

    async fn data_get(
        &self,
        table_id: TableId,
        ids: SpatialIdSet,
    ) -> Result<ValueGroups, AppError> {
        data::data_get(self, table_id, ids)
    }

    async fn list_users(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, UserRecord)>, AppError> {
        users::list_users(self, after, limit)
    }
}

impl WriteRepository for KasaneDbWrite<'_> {
    async fn database_create(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseDetail, AppError> {
        catalog::database_create(self, name, description)
    }

    async fn database_remove(&mut self, name: &str) -> Result<(), AppError> {
        catalog::database_remove(self, name)
    }

    async fn database_update(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<DatabaseDetail, AppError> {
        catalog::database_update(self, name, new_name, description)
    }

    async fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
        description: Option<String>,
        is_temporal: bool,
    ) -> Result<Table, AppError> {
        catalog::table_create(
            self,
            db_name,
            table_name,
            data_type,
            max_zoom_level,
            constraints,
            description,
            is_temporal,
        )
    }

    async fn table_update(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<TableConstraints>>,
        description: Option<Option<String>>,
        is_temporal: Option<bool>,
    ) -> Result<Table, AppError> {
        catalog::table_update(
            self,
            db_name,
            table_name,
            new_name,
            new_constraints,
            description,
            is_temporal,
        )
    }

    async fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError> {
        catalog::table_remove(self, db_name, table_name)
    }

    async fn data_insert(
        &mut self,
        table_id: TableId,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        data::data_insert(self, table_id, ids, data)
    }

    async fn data_upsert(
        &mut self,
        table_id: TableId,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        data::data_upsert(self, table_id, ids, data)
    }

    async fn data_remove(&mut self, table_id: TableId, ids: SpatialIdSet) -> Result<(), AppError> {
        data::data_remove(self, table_id, ids)
    }

    async fn put_user_record(
        &mut self,
        username: &str,
        record: &UserRecord,
    ) -> Result<(), AppError> {
        users::put_user_record(self, username, record)
    }

    async fn remove_user_record(&mut self, username: &str) -> Result<(), AppError> {
        users::remove_user_record(self, username)
    }

    async fn acl_put(&mut self, entry: AclEntry, principal: PrincipalId) -> Result<(), AppError> {
        users::acl_put(self, entry, principal)
    }

    async fn acl_remove(
        &mut self,
        principal: PrincipalId,
        target: DataTarget,
    ) -> Result<bool, AppError> {
        users::acl_remove(self, principal, target)
    }

    async fn acl_remove_principal(&mut self, principal: PrincipalId) -> Result<(), AppError> {
        users::acl_remove_principal(self, principal)
    }

    async fn acl_count(&self, principal: PrincipalId) -> Result<u32, AppError> {
        self.db.acl_count(&self.write_txn, principal)
    }
}
