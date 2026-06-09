use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseMetadata {
    pub id: Uuid,
}

impl redb::Value for DatabaseMetadata {
    type SelfType<'a> = DatabaseMetadata;
    type AsBytes<'a> = [u8; 16];

    fn fixed_width() -> Option<usize> {
        Some(16)
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let id = Uuid::from_slice(&data[0..16]).expect("invalid uuid bytes");
        DatabaseMetadata { id }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        *value.id.as_bytes()
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new("Kasane::DatabaseMetadata")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DatabaseInfoResponse {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreateDatabaseRequest {
    pub name: String,
}

pub mod table;
