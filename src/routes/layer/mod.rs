mod data;
mod layer;

use crate::AppState;
use axum::Router;

pub fn routes() -> Router<AppState> {
    Router::new().merge(layer::routes()).merge(data::routes())
}
