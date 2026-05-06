use kasane_logic::SingleId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GetValueResponse {
    pub ids: Vec<(SingleId, serde_json::Value)>,
}
