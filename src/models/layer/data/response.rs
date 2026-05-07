use kasane_logic::SingleId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GetDataResponse {
    pub ids: Vec<(SingleId, serde_json::Value)>,
}
