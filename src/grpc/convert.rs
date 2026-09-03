//! ドメインモデルと生成された protobuf 型との相互変換。

use tonic::Status;

use super::pb;
use crate::models::database::table::{
    Table, TableConstraints as DomainTableConstraints, TableDataType as DomainTableDataType,
    TableInfoResponse, TableSummary, UpdateTableConstraints as DomainUpdateTableConstraints,
};

pub fn table_data_type_to_domain(value: i32) -> Result<DomainTableDataType, Status> {
    match pb::TableDataType::try_from(value).unwrap_or(pb::TableDataType::Unspecified) {
        pb::TableDataType::Text => Ok(DomainTableDataType::Text),
        pb::TableDataType::Int => Ok(DomainTableDataType::Int),
        pb::TableDataType::Boolean => Ok(DomainTableDataType::Boolean),
        pb::TableDataType::Enum => Ok(DomainTableDataType::Enum),
        pb::TableDataType::Presence => Ok(DomainTableDataType::Presence),
        pb::TableDataType::Unspecified => {
            Err(Status::invalid_argument("data_type must be specified"))
        }
    }
}

pub fn table_data_type_to_pb(value: DomainTableDataType) -> i32 {
    (match value {
        DomainTableDataType::Text => pb::TableDataType::Text,
        DomainTableDataType::Int => pb::TableDataType::Int,
        DomainTableDataType::Boolean => pb::TableDataType::Boolean,
        DomainTableDataType::Enum => pb::TableDataType::Enum,
        DomainTableDataType::Presence => pb::TableDataType::Presence,
    }) as i32
}

pub fn table_constraints_to_domain(
    constraints: Option<pb::TableConstraints>,
) -> Result<Option<DomainTableConstraints>, Status> {
    let Some(constraints) = constraints else {
        return Ok(None);
    };
    let kind = constraints
        .kind
        .ok_or_else(|| Status::invalid_argument("constraints.kind must be set"))?;
    Ok(Some(match kind {
        pb::table_constraints::Kind::Text(t) => DomainTableConstraints::Text {
            min_length: t.min_length.map(|v| v as usize),
            max_length: t.max_length.map(|v| v as usize),
        },
        pb::table_constraints::Kind::Int(i) => DomainTableConstraints::Int {
            min: i.min,
            max: i.max,
        },
        // `mapping`/`next_id` はサーバー内部でのみ割り当てる（クライアントからは受け取らない）。
        pb::table_constraints::Kind::EnumConstraint(e) => DomainTableConstraints::Enum {
            choices: e.choices,
            mapping: Default::default(),
            next_id: 0,
        },
    }))
}

pub fn table_constraints_to_pb(
    constraints: Option<DomainTableConstraints>,
) -> Option<pb::TableConstraints> {
    constraints.map(|c| {
        let kind = match c {
            DomainTableConstraints::Text {
                min_length,
                max_length,
            } => pb::table_constraints::Kind::Text(pb::table_constraints::Text {
                min_length: min_length.map(|v| v as u64),
                max_length: max_length.map(|v| v as u64),
            }),
            DomainTableConstraints::Int { min, max } => {
                pb::table_constraints::Kind::Int(pb::table_constraints::Int { min, max })
            }
            DomainTableConstraints::Enum { choices, .. } => {
                pb::table_constraints::Kind::EnumConstraint(pb::table_constraints::Enum { choices })
            }
        };
        pb::TableConstraints { kind: Some(kind) }
    })
}

fn tri_u64(update: Option<pb::Uint64Update>) -> Option<Option<u64>> {
    update.map(|u| (!u.clear).then_some(u.value))
}

fn tri_i64(update: Option<pb::Int64Update>) -> Option<Option<i64>> {
    update.map(|u| (!u.clear).then_some(u.value))
}

pub fn tri_string(update: Option<pb::StringUpdate>) -> Option<Option<String>> {
    update.map(|u| (!u.clear).then_some(u.value))
}

fn touched_list(update: Option<pb::StringListUpdate>) -> Option<Vec<String>> {
    update.map(|w| w.values)
}

pub fn update_table_constraints_to_domain(
    update: pb::UpdateTableConstraints,
) -> Result<DomainUpdateTableConstraints, Status> {
    let kind = update
        .kind
        .ok_or_else(|| Status::invalid_argument("constraints.value.kind must be set"))?;
    Ok(match kind {
        pb::update_table_constraints::Kind::Text(t) => DomainUpdateTableConstraints::Text {
            min_length: tri_u64(t.min_length).map(|v| v.map(|v| v as usize)),
            max_length: tri_u64(t.max_length).map(|v| v.map(|v| v as usize)),
        },
        pb::update_table_constraints::Kind::Int(i) => DomainUpdateTableConstraints::Int {
            min: tri_i64(i.min),
            max: tri_i64(i.max),
        },
        pb::update_table_constraints::Kind::EnumUpdate(e) => DomainUpdateTableConstraints::Enum {
            choices: touched_list(e.choices),
            add_choices: touched_list(e.add_choices),
            remove_choices: touched_list(e.remove_choices),
        },
    })
}

/// `optional UpdateTableConstraintsUpdate` を `Option<Option<UpdateTableConstraints>>` へ。
/// 未設定 = 触らない、`clear` = 削除、それ以外は `value` を適用する。
pub fn update_table_constraints_update_to_domain(
    update: Option<pb::UpdateTableConstraintsUpdate>,
) -> Result<Option<Option<DomainUpdateTableConstraints>>, Status> {
    let Some(update) = update else {
        return Ok(None);
    };
    if update.clear {
        return Ok(Some(None));
    }
    let value = update.value.ok_or_else(|| {
        Status::invalid_argument("constraints.value must be set when clear=false")
    })?;
    Ok(Some(Some(update_table_constraints_to_domain(value)?)))
}

pub fn table_summary_to_pb(table: TableSummary) -> pb::TableSummary {
    pb::TableSummary {
        name: table.name,
        data_type: table_data_type_to_pb(table.data_type),
        max_zoom_level: table.max_zoom_level as u32,
        constraints: table_constraints_to_pb(table.constraints),
        description: table.description,
        is_temporal: table.is_temporal,
    }
}

pub fn table_domain_to_summary_pb(table: Table) -> pb::TableSummary {
    pb::TableSummary {
        name: table.name,
        data_type: table_data_type_to_pb(table.data_type),
        max_zoom_level: table.max_zoom_level as u32,
        constraints: table_constraints_to_pb(table.constraints),
        description: table.description,
        is_temporal: table.is_temporal,
    }
}

pub fn table_info_to_pb(table: TableInfoResponse) -> pb::TableInfo {
    pb::TableInfo {
        name: table.name,
        data_type: table_data_type_to_pb(table.data_type),
        max_zoom_level: table.max_zoom_level as u32,
        count: table.count,
        constraints: table_constraints_to_pb(table.constraints),
        description: table.description,
        is_temporal: table.is_temporal,
    }
}
