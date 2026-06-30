use crate::{
    error::AppError,
    models::database::table::{Table, TableConstraints, TableDataType, TableMetadata},
    repositories::KasaneDbWrite,
};

impl<'a> KasaneDbWrite<'a> {
    /// Tableの情報を取得する
    pub fn table_info(&self, db_name: &str, table_name: &str) -> Result<Option<Table>, AppError> {
        if db_name.is_empty() {
            return Ok(None);
        }
        let db_meta = {
            let db = self.db.databases;
            if let Some(m) = db.get(&self.write_txn, db_name)? {
                m
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        if let Some(meta_data) = self.table_caches.get(&(db_meta.id, table_name.to_string())) {
            return Ok(Some(Table {
                id: meta_data.id,
                name: table_name.to_string(),
                data_type: meta_data.data_type,
                max_zoom_level: meta_data.max_zoom_level,
                constraints: meta_data.constraints.clone(),
            }));
        }

        let db = self.db.tables;
        if let Some(m) = db.get(&self.write_txn, &(db_meta.id, table_name))? {
            Ok(Some(Table {
                id: m.id,
                name: table_name.to_string(),
                data_type: m.data_type,
                max_zoom_level: m.max_zoom_level,
                constraints: m.constraints,
            }))
        } else {
            Ok(None)
        }
    }

    /// Tableを作成する
    pub fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
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
        let db_meta = {
            let db = self.db.databases;
            if let Some(m) = db.get(&self.write_txn, db_name)? {
                m
            } else {
                return Err(AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                });
            }
        };

        if self.table_info(db_name, table_name)?.is_some() {
            return Err(AppError::TableAlreadyExists {
                name: table_name.to_string(),
            });
        }

        let db_index = self.db.table_id_index;

        let mut id = crate::models::id::TableId(uuid::Uuid::now_v7());
        loop {
            if db_index.get(&self.write_txn, &id)?.is_none() {
                break;
            }
            id = crate::models::id::TableId(uuid::Uuid::now_v7());
        }

        let mut actual_constraints = constraints.clone();
        if data_type == TableDataType::Enum {
            match &mut actual_constraints {
                Some(TableConstraints::Enum {
                    choices,
                    mapping,
                    next_id,
                }) => {
                    for c in choices.iter() {
                        if !mapping.contains_key(c) {
                            if *next_id == u16::MAX {
                                return Err(AppError::ConstraintViolation {
                                    reason: "Enumの選択肢が上限 (65535) に達しました".to_string(),
                                });
                            }
                            if *next_id == 0 {
                                *next_id = 1;
                            }
                            mapping.insert(c.clone(), *next_id);
                            *next_id += 1;
                        }
                    }
                }
                _ => {
                    return Err(AppError::ConstraintViolation {
                        reason: "Enum型には制約 (choices) が必須です".to_string(),
                    });
                }
            }
        }

        if let Some(c) = &actual_constraints
            && let Err(msg) = c.validate() {
                return Err(AppError::ConstraintViolation { reason: msg });
            }

        let meta = TableMetadata {
            id,
            data_type,
            max_zoom_level,
            constraints: actual_constraints.clone(),
        };

        let db = self.db.tables;
        db.put(&mut self.write_txn, &(db_meta.id, table_name), &meta)?;
        db_index.put(&mut self.write_txn, &id, &())?;

        self.table_caches
            .insert((db_meta.id, table_name.to_string()), meta);

        Ok(Table {
            id,
            name: table_name.to_string(),
            data_type,
            max_zoom_level,
            constraints: actual_constraints,
        })
    }

    /// Tableの名前や制約を変更する。
    pub fn table_update(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<crate::models::database::table::UpdateTableConstraints>>,
        validate_existing_data: bool,
    ) -> Result<Table, AppError> {
        let db_meta = {
            let db = self.db.databases;
            db.get(&self.write_txn, db_name)?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                })?
        };

        let mut table = {
            self.table_info(db_name, table_name)?
                .ok_or_else(|| AppError::TableNotFound {
                    name: table_name.to_string(),
                })?
        };

        let changed_name = if let Some(nn) = new_name {
            if nn != table_name {
                if self.table_info(db_name, nn)?.is_some() {
                    return Err(AppError::TableAlreadyExists {
                        name: nn.to_string(),
                    });
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
                Some(nc) => match (&table.data_type, nc) {
                    (
                        TableDataType::Text,
                        crate::models::database::table::UpdateTableConstraints::Text {
                            min_length,
                            max_length,
                        },
                    ) => {
                        let (mut current_min, mut current_max) = match &table.constraints {
                            Some(TableConstraints::Text {
                                min_length,
                                max_length,
                            }) => (*min_length, *max_length),
                            _ => (None, None),
                        };
                        if let Some(v) = min_length {
                            current_min = v;
                        }
                        if let Some(v) = max_length {
                            current_max = v;
                        }
                        Some(TableConstraints::Text {
                            min_length: current_min,
                            max_length: current_max,
                        })
                    }
                    (
                        TableDataType::TinyInt,
                        crate::models::database::table::UpdateTableConstraints::TinyInt { min, max },
                    ) => {
                        let (mut current_min, mut current_max) = match &table.constraints {
                            Some(TableConstraints::TinyInt { min, max }) => (*min, *max),
                            _ => (None, None),
                        };
                        if let Some(v) = min {
                            current_min = v;
                        }
                        if let Some(v) = max {
                            current_max = v;
                        }
                        Some(TableConstraints::TinyInt {
                            min: current_min,
                            max: current_max,
                        })
                    }
                    (
                        TableDataType::SmallInt,
                        crate::models::database::table::UpdateTableConstraints::SmallInt { min, max },
                    ) => {
                        let (mut current_min, mut current_max) = match &table.constraints {
                            Some(TableConstraints::SmallInt { min, max }) => (*min, *max),
                            _ => (None, None),
                        };
                        if let Some(v) = min {
                            current_min = v;
                        }
                        if let Some(v) = max {
                            current_max = v;
                        }
                        Some(TableConstraints::SmallInt {
                            min: current_min,
                            max: current_max,
                        })
                    }
                    (
                        TableDataType::Int,
                        crate::models::database::table::UpdateTableConstraints::Int { min, max },
                    ) => {
                        let (mut current_min, mut current_max) = match &table.constraints {
                            Some(TableConstraints::Int { min, max }) => (*min, *max),
                            _ => (None, None),
                        };
                        if let Some(v) = min {
                            current_min = v;
                        }
                        if let Some(v) = max {
                            current_max = v;
                        }
                        Some(TableConstraints::Int {
                            min: current_min,
                            max: current_max,
                        })
                    }
                    (
                        TableDataType::BigInt,
                        crate::models::database::table::UpdateTableConstraints::BigInt { min, max },
                    ) => {
                        let (mut current_min, mut current_max) = match &table.constraints {
                            Some(TableConstraints::BigInt { min, max }) => (*min, *max),
                            _ => (None, None),
                        };
                        if let Some(v) = min {
                            current_min = v;
                        }
                        if let Some(v) = max {
                            current_max = v;
                        }
                        Some(TableConstraints::BigInt {
                            min: current_min,
                            max: current_max,
                        })
                    }
                    (
                        TableDataType::Float,
                        crate::models::database::table::UpdateTableConstraints::Float { min, max },
                    ) => {
                        let (mut current_min, mut current_max) = match &table.constraints {
                            Some(TableConstraints::Float { min, max }) => (*min, *max),
                            _ => (None, None),
                        };
                        if let Some(v) = min {
                            current_min = v;
                        }
                        if let Some(v) = max {
                            current_max = v;
                        }
                        Some(TableConstraints::Float {
                            min: current_min,
                            max: current_max,
                        })
                    }
                    (
                        TableDataType::Double,
                        crate::models::database::table::UpdateTableConstraints::Double { min, max },
                    ) => {
                        let (mut current_min, mut current_max) = match &table.constraints {
                            Some(TableConstraints::Double { min, max }) => (*min, *max),
                            _ => (None, None),
                        };
                        if let Some(v) = min {
                            current_min = v;
                        }
                        if let Some(v) = max {
                            current_max = v;
                        }
                        Some(TableConstraints::Double {
                            min: current_min,
                            max: current_max,
                        })
                    }
                    (
                        TableDataType::Enum,
                        crate::models::database::table::UpdateTableConstraints::Enum {
                            choices,
                            add_choices,
                            remove_choices,
                        },
                    ) => {
                        let (mut current_choices, mut mapping, mut next_id) =
                            match table.constraints.take() {
                                Some(TableConstraints::Enum {
                                    choices,
                                    mapping,
                                    next_id,
                                }) => (choices, mapping, next_id),
                                _ => (Vec::new(), std::collections::HashMap::new(), 1),
                            };
                        if let Some(new_choices) = choices {
                            current_choices = new_choices;
                        }
                        if let Some(adds) = add_choices {
                            for add in adds {
                                if !current_choices.contains(&add) {
                                    current_choices.push(add);
                                }
                            }
                        }
                        if let Some(removes) = remove_choices {
                            current_choices.retain(|c| !removes.contains(c));
                        }
                        for c in &current_choices {
                            if !mapping.contains_key(c) {
                                if next_id == u16::MAX {
                                    return Err(AppError::ConstraintViolation {
                                        reason: "Enumの選択肢が上限 (65535) に達しました".to_string(),
                                    });
                                }
                                if next_id == 0 {
                                    next_id = 1;
                                }
                                mapping.insert(c.clone(), next_id);
                                next_id += 1;
                            }
                        }
                        Some(TableConstraints::Enum {
                            choices: current_choices,
                            mapping,
                            next_id,
                        })
                    }
                    (TableDataType::Presence, _) => {
                        return Err(AppError::ConstraintViolation {
                            reason: "Presence型には制約を指定できません".to_string(),
                        });
                    }
                    (_, _) => {
                        return Err(AppError::ConstraintViolation {
                            reason: "制約の型がデータ型と一致しません".to_string(),
                        });
                    }
                },
            };

            if let Some(c) = &new_c
                && let Err(msg) = c.validate() {
                    return Err(AppError::ConstraintViolation { reason: msg });
                }

            table.constraints = new_c;

            if validate_existing_data {
                let tables_data = self
                    .db
                    .tables_data
                    .remap_types::<heed::types::Bytes, heed::types::Bytes>();
                let prefix = table.id.into_bytes();

                for iter in tables_data.prefix_iter(&self.write_txn, prefix.as_slice())? {
                    let (_, v_bytes) = iter?;
                    use crate::repositories::database::table::data::shard::ShardEntry;
                    match ShardEntry::decode(v_bytes)? {
                        ShardEntry::Leaf(map_bytes) => {
                            let map = unsafe {
                                kasane_logic::SpatialIdMap::<Vec<u8>>::from_bytes(&map_bytes)
                            }
                            .map_err(|e| {
                                AppError::InternalError(format!("rkyv deserialize: {}", e))
                            })?;
                            for (_, stored_val) in map.iter() {
                                let bytes = stored_val.as_slice();
                                let restored_json = crate::services::helpers::value::restore_value(
                                    table.data_type,
                                    table.constraints.as_ref(),
                                    bytes,
                                )?;
                                crate::services::helpers::value::interpret_value(
                                    table.data_type,
                                    table.constraints.as_ref(),
                                    restored_json,
                                )?;
                            }
                        }
                        ShardEntry::Pointers(_) => {}
                    }
                }
            }
        }

        let meta = TableMetadata {
            id: table.id,
            data_type: table.data_type,
            max_zoom_level: table.max_zoom_level,
            constraints: table.constraints.clone(),
        };

        let db = self.db.tables;
        if changed_name {
            db.delete(&mut self.write_txn, &(db_meta.id, table_name))?;
            self.table_caches
                .remove(&(db_meta.id, table_name.to_string()));
        }
        db.put(&mut self.write_txn, &(db_meta.id, &table.name), &meta)?;
        self.table_caches
            .insert((db_meta.id, table.name.clone()), meta);

        Ok(table)
    }

    /// Tableを削除する。
    pub fn table_remove(&mut self, db_name: &str, table_name: &str) -> Result<(), AppError> {
        let table = match self.table_info(db_name, table_name)? {
            Some(t) => t,
            None => {
                return Err(AppError::TableNotFound {
                    name: table_name.to_string(),
                });
            }
        };

        let db_meta = {
            let db = self.db.databases;
            db.get(&self.write_txn, db_name)?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: db_name.to_string(),
                })?
        };

        // 1. シャードデータを全削除（tables_data の table_id プレフィックス）。
        //    反復中に削除できないため、キーを集めてから削除する。
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

        // 1b. 値インデックスも table_id プレフィックスで全削除。
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

        // 2. テーブルメタデータと ID インデックスを削除。
        self.db
            .tables
            .delete(&mut self.write_txn, &(db_meta.id, table_name))?;
        self.db
            .table_id_index
            .delete(&mut self.write_txn, &table.id)?;

        // 3. キャッシュから除去。
        self.table_caches
            .remove(&(db_meta.id, table_name.to_string()));

        Ok(())
    }
}
