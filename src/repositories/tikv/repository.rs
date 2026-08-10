//! [`TikvRead`] / [`TikvWrite`] を抽象 API の trait 群へ適合させる層。
//!
//! 実処理は `catalog` / `users` / `data` の各モジュールにあり、ここはその委譲だけを行う。
//! trait の実装は 1 つのブロックにまとめる必要があるため、操作の定義（モジュール分割）と
//! trait への適合（この 1 箇所）を分けている。

use kasane_logic::{FlexId, SpatialIdSet};

use crate::error::AppError;
use crate::models::database::DatabaseInfoResponse;
use crate::models::database::table::{
    Table, TableConstraints, TableDataType, UpdateTableConstraints,
};
use crate::models::id::{DatabaseId, TableId};
use crate::models::users::{PrivilegeRule, PrivilegeTarget, User, UserMetadata};
use crate::repositories::{CatalogRepository, ReadRepository, ValueGroups, WriteRepository};

use super::kv::Reader;
use super::{TikvRead, TikvWrite, catalog, users};

/// [`CatalogRepository`] の点参照は読み取り側と書き込み側で同一なので、
/// impl ヘッダだけを差し替えて本体を共有する。
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

            async fn database_name(&self, db_id: DatabaseId) -> Result<Option<String>, AppError> {
                catalog::database_name(&self.txn, db_id).await
            }

            async fn table_name(&self, table_id: TableId) -> Result<Option<String>, AppError> {
                catalog::table_name(&self.txn, table_id).await
            }

            async fn user_meta(&self, username: &str) -> Result<Option<UserMetadata>, AppError> {
                users::user_meta(&self.txn, username).await
            }

            async fn table_names(&self, db_id: DatabaseId) -> Result<Vec<String>, AppError> {
                catalog::table_names(&self.txn, db_id).await
            }
        }
    };
}

impl_catalog_repository!(impl<R: Reader> CatalogRepository for TikvRead<'_, R>);
impl_catalog_repository!(impl CatalogRepository for TikvWrite<'_>);

impl<R: Reader> ReadRepository for TikvRead<'_, R> {
    async fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError> {
        catalog::database_info(&self.txn, name).await
    }

    async fn database_list(&self) -> Result<Vec<(DatabaseId, DatabaseInfoResponse)>, AppError> {
        self.database_list_impl().await
    }

    async fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        catalog::table_info(&self.txn, db_name, table_name).await
    }

    async fn table_list(&self, db_name: &str) -> Result<Vec<Table>, AppError> {
        self.table_list_impl(db_name).await
    }

    async fn table_list_by_id(&self, db_id: DatabaseId) -> Result<Vec<Table>, AppError> {
        catalog::table_list_by_id(&self.txn, db_id).await
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

    async fn get_user(&self, username: &str) -> Result<Option<User>, AppError> {
        self.get_user_impl(username).await
    }

    async fn require_user(&self, username: &str) -> Result<User, AppError> {
        self.require_user_impl(username).await
    }

    async fn get_all_users(&self) -> Result<Vec<User>, AppError> {
        self.get_all_users_impl().await
    }
}

impl WriteRepository for TikvWrite<'_> {
    async fn database_info(&self, name: &str) -> Result<Option<DatabaseInfoResponse>, AppError> {
        catalog::database_info(&self.txn, name).await
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
    ) -> Result<Table, AppError> {
        self.table_create_impl(
            db_name,
            table_name,
            data_type,
            max_zoom_level,
            constraints,
            description,
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
        validate_existing_data: bool,
    ) -> Result<Table, AppError> {
        self.table_update_impl(
            db_name,
            table_name,
            new_name,
            new_constraints,
            description,
            validate_existing_data,
        )
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
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        self.data_insert_impl(table_id, data_type, ids, data).await
    }

    async fn data_upsert(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        self.data_upsert_impl(table_id, data_type, ids, data).await
    }

    async fn data_remove(
        &mut self,
        table_id: TableId,
        data_type: TableDataType,
        ids: SpatialIdSet,
    ) -> Result<(), AppError> {
        self.data_remove_impl(table_id, data_type, ids).await
    }

    async fn create_user(
        &mut self,
        username: &str,
        id: uuid::Uuid,
        password_hash: String,
        privileges: &[PrivilegeRule],
    ) -> Result<(), AppError> {
        self.create_user_impl(username, id, password_hash, privileges)
            .await
    }

    async fn set_password(
        &mut self,
        username: &str,
        password_hash: String,
    ) -> Result<(), AppError> {
        self.set_password_impl(username, password_hash).await
    }

    async fn grant_privilege(
        &mut self,
        username: &str,
        rule: &PrivilegeRule,
    ) -> Result<(), AppError> {
        self.grant_privilege_impl(username, rule).await
    }

    async fn revoke_privilege(
        &mut self,
        username: &str,
        target: &PrivilegeTarget,
    ) -> Result<(), AppError> {
        self.revoke_privilege_impl(username, target).await
    }

    async fn delete_user(&mut self, username: &str) -> Result<(), AppError> {
        self.delete_user_impl(username).await
    }
}
