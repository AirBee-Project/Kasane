use crate::{
    error::AppError,
    models::database::table::{Table, TableConstraints, TableDataType, TableMetadata},
    repositories::KasaneDbWrite,
};

impl<'a> KasaneDbWrite<'a> {
    /// Tableの情報を取得する
    #[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
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


        let db = self.db.tables;
        if let Some(m) = db.get(&self.write_txn, &(db_meta.id, table_name))? {
            Ok(Some(Table {
                id: m.id,
                name: table_name.to_string(),
                data_type: m.data_type,
                max_zoom_level: m.max_zoom_level,
                constraints: m.constraints,
                description: m.description,
            }))
        } else {
            Ok(None)
        }
    }

    /// Tableを作成する
    #[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
    pub fn table_create(
        &mut self,
        db_name: &str,
        table_name: &str,
        data_type: TableDataType,
        max_zoom_level: u8,
        constraints: Option<TableConstraints>,
        description: Option<String>,
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
                                    reason: "Enum choices reached maximum limit (65535)"
                                        .to_string(),
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
                        reason: "Enum type requires 'choices' constraint".to_string(),
                    });
                }
            }
        }

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
        };

        let db = self.db.tables;
        db.put(&mut self.write_txn, &(db_meta.id, table_name), &meta)?;
        db_index.put(&mut self.write_txn, &id, &())?;

        Ok(Table {
            id,
            name: table_name.to_string(),
            data_type,
            max_zoom_level,
            constraints: actual_constraints,
            description,
        })
    }

    /// Tableの名前や制約を変更する。
    #[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
    pub fn table_update(
        &mut self,
        db_name: &str,
        table_name: &str,
        new_name: Option<&str>,
        new_constraints: Option<Option<crate::models::database::table::UpdateTableConstraints>>,
        description: Option<Option<String>>,
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
                Some(nc) => self.merge_constraints(&table.data_type, &table.constraints, nc)?,
            };

            if let Some(c) = &new_c
                && let Err(msg) = c.validate()
            {
                return Err(AppError::ConstraintViolation { reason: msg });
            }

            table.constraints = new_c;

            if validate_existing_data {
                self.validate_table_existing_data(
                    table.id,
                    table.data_type,
                    table.constraints.as_ref(),
                )?;
            }
        }

        if let Some(desc) = description {
            table.description = desc;
        }

        let meta = TableMetadata {
            id: table.id,
            data_type: table.data_type,
            max_zoom_level: table.max_zoom_level,
            constraints: table.constraints.clone(),
            description: table.description.clone(),
        };

        let db = self.db.tables;
        if changed_name {
            db.delete(&mut self.write_txn, &(db_meta.id, table_name))?;
        }
        db.put(&mut self.write_txn, &(db_meta.id, &table.name), &meta)?;

        Ok(table)
    }

    fn merge_constraints(
        &self,
        data_type: &TableDataType,
        current_constraints: &Option<TableConstraints>,
        nc: crate::models::database::table::UpdateTableConstraints,
    ) -> Result<Option<TableConstraints>, AppError> {
        match (data_type, nc) {
            (
                TableDataType::Text,
                crate::models::database::table::UpdateTableConstraints::Text {
                    min_length,
                    max_length,
                },
            ) => {
                let (mut current_min, mut current_max) = match current_constraints {
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
                Ok(Some(TableConstraints::Text {
                    min_length: current_min,
                    max_length: current_max,
                }))
            }
            (
                TableDataType::Int,
                crate::models::database::table::UpdateTableConstraints::Int { min, max },
            ) => {
                let (mut current_min, mut current_max) = match current_constraints {
                    Some(TableConstraints::Int { min, max }) => (*min, *max),
                    _ => (None, None),
                };
                if let Some(v) = min {
                    current_min = v;
                }
                if let Some(v) = max {
                    current_max = v;
                }
                Ok(Some(TableConstraints::Int {
                    min: current_min,
                    max: current_max,
                }))
            }
            (
                TableDataType::Float,
                crate::models::database::table::UpdateTableConstraints::Float { min, max },
            ) => {
                let (mut current_min, mut current_max) = match current_constraints {
                    Some(TableConstraints::Float { min, max }) => (*min, *max),
                    _ => (None, None),
                };
                if let Some(v) = min {
                    current_min = v;
                }
                if let Some(v) = max {
                    current_max = v;
                }
                Ok(Some(TableConstraints::Float {
                    min: current_min,
                    max: current_max,
                }))
            }
            (
                TableDataType::Enum,
                crate::models::database::table::UpdateTableConstraints::Enum {
                    choices,
                    add_choices,
                    remove_choices,
                },
            ) => {
                let (mut current_choices, mut mapping, mut next_id) = match current_constraints {
                    Some(TableConstraints::Enum {
                        choices,
                        mapping,
                        next_id,
                    }) => (choices.clone(), mapping.clone(), *next_id),
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
                                reason: "Enum choices reached maximum limit (65535)".to_string(),
                            });
                        }
                        if next_id == 0 {
                            next_id = 1;
                        }
                        mapping.insert(c.clone(), next_id);
                        next_id += 1;
                    }
                }
                Ok(Some(TableConstraints::Enum {
                    choices: current_choices,
                    mapping,
                    next_id,
                }))
            }
            (TableDataType::Presence, _) => Err(AppError::ConstraintViolation {
                reason: "Presence type cannot have constraints".to_string(),
            }),
            (_, _) => Err(AppError::ConstraintViolation {
                reason: "Constraint type does not match data type".to_string(),
            }),
        }
    }

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
            use crate::repositories::database::table::data::shard::ShardEntry;
            match ShardEntry::decode(v_bytes)? {
                ShardEntry::Leaf(map_bytes) => {
                    let map =
                        unsafe { kasane_logic::SpatialIdMap::<Vec<u8>>::from_bytes(&map_bytes) }
                            .map_err(|e| {
                                AppError::InternalError(format!("rkyv deserialize: {}", e))
                            })?;
                    for (_, stored_val) in map.iter() {
                        let bytes = stored_val.as_slice();
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

    /// Tableを削除する。
    #[tracing::instrument(skip_all, fields(db_name = %db_name, table_name = %table_name))]
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

        Ok(())
    }

    /// Tableをコピーする。
    #[tracing::instrument(skip_all)]
    pub fn table_copy(
        &mut self,
        src_db_name: &str,
        src_table_name: &str,
        copy_db_name: &str,
        copy_table_name: &str,
    ) -> Result<Table, AppError> {
        // 1. コピー元データベースの存在確認
        let src_db_meta = {
            let db = self.db.databases;
            db.get(&self.write_txn, src_db_name)?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: src_db_name.to_string(),
                })?
        };

        // 2. コピー元テーブルの存在確認
        let src_table_meta = {
            let db = self.db.tables;
            db.get(&self.write_txn, &(src_db_meta.id, src_table_name))?
                .ok_or_else(|| AppError::TableNotFound {
                    name: src_table_name.to_string(),
                })?
        };

        // 3. コピー先データベースの存在確認
        let copy_db_meta = {
            let db = self.db.databases;
            db.get(&self.write_txn, copy_db_name)?
                .ok_or_else(|| AppError::DatabaseNotFound {
                    name: copy_db_name.to_string(),
                })?
        };

        // 4. コピー先テーブルの重複確認
        let db_tables = self.db.tables;
        if db_tables
            .get(&self.write_txn, &(copy_db_meta.id, copy_table_name))?
            .is_some()
        {
            return Err(AppError::TableAlreadyExists {
                name: copy_table_name.to_string(),
            });
        }

        // コピー先テーブル名の妥当性検証
        crate::services::helpers::name_valid::name_valid(copy_table_name)?;

        // 5. 新しい TableId を生成
        let db_index = self.db.table_id_index;
        let mut copy_table_id = crate::models::id::TableId(uuid::Uuid::now_v7());
        loop {
            if db_index.get(&self.write_txn, &copy_table_id)?.is_none() {
                break;
            }
            copy_table_id = crate::models::id::TableId(uuid::Uuid::now_v7());
        }

        // 6. 新しい TableMetadata を構成
        let copy_table_meta = TableMetadata {
            id: copy_table_id,
            data_type: src_table_meta.data_type,
            max_zoom_level: src_table_meta.max_zoom_level,
            constraints: src_table_meta.constraints.clone(),
            description: src_table_meta.description.clone(),
        };

        // 7. 新しいテーブルメタデータと ID インデックスを書き込み
        db_tables.put(
            &mut self.write_txn,
            &(copy_db_meta.id, copy_table_name),
            &copy_table_meta,
        )?;
        db_index.put(&mut self.write_txn, &copy_table_id, &())?;

        // 8. tables_data のデータを全コピー
        let tables_data = self
            .db
            .tables_data
            .remap_types::<heed::types::Bytes, heed::types::Bytes>();
        let src_prefix = src_table_meta.id.into_bytes();

        let mut data_to_insert = Vec::new();
        for iter in tables_data.prefix_iter(&self.write_txn, src_prefix.as_slice())? {
            let (k_bytes, v_bytes) = iter?;
            if k_bytes.len() == 30 {
                let mut dest_k_bytes = k_bytes.to_vec();
                dest_k_bytes[0..16].copy_from_slice(&copy_table_id.into_bytes());
                data_to_insert.push((dest_k_bytes, v_bytes.to_vec()));
            }
        }
        for (k, v) in data_to_insert {
            tables_data.put(&mut self.write_txn, &k, &v)?;
        }

        // 9. value_index のデータを全コピー
        let value_index = self.db.value_index;
        let mut index_to_insert = Vec::new();
        for iter in value_index.prefix_iter(&self.write_txn, src_prefix.as_slice())? {
            let (k_bytes, _) = iter?;
            if k_bytes.len() >= 16 {
                let mut dest_k_bytes = k_bytes.to_vec();
                dest_k_bytes[0..16].copy_from_slice(&copy_table_id.into_bytes());
                index_to_insert.push(dest_k_bytes);
            }
        }
        for k in index_to_insert {
            value_index.put(&mut self.write_txn, &k, &())?;
        }

        Ok(Table {
            id: copy_table_id,
            name: copy_table_name.to_string(),
            data_type: src_table_meta.data_type,
            max_zoom_level: src_table_meta.max_zoom_level,
            constraints: src_table_meta.constraints,
            description: src_table_meta.description,
        })
    }
}
