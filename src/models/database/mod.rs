use serde::{Deserialize, Serialize};

pub const MAX_DESCRIPTION_LENGTH: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseMetadata {
    pub id: crate::models::id::DatabaseId,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfoResponse {
    pub name: String,
    pub description: Option<String>,
}

pub mod table;
