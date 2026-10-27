use std::collections::{BTreeSet, HashMap};

use spatial_id::collection::SpatialIdSet;

use super::{MemoryRead, MemoryWrite, users};
use crate::error::AppError;
use crate::models::database::DatabaseDetail;
use crate::models::id::{DatabaseId, PrincipalId, TableId};
use crate::models::table::{Table, TableConstraints, TableDataType};
use crate::models::users::{AclEntry, Grant, Scope, UserRecord};
use crate::repositories::{
    CatalogRepository, ReadRepository, ResolvedTable, ValueGroups, WriteRepository,
};

macro_rules! impl_catalog_repository {
    ($target:ty) => {
        impl CatalogRepository for $target {
            async fn database_id(&self, name: &str) -> Result<Option<DatabaseId>, AppError> {
                Ok(self.0.database_id(name))
            }

            async fn table_id(
                &self,
                db_id: DatabaseId,
                table_name: &str,
            ) -> Result<Option<TableId>, AppError> {
                Ok(self.0.table_id(db_id, table_name))
            }

            async fn user_record(&self, username: &str) -> Result<Option<UserRecord>, AppError> {
                Ok(users::user_record(username))
            }

            async fn database_names(
                &self,
                ids: &[DatabaseId],
            ) -> Result<HashMap<DatabaseId, String>, AppError> {
                Ok(self
                    .0
                    .databases()
                    .filter(|(id, _)| ids.contains(id))
                    .map(|(id, info)| (id, info.name))
                    .collect())
            }

            async fn table_entries(
                &self,
                ids: &[TableId],
            ) -> Result<HashMap<TableId, (DatabaseId, String)>, AppError> {
                Ok(self
                    .0
                    .table_entries()
                    .filter(|(id, _)| ids.contains(id))
                    .collect())
            }

            // root は全体の admin なので、認可は ACL を見る前に決着する。
            async fn grant_for(&self, _: PrincipalId, scope: Scope) -> Result<Grant, AppError> {
                Ok(match scope {
                    Scope::Database(_) => Grant::Database(None),
                    Scope::Table(..) => Grant::Table {
                        database: None,
                        table: None,
                    },
                    Scope::AnyIn(_) => Grant::AnyIn(false),
                })
            }

            async fn acl_entries(&self, _: PrincipalId) -> Result<Vec<AclEntry>, AppError> {
                Ok(Vec::new())
            }

            async fn table_count(&self, table_id: TableId) -> Result<u64, AppError> {
                Ok(self.0.table_count(table_id))
            }

            async fn acl_databases(
                &self,
                _: PrincipalId,
            ) -> Result<BTreeSet<DatabaseId>, AppError> {
                Ok(BTreeSet::new())
            }

            async fn acl_tables_in(
                &self,
                _: PrincipalId,
                _: DatabaseId,
            ) -> Result<BTreeSet<TableId>, AppError> {
                Ok(BTreeSet::new())
            }
        }
    };
}

impl_catalog_repository!(MemoryRead);
impl_catalog_repository!(MemoryWrite);

impl ReadRepository for MemoryRead {
    async fn database_info(
        &self,
        name: &str,
    ) -> Result<Option<(DatabaseId, DatabaseDetail)>, AppError> {
        Ok(self.0.database_info(name))
    }

    async fn database_list(&self) -> Result<Vec<(DatabaseId, DatabaseDetail)>, AppError> {
        Ok(self.0.databases().collect())
    }

    async fn databases_by_id(
        &self,
        ids: &[DatabaseId],
    ) -> Result<Vec<(DatabaseId, DatabaseDetail)>, AppError> {
        Ok(self
            .0
            .databases()
            .filter(|(id, _)| ids.contains(id))
            .collect())
    }

    async fn resolve_tables(
        &self,
        refs: &[(String, String)],
    ) -> Result<Vec<ResolvedTable>, AppError> {
        Ok(refs
            .iter()
            .map(|(db_name, table_name)| self.0.resolve_table(db_name, table_name))
            .collect())
    }

    async fn table_list_by_id(&self, db_id: DatabaseId) -> Result<Vec<Table>, AppError> {
        Ok(self.0.tables_in(db_id).collect())
    }

    async fn tables_by_id(
        &self,
        db_id: DatabaseId,
        ids: &[TableId],
    ) -> Result<Vec<Table>, AppError> {
        Ok(self
            .0
            .tables_in(db_id)
            .filter(|table| ids.contains(&table.id))
            .collect())
    }

    async fn data_get(
        &self,
        table_id: TableId,
        ids: SpatialIdSet,
    ) -> Result<ValueGroups, AppError> {
        Ok(self.0.data_get(table_id, ids))
    }

    async fn list_users(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, UserRecord)>, AppError> {
        Ok(users::list_users(after, limit))
    }
}

impl WriteRepository for MemoryWrite {
    async fn database_create(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseDetail, AppError> {
        self.0.database_create(name, description)
    }

    async fn database_remove(&mut self, name: &str) -> Result<(), AppError> {
        self.0.database_remove(name)
    }

    async fn database_update(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<DatabaseDetail, AppError> {
        self.0.database_update(name, new_name, description)
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
        self.0.table_create(
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
        self.0.table_update(
            db_name,
            table_name,
            new_name,
            new_constraints,
            description,
            is_temporal,
        )
    }

    async fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError> {
        self.0.table_remove(db_name, table_name)
    }

    async fn data_insert(
        &mut self,
        table_id: TableId,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        self.0.data_insert(table_id, ids, data);
        Ok(())
    }

    async fn data_upsert(
        &mut self,
        table_id: TableId,
        ids: SpatialIdSet,
        data: &[u8],
    ) -> Result<(), AppError> {
        self.0.data_upsert(table_id, ids, data);
        Ok(())
    }

    async fn data_remove(&mut self, table_id: TableId, ids: SpatialIdSet) -> Result<(), AppError> {
        self.0.data_remove(table_id, ids);
        Ok(())
    }
}
