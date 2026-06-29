use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseMetadata {
    pub id: crate::models::id::DatabaseId,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DatabaseInfoResponse {
    #[schema(example = "example_database")]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateDatabaseRequest {
    #[schema(example = "example_database")]
    pub name: String,
}

pub mod table;
