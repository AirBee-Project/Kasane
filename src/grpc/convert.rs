//! Kasaneの型と生成された protobuf型との相互変換

use tonic::Status;

use super::pb;
use crate::error::AppError;
use crate::models::database::table::{
    TableConstraints as DomainTableConstraints, TableDataType as DomainTableDataType, TableInfo,
};

/// proto の `enumeration` フィールド（生の `i32`）を、範囲外の値なら `unspecified` にフォールバックした上で対応する enum 値へ変換する。
pub fn enum_from_i32<T: TryFrom<i32> + Copy>(value: i32, unspecified: T) -> T {
    T::try_from(value).unwrap_or(unspecified)
}

/// フィールドが未設定なら `InvalidArgument` として返す。
pub fn required<T>(value: Option<T>, field: &str) -> Result<T, Status> {
    value.ok_or_else(|| {
        AppError::InvalidArgument {
            reason: format!("{field} must be set"),
        }
        .into()
    })
}

/// `u32` を `u8` へ、範囲に収まらなければ `InvalidArgument` として返す。
pub fn u8_from_u32(value: u32, field: &str) -> Result<u8, Status> {
    u8::try_from(value).map_err(|_| {
        AppError::InvalidArgument {
            reason: format!("{field} must be between 0 and 255, got {value}"),
        }
        .into()
    })
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
            pb::TableDataType::Unspecified => Err(AppError::InvalidArgument {
                reason: "data_type must be specified".to_string(),
            }
            .into()),
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
        let kind = required(constraints.kind, "constraints.kind")?;
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

impl TryFrom<pb::update_table_request::ConstraintsUpdate> for Option<DomainTableConstraints> {
    type Error = Status;

    fn try_from(update: pb::update_table_request::ConstraintsUpdate) -> Result<Self, Self::Error> {
        match update {
            pb::update_table_request::ConstraintsUpdate::ClearConstraints(_) => Ok(None),
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

impl From<TableInfo> for pb::TableInfo {
    fn from(TableInfo { table, count }: TableInfo) -> Self {
        Self {
            name: table.name,
            data_type: table.data_type.into(),
            max_zoom_level: table.max_zoom_level as u32,
            count,
            constraints: table.constraints.map(Into::into),
            description: table.description,
            is_temporal: table.is_temporal,
            value_index: table.value_index,
        }
    }
}
