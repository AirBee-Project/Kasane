use kasane_logic::SingleId;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct GetValueResponse {
    pub ids: Vec<(SingleId, serde_json::Value)>,
}
