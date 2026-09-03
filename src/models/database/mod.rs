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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDatabaseRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDatabaseRequest {
    pub new_name: Option<String>,
    #[serde(default, deserialize_with = "crate::models::helpers::double_option")]
    pub description: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyDatabaseRequest {
    pub copy_name: String,
}

pub mod table;
