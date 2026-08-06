use serde::{Deserialize, Serialize};

pub const MAX_DESCRIPTION_LENGTH: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseMetadata {
    pub id: crate::models::id::DatabaseId,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DatabaseInfoResponse {
    #[schema(example = "example_database")]
    pub name: String,
    #[schema(example = "データベースの説明文")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateDatabaseRequest {
    #[schema(example = "example_database")]
    pub name: String,
    #[schema(example = "データベースの説明文")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UpdateDatabaseRequest {
    #[schema(example = "example_database_renamed")]
    pub new_name: Option<String>,
    #[serde(default, deserialize_with = "crate::models::helpers::double_option")]
    #[schema(value_type = Option<String>, example = "更新されたデータベースの説明です")]
    pub description: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CopyDatabaseRequest {
    #[schema(example = "example_database_copy")]
    pub copy_name: String,
}

pub mod table;
