use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::{AppState, openapi::ApiDoc};

pub fn routes() -> Router<AppState> {
    let mut api_doc = ApiDoc::openapi();
    api_doc.info.version = env!("CARGO_PKG_VERSION").to_string();

    Router::new().merge(SwaggerUi::new("/swagger-ui").url("/openapi.json", api_doc))
}
