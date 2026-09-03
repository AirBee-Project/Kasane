//! Kasaneの型と生成された protobuf型との相互変換

use tonic::Status;

use super::pb;
use crate::models::database::table::{
    Table, TableConstraints as DomainTableConstraints, TableDataType as DomainTableDataType,
    TableInfoResponse, TableSummary, UpdateTableConstraints as DomainUpdateTableConstraints,
};

/// proto の `enumeration` フィールド（生の `i32`）を、範囲外の値なら `unspecified` にフォールバックした上で対応する enum 値へ変換する。
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

impl TryFrom<i32> for DomainTableDataType {
    type Error = Status;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        enum_from_i32(value, pb::TableDataType::Unspecified).try_into()
    }
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

impl From<DomainTableDataType> for i32 {
    fn from(value: DomainTableDataType) -> Self {
        pb::TableDataType::from(value) as i32
    }
}

impl TryFrom<pb::TableConstraints> for DomainTableConstraints {
    type Error = Status;

    fn try_from(constraints: pb::TableConstraints) -> Result<Self, Self::Error> {
        let kind = constraints
            .kind
            .ok_or_else(|| Status::invalid_argument("constraints.kind must be set"))?;
        Ok(match kind {
            pb::table_constraints::Kind::Text(t) => Self::Text {
                min_length: t.min_length.map(|v| v as usize),
                max_length: t.max_length.map(|v| v as usize),
            },
            pb::table_constraints::Kind::Int(i) => Self::Int {
                min: i.min,
                max: i.max,
            },
            pb::table_constraints::Kind::EnumConstraint(e) => Self::Enum {
                choices: e.choices,
                mapping: Default::default(),
                next_id: 0,
            },
        })
    }
}

impl From<DomainTableConstraints> for pb::TableConstraints {
    fn from(constraints: DomainTableConstraints) -> Self {
        let kind = match constraints {
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
        Self { kind: Some(kind) }
    }
}

impl TryFrom<pb::UpdateTableConstraints> for DomainUpdateTableConstraints {
    type Error = Status;

    fn try_from(update: pb::UpdateTableConstraints) -> Result<Self, Self::Error> {
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
                Self::Text {
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
                Self::Int { min, max }
            }
            pb::update_table_constraints::Kind::EnumUpdate(e) => Self::Enum {
                choices: (!e.choices.is_empty()).then_some(e.choices),
                add_choices: (!e.add_choices.is_empty()).then_some(e.add_choices),
                remove_choices: (!e.remove_choices.is_empty()).then_some(e.remove_choices),
            },
        })
    }
}

impl TryFrom<pb::update_table_request::ConstraintsUpdate> for Option<DomainUpdateTableConstraints> {
    type Error = Status;

    fn try_from(update: pb::update_table_request::ConstraintsUpdate) -> Result<Self, Self::Error> {
        match update {
            pb::update_table_request::ConstraintsUpdate::ClearConstraints(true) => Ok(None),
            pb::update_table_request::ConstraintsUpdate::ClearConstraints(false) => Ok(None),
            pb::update_table_request::ConstraintsUpdate::SetConstraints(c) => {
                Ok(Some(c.try_into()?))
            }
        }
    }
}

impl From<pb::update_database_request::DescriptionUpdate> for Option<String> {
    fn from(update: pb::update_database_request::DescriptionUpdate) -> Self {
        match update {
            pb::update_database_request::DescriptionUpdate::ClearDescription(true) => None,
            pb::update_database_request::DescriptionUpdate::ClearDescription(false) => None,
            pb::update_database_request::DescriptionUpdate::SetDescription(s) => Some(s),
        }
    }
}

impl From<pb::update_table_request::DescriptionUpdate> for Option<String> {
    fn from(update: pb::update_table_request::DescriptionUpdate) -> Self {
        match update {
            pb::update_table_request::DescriptionUpdate::ClearDescription(true) => None,
            pb::update_table_request::DescriptionUpdate::ClearDescription(false) => None,
            pb::update_table_request::DescriptionUpdate::SetDescription(s) => Some(s),
        }
    }
}

impl From<TableSummary> for pb::TableSummary {
    fn from(table: TableSummary) -> Self {
        Self {
            name: table.name,
            data_type: table.data_type.into(),
            max_zoom_level: table.max_zoom_level as u32,
            constraints: table.constraints.map(Into::into),
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
            data_type: table.data_type.into(),
            max_zoom_level: table.max_zoom_level as u32,
            count: table.count,
            constraints: table.constraints.map(Into::into),
            description: table.description,
            is_temporal: table.is_temporal,
        }
    }
}
