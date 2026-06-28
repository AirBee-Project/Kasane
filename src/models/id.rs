use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DatabaseId(pub Uuid);

impl fmt::Display for DatabaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for DatabaseId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(DatabaseId(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for DatabaseId {
    fn from(uuid: Uuid) -> Self {
        DatabaseId(uuid)
    }
}

impl DatabaseId {
    pub fn into_bytes(self) -> [u8; 16] {
        self.0.into_bytes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TableId(pub Uuid);

impl fmt::Display for TableId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for TableId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(TableId(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for TableId {
    fn from(uuid: Uuid) -> Self {
        TableId(uuid)
    }
}

impl TableId {
    pub fn into_bytes(self) -> [u8; 16] {
        self.0.into_bytes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId(pub Uuid);

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for UserId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(UserId(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for UserId {
    fn from(uuid: Uuid) -> Self {
        UserId(uuid)
    }
}

impl UserId {
    pub fn into_bytes(self) -> [u8; 16] {
        self.0.into_bytes()
    }
}
