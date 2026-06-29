use crate::database::table::common::TestApp;
use crate::database::table::data::common::put_data;
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn test_table_data_get_stream() {
    let test_app = TestApp::new();

    let table_name = "stream_table";
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", table_name, "Int", 25)
        .await;

    put_data(
        &test_app,
        table_name,
        &serde_json::json!({
            "value": 500,
            "spatial_ids": [{ "z": 20, "f": 0, "x": 10, "y": 10, "type": "singleId" }]
        }),
    )
    .await;

    let query = serde_json::json!([{ "z": 20, "f": 0, "x": 10, "y": 10, "type": "singleId" }]);
    let body = serde_json::json!({ "spatial_ids": query });

    let req = Request::builder()
        .method("POST")
        .uri(format!(
            "/databases/test_db/tables/{}/data/search/stream?format=singleId",
            table_name
        ))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(bytes.to_vec()).unwrap();

    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(
        lines.len(),
        2,
        "Should contain one dictionary event and one data event"
    );

    let dict_event: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(dict_event["type"], "dictionary");
    assert_eq!(dict_event["value"], 500);

    let data_event: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(data_event["type"], "data");
    assert_eq!(data_event["spatialIds"][0]["x"], 10);
    assert_eq!(data_event["valueRef"], dict_event["valueRef"]);
}
