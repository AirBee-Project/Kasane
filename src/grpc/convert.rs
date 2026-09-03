//! ドメインモデルと生成された protobuf 型との相互変換。

use tonic::Status;

use super::pb;
use crate::models::database::table::{
    Table, TableConstraints as DomainTableConstraints, TableDataType as DomainTableDataType,
    TableInfoResponse, TableSummary, UpdateTableConstraints as DomainUpdateTableConstraints,
};

/// proto の `enumeration` フィールド（生の `i32`）を、範囲外の値なら `unspecified` に
/// フォールバックした上で対応する enum 値へ変換する。`Unspecified` バリアントを持つ
/// 全ての proto enum で共通のパターン。
pub fn enum_from_i32<T: TryFrom<i32> + Copy>(value: i32, unspecified: T) -> T {
    T::try_from(value).unwrap_or(unspecified)
}

impl TryFrom<pb::TableDataType> for DomainTableDataType {
    type Error = Status;

    fn try_from(value: pb::TableDataType) -> Result<Self, Self::Error> {
        match value {
            pb::TableDataType::Text => Ok(Self::Text),
            pb::TableDataType::Int => Ok(Self::Int),
            pb::TableDataType::Boolean => Ok(Self::Boolean),
            pb::TableDataType::Enum => Ok(Self::Enum),
            pb::TableDataType::Presence => Ok(Self::Presence),
            pb::TableDataType::Unspecified => {
                Err(Status::invalid_argument("data_type must be specified"))
            }
        }
    }
}

pub fn table_data_type_to_domain(value: i32) -> Result<DomainTableDataType, Status> {
    enum_from_i32(value, pb::TableDataType::Unspecified).try_into()
}

impl From<DomainTableDataType> for pb::TableDataType {
    fn from(value: DomainTableDataType) -> Self {
        match value {
            DomainTableDataType::Text => Self::Text,
            DomainTableDataType::Int => Self::Int,
            DomainTableDataType::Boolean => Self::Boolean,
            DomainTableDataType::Enum => Self::Enum,
            DomainTableDataType::Presence => Self::Presence,
        }
    }
}

pub fn table_data_type_to_pb(value: DomainTableDataType) -> i32 {
    let pb_type: pb::TableDataType = value.into();
    pb_type as i32
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

pub fn update_table_constraints_to_domain(
    update: pb::UpdateTableConstraints,
) -> Result<DomainUpdateTableConstraints, Status> {
    let kind = update
        .kind
        .ok_or_else(|| Status::invalid_argument("constraints.value.kind must be set"))?;
    Ok(match kind {
        pb::update_table_constraints::Kind::Text(t) => {
            use pb::update_table_constraints::text_update::{MaxLengthUpdate, MinLengthUpdate};
            let min_length = match t.min_length_update {
                Some(MinLengthUpdate::ClearMinLength(true)) => Some(None),
                Some(MinLengthUpdate::SetMinLength(v)) => Some(Some(v as usize)),
                _ => None,
            };
            let max_length = match t.max_length_update {
                Some(MaxLengthUpdate::ClearMaxLength(true)) => Some(None),
                Some(MaxLengthUpdate::SetMaxLength(v)) => Some(Some(v as usize)),
                _ => None,
            };
            DomainUpdateTableConstraints::Text {
                min_length,
                max_length,
            }
        }
        pb::update_table_constraints::Kind::Int(i) => {
            use pb::update_table_constraints::int_update::{MaxUpdate, MinUpdate};
            let min = match i.min_update {
                Some(MinUpdate::ClearMin(true)) => Some(None),
                Some(MinUpdate::SetMin(v)) => Some(Some(v)),
                _ => None,
            };
            let max = match i.max_update {
                Some(MaxUpdate::ClearMax(true)) => Some(None),
                Some(MaxUpdate::SetMax(v)) => Some(Some(v)),
                _ => None,
            };
            DomainUpdateTableConstraints::Int { min, max }
        }
        pb::update_table_constraints::Kind::EnumUpdate(e) => DomainUpdateTableConstraints::Enum {
            choices: (!e.choices.is_empty()).then_some(e.choices),
            add_choices: (!e.add_choices.is_empty()).then_some(e.add_choices),
            remove_choices: (!e.remove_choices.is_empty()).then_some(e.remove_choices),
        },
    })
}

pub fn parse_table_constraints_update(
    update: Option<pb::update_table_request::ConstraintsUpdate>,
) -> Result<Option<Option<DomainUpdateTableConstraints>>, Status> {
    match update {
        None => Ok(None),
        Some(pb::update_table_request::ConstraintsUpdate::ClearConstraints(true)) => Ok(Some(None)),
        Some(pb::update_table_request::ConstraintsUpdate::ClearConstraints(false)) => Ok(None),
        Some(pb::update_table_request::ConstraintsUpdate::SetConstraints(c)) => {
            Ok(Some(Some(update_table_constraints_to_domain(c)?)))
        }
    }
}

/// `ClearDescription`/`SetDescription` の2バリアントを持つ oneof。生成コードは
/// メッセージごとに別の Rust 型になるため、[`parse_description_update`] が両方を
/// 同じ形で扱えるようこのトレイトで橋渡しする。
///
/// `ClearDescription(false)` は「クリアしない」という積極的な指定ではなく、
/// フィールド自体が無いのと同じ「触らない」を表す（`oneof` にデフォルト値を
/// 送ってしまった場合の取り扱い）。
pub trait DescriptionUpdateOneof {
    fn into_tri_state(self) -> Option<Option<String>>;
}

impl DescriptionUpdateOneof for pb::update_database_request::DescriptionUpdate {
    fn into_tri_state(self) -> Option<Option<String>> {
        match self {
            Self::ClearDescription(true) => Some(None),
            Self::ClearDescription(false) => None,
            Self::SetDescription(s) => Some(Some(s)),
        }
    }
}

impl DescriptionUpdateOneof for pb::update_table_request::DescriptionUpdate {
    fn into_tri_state(self) -> Option<Option<String>> {
        match self {
            Self::ClearDescription(true) => Some(None),
            Self::ClearDescription(false) => None,
            Self::SetDescription(s) => Some(Some(s)),
        }
    }
}

pub fn parse_description_update<T: DescriptionUpdateOneof>(
    update: Option<T>,
) -> Option<Option<String>> {
    update.and_then(T::into_tri_state)
}

impl From<TableSummary> for pb::TableSummary {
    fn from(table: TableSummary) -> Self {
        Self {
            name: table.name,
            data_type: table_data_type_to_pb(table.data_type),
            max_zoom_level: table.max_zoom_level as u32,
            constraints: table_constraints_to_pb(table.constraints),
            description: table.description,
            is_temporal: table.is_temporal,
        }
    }
}

impl From<Table> for pb::TableSummary {
    fn from(table: Table) -> Self {
        TableSummary::from(table).into()
    }
}

impl From<TableInfoResponse> for pb::TableInfo {
    fn from(table: TableInfoResponse) -> Self {
        Self {
            name: table.name,
            data_type: table_data_type_to_pb(table.data_type),
            max_zoom_level: table.max_zoom_level as u32,
            count: table.count,
            constraints: table_constraints_to_pb(table.constraints),
            description: table.description,
            is_temporal: table.is_temporal,
        }
    }
}
