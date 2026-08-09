use kasane::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() {
    let openapi_json = serde_json::to_string_pretty(&ApiDoc::openapi()).unwrap();
    std::fs::write("openapi.json", openapi_json).unwrap();
}
