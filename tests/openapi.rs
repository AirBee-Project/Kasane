#[test]
/// OpenAPI仕様書（JSONおよびYAML）の生成が正常に行えるかを検証する。
fn generate_openapi_spec() {
    use kasane::openapi::ApiDoc;
    use utoipa::OpenApi;

    let openapi_json = serde_json::to_string_pretty(&ApiDoc::openapi()).unwrap();
    let openapi_yaml = serde_yaml::to_string(&ApiDoc::openapi()).unwrap();

    std::fs::write("openapi.json", openapi_json).expect("Failed to write openapi.json");
    std::fs::write("openapi.yaml", openapi_yaml).expect("Failed to write openapi.yaml");

    println!("OpenAPI spec generated successfully!");
}
