//! 検索結果の時間表現が、暦の単位（`AllowedIntervals::calendar`）へ正しく
//! 結合し直される（coalesce される）ことを検証する。
//!
//! `i`（時間間隔）は入力時に暦の単位（`1`/`60`/`3600`/`86400`/`34359738368`）のみに
//! 制限される。`86400`（1日）は2の冪ではないため、挿入時は内部で複数の木Segmentへ
//! 分解されるが、読み出し時には暦の単位に結合し直され、単一のエントリとして戻るはず。

use kasane::models::spatial_id::RawSingleId;

use crate::database::table::common::TestApp;
use crate::database::table::data::common::put_data;

#[tokio::test]
async fn read_back_coalesces_a_non_power_of_two_calendar_interval() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    let single_id_query = serde_json::json!([{
        "z": 20, "f": 0, "x": 931386, "y": 412905,
        "i": 86400, "t": 5,
        "type": "singleId"
    }]);

    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 7, "spatial_ids": single_id_query }),
    )
    .await;

    let result_json = crate::database::table::data::common::search_data(
        &test_app,
        "test_table",
        &single_id_query,
    )
    .await;

    let data = result_json["data"].as_array().expect("no data");
    assert_eq!(data.len(), 1);
    let spatial_ids = data[0]["spatialIds"].as_array().expect("no spatialIds");
    assert_eq!(
        spatial_ids.len(),
        1,
        "expected the day-long segment to coalesce back into a single entry, got {spatial_ids:?}"
    );

    let got: RawSingleId = serde_json::from_value(spatial_ids[0].clone()).unwrap();
    assert_eq!(
        got,
        RawSingleId {
            z: 20,
            f: 0,
            x: 931386,
            y: 412905,
            i: Some(86400),
            t: Some(5),
        }
    );
}

/// 隣接する2つの `HOUR`（3600秒）Segmentを別々に挿入しても、読み出し時には
/// `i=3600, t=[0,1]` の1エントリへ結合される。
#[tokio::test]
async fn read_back_coalesces_adjacent_segments_into_a_range() {
    let test_app = TestApp::new();
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int", 25)
        .await;

    for t in [0, 1] {
        let query = serde_json::json!([{
            "z": 20, "f": 0, "x": 931386, "y": 412905,
            "i": 3600, "t": t,
            "type": "singleId"
        }]);
        put_data(
            &test_app,
            "test_table",
            &serde_json::json!({ "value": 9, "spatial_ids": query }),
        )
        .await;
    }

    let range_query = serde_json::json!([{
        "z": 20, "f": [0, 0], "x": [931386, 931386], "y": [412905, 412905],
        "i": 3600, "t": [0, 1],
        "type": "rangeId"
    }]);
    let body = serde_json::json!({ "spatial_ids": range_query });
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables/test_table/data/search?format=rangeId")
        .header("Content-Type", "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&body).unwrap(),
        ))
        .unwrap();
    let response = tower::ServiceExt::oneshot(test_app.app.clone(), req)
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let result_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let data = result_json["data"].as_array().expect("no data");
    assert_eq!(data.len(), 1);
    let spatial_ids = data[0]["spatialIds"].as_array().expect("no spatialIds");
    assert_eq!(
        spatial_ids.len(),
        1,
        "expected the two adjacent hour segments to coalesce into one range, got {spatial_ids:?}"
    );
    assert_eq!(spatial_ids[0]["i"], serde_json::json!(3600));
    assert_eq!(spatial_ids[0]["t"], serde_json::json!([0, 1]));
}
