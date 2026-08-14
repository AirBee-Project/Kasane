use std::collections::HashMap;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use kasane::models::spatial_id::RawSingleId;
use serde::de::DeserializeOwned;
use tower::ServiceExt;

use crate::common::TestApp;

/// `PUT /tables/{table_name}/data` でデータを挿入し、200 OKを検証してレスポンスを返す。
///
/// # Arguments
/// - `test_app`   - テスト用アプリケーション
/// - `table_name` - 対象レイヤー名
/// - `body`       - リクエストボディ。`value` と `query` を含む JSON を渡す
///
/// # Example
/// ```
/// let query = serde_json::json!({
///     "ids": [{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }],
///     "type": "spatialIds"
/// });
///
/// put_data(
///     &test_app,
///     "my_table",
///     &serde_json::json!({ "value": 42, "spatial_ids": query }),
/// )
/// .await;
/// ```
pub async fn put_data(
    test_app: &TestApp,
    table_name: &str,
    body: &serde_json::Value,
) -> axum::response::Response {
    let req = Request::builder()
        .method("PUT")
        .uri(format!("/databases/test_db/tables/{}/data", table_name))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    if status != StatusCode::OK {
        let (_, body) = response.into_parts();
        let bytes = to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&bytes);
        panic!(
            "PUT /tables/{}/data failed.\nstatus: {}\nbody: {}",
            table_name, status, text,
        );
    }
    response
}

/// `PATCH /tables/{table_name}/data` でデータを upsert し、200 OKを検証する。
pub async fn patch_data(test_app: &TestApp, table_name: &str, body: &serde_json::Value) {
    let req = Request::builder()
        .method("PATCH")
        .uri(format!("/databases/test_db/tables/{}/data", table_name))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

/// `POST /tables/{table_name}/data/search` でデータを検索し、レスポンスボディを JSON で返す。
///
/// `query` には空間ID指定部分のみを渡せばよく、`{ "spatial_ids": ... }` へのラップはこの関数内で行う。
///
/// # Arguments
/// - `test_app`   - テスト用アプリケーション
/// - `table_name` - 対象レイヤー名
/// - `query`      - 空間IDクエリ (`spatialIds` JSON オブジェクト)
///
/// # Example
/// ```
/// let query = serde_json::json!({
///     "ids": [{ "z": 20, "f": 0, "x": 931386, "y": 412905, "type": "singleId" }],
///     "type": "spatialIds"
/// });
///
/// let result_json = search_data(&test_app, "my_table", &query).await;
/// assert_eq!(result_json["ids"][0]["data"], 42);
/// ```
pub async fn search_data(
    test_app: &TestApp,
    table_name: &str,
    query: &serde_json::Value,
) -> serde_json::Value {
    let body = serde_json::json!({ "spatial_ids": query });

    let req = Request::builder()
        .method("POST")
        .uri(format!(
            "/databases/test_db/tables/{}/data/search?format=singleId",
            table_name
        ))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body_bytes).unwrap()
}

/// `search_data` の結果JSONの先頭エントリのデータ値と空間IDを検証する。
///
/// # Arguments
/// - `result_json`   - `search_data` の返り値
/// - `expected_data` - 期待するデータ値。`i64` など `Into<serde_json::Value>` を実装した型を渡す
/// - `expected_id`   - 期待する空間ID
///
/// # Example
/// ```
/// let result_json = search_data(&test_app, "my_table", &query).await;
/// assert_first_entry(
///     &result_json,
///     3i64,
///     RawSingleId { z: 20, f: 0, x: 931386, y: 412905, i: None, t: None },
/// );
/// ```
pub fn assert_first_entry(
    result_json: &serde_json::Value,
    expected_data: impl Into<serde_json::Value>,
    expected_id: RawSingleId,
) {
    let dict = result_json["dictionary"].as_array().expect("No dictionary");
    let data = result_json["data"].as_array().expect("No data");
    assert!(!data.is_empty(), "Data is empty");

    let group = &data[0];
    let value_ref = group["valueRef"].as_u64().unwrap() as usize;
    let actual_data = &dict[value_ref];

    assert_eq!(actual_data, &expected_data.into());

    let spatial_ids = group["spatialIds"].as_array().expect("No spatialIds");
    assert!(!spatial_ids.is_empty(), "Spatial IDs are empty");

    let first_id = &spatial_ids[0];
    assert_eq!(first_id["z"], serde_json::json!(expected_id.z));
    assert_eq!(first_id["f"], serde_json::json!(expected_id.f));
    assert_eq!(first_id["x"], serde_json::json!(expected_id.x));
    assert_eq!(first_id["y"], serde_json::json!(expected_id.y));
}

/// `search_data` で得たレスポンス JSON を `HashMap<RawSingleId, T>` に変換する。
///
/// `T` には `serde::de::DeserializeOwned` を実装した任意の型を指定できる。
/// `Int` レイヤーなら `i64`、`String` レイヤーなら `String` など。
///
/// # Arguments
/// - `body_json` - `search_data` の返り値など、`ids` 配列を含む JSON
///
/// # Example
/// ```
/// // Int レイヤー: 値を i64 として取得する
/// let map: HashMap<RawSingleId, i64> = to_result_map(&result_json);
///
/// // String レイヤー: 値を String として取得する
/// let map: HashMap<RawSingleId, String> = to_result_map(&result_json);
pub fn to_result_map<T: DeserializeOwned>(
    body_json: &serde_json::Value,
) -> HashMap<RawSingleId, T> {
    let mut result_map = HashMap::new();

    if let (Some(dict), Some(data_arr)) = (
        body_json["dictionary"].as_array(),
        body_json["data"].as_array(),
    ) {
        for group in data_arr {
            let value_ref = group["valueRef"].as_u64().unwrap() as usize;
            let val = &dict[value_ref];

            if let Some(spatial_ids) = group["spatialIds"].as_array() {
                for item in spatial_ids {
                    let z = item["z"].as_i64().unwrap() as u8;
                    let f = item["f"].as_i64().unwrap() as i32;
                    let x = item["x"].as_u64().unwrap() as u32;
                    let y = item["y"].as_u64().unwrap() as u32;

                    // value は clone() が必要（複数のIDが同じ値を参照するため）
                    let data_clone: T = serde_json::from_value(val.clone()).unwrap();
                    result_map.insert(
                        RawSingleId {
                            z,
                            f,
                            x,
                            y,
                            i: None,
                            t: None,
                        },
                        data_clone,
                    );
                }
            }
        }
    }
    result_map
}
