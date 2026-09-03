use serde::{Deserialize, Serialize};
use std::fmt;

use super::database::table::TableDataType;

/// Kasane が扱うプリミティブな値リテラル。
///
/// テーブルの値、クエリの動的リテラル、検索結果の辞書などで使われる。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ValueLiteral {
    Null,
    Bool(bool),
    Int(i64),
    String(String),
}

/// [`ValueLiteral`] の値種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValueType {
    Null,
    Bool,
    Int,
    String,
}

impl ValueLiteral {
    pub fn value_type(&self) -> ValueType {
        match self {
            Self::Null => ValueType::Null,
            Self::Bool(_) => ValueType::Bool,
            Self::Int(_) => ValueType::Int,
            Self::String(_) => ValueType::String,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

impl fmt::Display for ValueLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(i) => write!(f, "{i}"),
            Self::String(s) => write!(f, "{s}"),
        }
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "Null"),
            Self::Bool => write!(f, "Bool"),
            Self::Int => write!(f, "Int"),
            Self::String => write!(f, "String"),
        }
    }
}

impl From<i64> for ValueLiteral {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<String> for ValueLiteral {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for ValueLiteral {
    fn from(v: &str) -> Self {
        Self::String(v.to_string())
    }
}

impl From<bool> for ValueLiteral {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<()> for ValueLiteral {
    fn from(_: ()) -> Self {
        Self::Null
    }
}

impl From<TableDataType> for ValueType {
    fn from(value: TableDataType) -> Self {
        match value {
            TableDataType::Text | TableDataType::Enum => Self::String,
            TableDataType::Int => Self::Int,
            TableDataType::Boolean => Self::Bool,
            TableDataType::Presence => Self::Null,
        }
    }
}
