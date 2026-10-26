use std::collections::BTreeSet;

use uuid::Uuid;

use super::State;
use crate::error::{AppError, Resource};
use crate::models::database::table::{Table, TableConstraints, TableDataType, TableMetadata};
use crate::models::database::{DatabaseInfoResponse, DatabaseMetadata};
use crate::models::id::{DatabaseId, TableId};
use crate::repositories::ResolvedTable;
use crate::services::helpers::value::{interpret_value, restore_value};

fn info(name: &str, meta: &DatabaseMetadata) -> DatabaseInfoResponse {
    DatabaseInfoResponse {
        name: name.to_string(),
        description: meta.description.clone(),
    }
}

impl State {
    fn next_id(&mut self) -> Uuid {
        self.last_id += 1;
        Uuid::from_u128(self.last_id)
    }

    pub(super) fn database_id(&self, name: &str) -> Option<DatabaseId> {
        self.databases.get(name).map(|meta| meta.id)
    }

    fn require_database_id(&self, name: &str) -> Result<DatabaseId, AppError> {
        self.database_id(name)
            .ok_or_else(|| Resource::Database.not_found(name))
    }

    pub(super) fn table_id(&self, db_id: DatabaseId, name: &str) -> Option<TableId> {
        self.tables
            .get(&(db_id, name.to_string()))
            .map(|meta| meta.id)
    }

    pub(super) fn database_info(&self, name: &str) -> Option<(DatabaseId, DatabaseInfoResponse)> {
        self.databases
            .get(name)
            .map(|meta| (meta.id, info(name, meta)))
    }

    pub(super) fn databases(&self) -> impl Iterator<Item = (DatabaseId, DatabaseInfoResponse)> {
        self.databases
            .iter()
            .map(|(name, meta)| (meta.id, info(name, meta)))
    }

    pub(super) fn table_entries(&self) -> impl Iterator<Item = (TableId, (DatabaseId, String))> {
        self.tables
            .iter()
            .map(|((db_id, name), meta)| (meta.id, (*db_id, name.clone())))
    }

    pub(super) fn tables_in(&self, db_id: DatabaseId) -> impl Iterator<Item = Table> {
        self.tables
            .iter()
            .filter(move |((owner, _), _)| *owner == db_id)
            .map(|((_, name), meta)| Table::from_meta(name, meta.clone()))
    }

    pub(super) fn resolve_table(&self, db_name: &str, table_name: &str) -> ResolvedTable {
        let db_id = self.database_id(db_name);
        let table = db_id.and_then(|db_id| {
            self.tables
                .get(&(db_id, table_name.to_string()))
                .map(|meta| Table::from_meta(table_name, meta.clone()))
        });
        ResolvedTable { db_id, table }
    }

    pub(super) fn database_create(
        &mut self,
        name: &str,
        description: Option<String>,
    ) -> Result<DatabaseInfoResponse, AppError> {
        if self.databases.contains_key(name) {
            return Err(Resource::Database.already_exists(name));
        }
        let meta = DatabaseMetadata {
            id: DatabaseId(self.next_id()),
            description,
        };
        let created = info(name, &meta);
        self.databases.insert(name.to_string(), meta);
        Ok(created)
    }

    pub(super) fn database_remove(&mut self, name: &str) -> Result<(), AppError> {
        let meta = self
            .databases
            .remove(name)
            .ok_or_else(|| Resource::Database.not_found(name))?;
        let removed: Vec<TableId> = self
            .tables
            .iter()
            .filter(|((db_id, _), _)| *db_id == meta.id)
            .map(|(_, table)| table.id)
            .collect();
        self.tables.retain(|(db_id, _), _| *db_id != meta.id);
        for table_id in removed {
            self.data.remove(&table_id);
        }
        Ok(())
    }

    pub(super) fn database_update(
        &mut self,
        name: &str,
        new_name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<DatabaseInfoResponse, AppError> {
        let final_name = new_name.unwrap_or_else(|| name.to_string());
        if final_name != name && self.databases.contains_key(&final_name) {
            return Err(Resource::Database.already_exists(final_name));
        }
        let mut meta = self
            .databases
            .remove(name)
            .ok_or_else(|| Resource::Database.not_found(name))?;
        if let Some(description) = description {
            meta.description = description;
        }
        let updated = info(&final_name, &meta);
        self.databases.insert(final_name, meta);
        Ok(updated)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
        description: Option<String>,
        is_temporal: bool,
    ) -> Result<Table, AppError> {
        if table_name.is_empty() {
            return Err(AppError::invalid_argument("table name cannot be empty"));
        }
        let key = (self.require_database_id(db_name)?, table_name.to_string());
        if self.tables.contains_key(&key) {
            return Err(Resource::Table.already_exists(table_name));
        }

        let constraints = TableConstraints::with_enum_ids(data_type, constraints)
            .map_err(AppError::constraint_violation)?;
        if let Some(c) = &constraints {
            c.validate().map_err(AppError::constraint_violation)?;
        }

        let meta = TableMetadata {
            id: TableId(self.next_id()),
            data_type,
            max_zoom_level,
            constraints,
            description,
            is_temporal,
        };
        self.tables.insert(key, meta.clone());
        Ok(Table::from_meta(table_name, meta))
    }

    pub(super) fn table_update(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<TableConstraints>>,
        description: Option<Option<String>>,
        is_temporal: Option<bool>,
    ) -> Result<Table, AppError> {
        let db_id = self.require_database_id(db_name)?;
        let final_name = new_name.unwrap_or(table_name);
        if final_name != table_name && self.tables.contains_key(&(db_id, final_name.to_string())) {
            return Err(Resource::Table.already_exists(final_name));
        }
        let mut meta = self
            .tables
            .get(&(db_id, table_name.to_string()))
            .cloned()
            .ok_or_else(|| Resource::Table.not_found(table_name))?;

        if let Some(new_constraints) = new_constraints {
            let constraints = match new_constraints {
                None => None,
                Some(c) => {
                    TableConstraints::replaced_with(meta.data_type, meta.constraints.as_ref(), c)
                        .map_err(AppError::constraint_violation)?
                }
            };
            if let Some(c) = &constraints {
                c.validate().map_err(AppError::constraint_violation)?;
            }
            let stored: BTreeSet<&[u8]> = self
                .data
                .get(&meta.id)
                .into_iter()
                .flat_map(|map| map.iter().map(|(_, value)| value.as_slice()))
                .collect();
            for bytes in stored {
                let restored = restore_value(meta.data_type, constraints.as_ref(), bytes)?;
                interpret_value(meta.data_type, constraints.as_ref(), restored)?;
            }
            meta.constraints = constraints;
        }
        if let Some(description) = description {
            meta.description = description;
        }
        if let Some(is_temporal) = is_temporal {
            meta.is_temporal = is_temporal;
        }

        self.tables.remove(&(db_id, table_name.to_string()));
        self.tables
            .insert((db_id, final_name.to_string()), meta.clone());
        Ok(Table::from_meta(final_name, meta))
    }

    pub(super) fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError> {
        let db_id = self.require_database_id(db_name)?;
        let meta = self
            .tables
            .remove(&(db_id, table_name.to_string()))
            .ok_or_else(|| Resource::Table.not_found(table_name))?;
        self.data.remove(&meta.id);
        Ok(())
    }
}
