mod table;
mod data;

use crate::AppState;
use axum::Router;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(table::routes())
        .merge(data::routes())
}
