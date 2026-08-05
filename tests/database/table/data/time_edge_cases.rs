//! 時間軸の入出力に関する境界値・コーナーケースの検証。
//!
//! - 入力バリデーション（`i`/`t` の組み合わせ、暦の単位以外の拒否、FlexIdのズーム範囲）
//! - 過剰結合・過小結合が起きていないか（隣接していないSegmentや値が違うSegmentは
//!   結合されてはならない／暦のより粗い単位に丸め込めるときはそちらへ丸め込まれる）
//! - 上書き・削除が時間Segment単位で正しく効くか
//! - 境界値でのラウンドトリップ（秒単位・全時間）

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use tower::ServiceExt;

use crate::database::table::common::TestApp;
use crate::database::table::data::common::{put_data, search_data};

async fn setup(test_app: &TestApp) {
    test_app.create_database("test_db").await;
    test_app
        .create_table("test_db", "test_table", "Int")
        .await;
}

/// `PUT /data` を投げてステータスコードだけ返す（成功・失敗どちらも許容する）。
async fn put_status(
    test_app: &TestApp,
    body: &serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("PUT")
        .uri("/databases/test_db/tables/test_table/data")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(body).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

/// `POST /data/search?format=rangeId` の生レスポンスJSONを返す。
async fn search_range(test_app: &TestApp, query: &serde_json::Value) -> serde_json::Value {
    let body = serde_json::json!({ "spatial_ids": query });
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables/test_table/data/search?format=rangeId")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// `POST /data/search?format=flexId` の生レスポンスJSONを返す。
async fn search_flex(test_app: &TestApp, query: &serde_json::Value) -> serde_json::Value {
    let body = serde_json::json!({ "spatial_ids": query });
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables/test_table/data/search?format=flexId")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// ---------------------------------------------------------------------------
// 入力バリデーション
// ---------------------------------------------------------------------------

#[tokio::test]
async fn i_without_t_is_rejected_for_single_id() {
    let test_app = TestApp::new();
    setup(&test_app).await;
    let body = serde_json::json!({
        "value": 1,
        "spatial_ids": [{"type":"singleId","z":20,"f":0,"x":1,"y":1,"i":3600}]
    });
    let (status, _) = put_status(&test_app, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn t_without_i_is_rejected_for_single_id() {
    let test_app = TestApp::new();
    setup(&test_app).await;
    let body = serde_json::json!({
        "value": 1,
        "spatial_ids": [{"type":"singleId","z":20,"f":0,"x":1,"y":1,"t":0}]
    });
    let (status, _) = put_status(&test_app, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn i_without_t_is_rejected_for_range_id() {
    let test_app = TestApp::new();
    setup(&test_app).await;
    let body = serde_json::json!({
        "value": 1,
        "spatial_ids": [{"type":"rangeId","z":20,"f":[0,0],"x":[1,1],"y":[1,1],"i":3600}]
    });
    let (status, _) = put_status(&test_app, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn t_without_i_is_rejected_for_range_id() {
    let test_app = TestApp::new();
    setup(&test_app).await;
    let body = serde_json::json!({
        "value": 1,
        "spatial_ids": [{"type":"rangeId","z":20,"f":[0,0],"x":[1,1],"y":[1,1],"t":[0,1]}]
    });
    let (status, _) = put_status(&test_app, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn t_zoomlevel_without_t_index_is_rejected_for_flex_id() {
    let test_app = TestApp::new();
    setup(&test_app).await;
    let body = serde_json::json!({
        "value": 1,
        "spatial_ids": [{
            "type":"flexId",
            "fZoomlevel":20,"fIndex":0,"xZoomlevel":20,"xIndex":1,"yZoomlevel":20,"yIndex":1,
            "tZoomlevel":20
        }]
    });
    let (status, _) = put_status(&test_app, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn t_index_without_t_zoomlevel_is_rejected_for_flex_id() {
    let test_app = TestApp::new();
    setup(&test_app).await;
    let body = serde_json::json!({
        "value": 1,
        "spatial_ids": [{
            "type":"flexId",
            "fZoomlevel":20,"fIndex":0,"xZoomlevel":20,"xIndex":1,"yZoomlevel":20,"yIndex":1,
            "tIndex":5
        }]
    });
    let (status, _) = put_status(&test_app, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// `i` は暦の単位（1/60/3600/86400/2^35）以外は拒否される。
#[tokio::test]
async fn non_calendar_intervals_are_rejected() {
    let test_app = TestApp::new();
    setup(&test_app).await;
    for i in [2u64, 30, 1800, 7200, 43200, 172800] {
        let body = serde_json::json!({
            "value": 1,
            "spatial_ids": [{"type":"singleId","z":20,"f":0,"x":1,"y":1,"i":i,"t":0}]
        });
        let (status, resp) = put_status(&test_app, &body).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "i={i} should have been rejected"
        );
        assert_eq!(resp["code"], "invalid_spatial_id", "i={i}: {resp:?}");
    }
}

#[tokio::test]
async fn zero_interval_is_rejected() {
    let test_app = TestApp::new();
    setup(&test_app).await;
    let body = serde_json::json!({
        "value": 1,
        "spatial_ids": [{"type":"singleId","z":20,"f":0,"x":1,"y":1,"i":0,"t":0}]
    });
    let (status, resp) = put_status(&test_app, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(resp["code"], "invalid_spatial_id", "{resp:?}");
}

/// 「時間指定が不正」というひとつのユーザーミスに、複数のエラーコードを割り当てない。
///
/// `i=0` は kasane-logic の `Interval::new` が、`i=7` はこちらの暦チェックが、
/// `t` の範囲外は `with_time` が弾く——と検出箇所はバラバラだが、クライアントから見れば
/// どれも同じ「`i`/`t` の指定ミス」なので、`code` は1つに揃っていなければならない。
#[tokio::test]
async fn every_invalid_time_specification_shares_one_error_code() {
    let test_app = TestApp::new();
    setup(&test_app).await;

    let single = |extra: serde_json::Value| {
        let mut id = serde_json::json!({"type":"singleId","z":20,"f":0,"x":1,"y":1});
        let obj = id.as_object_mut().unwrap();
        for (k, v) in extra.as_object().unwrap() {
            obj.insert(k.clone(), v.clone());
        }
        serde_json::json!({ "value": 1, "spatial_ids": [id] })
    };

    let cases = [
        // `Interval::new` が弾く（0 / 上限超え）
        ("i=0", single(serde_json::json!({"i":0,"t":0}))),
        (
            "i over max",
            single(serde_json::json!({"i":34359738369u64,"t":0})),
        ),
        // 暦の単位チェックが弾く
        ("i=7", single(serde_json::json!({"i":7,"t":0}))),
        // `with_time` が弾く（区間の終端が 2^35 秒を超える）
        (
            "t out of range",
            single(serde_json::json!({"i":86400,"t":u64::MAX})),
        ),
        // 片方だけの指定
        ("i without t", single(serde_json::json!({"i":3600}))),
        ("t without i", single(serde_json::json!({"t":0}))),
        // FlexId 側も同じコードに揃える
        (
            "flexId tZoomlevel without tIndex",
            serde_json::json!({
                "value": 1,
                "spatial_ids": [{
                    "type":"flexId",
                    "fZoomlevel":20,"fIndex":0,"xZoomlevel":20,"xIndex":1,
                    "yZoomlevel":20,"yIndex":1,"tZoomlevel":25
                }]
            }),
        ),
        (
            "flexId tZoomlevel out of range",
            serde_json::json!({
                "value": 1,
                "spatial_ids": [{
                    "type":"flexId",
                    "fZoomlevel":20,"fIndex":0,"xZoomlevel":20,"xIndex":1,
                    "yZoomlevel":20,"yIndex":1,"tZoomlevel":36,"tIndex":0
                }]
            }),
        ),
    ];

    for (label, body) in cases {
        let (status, resp) = put_status(&test_app, &body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{label}: {resp:?}");
        assert_eq!(
            resp["code"], "invalid_spatial_id",
            "{label} should use the same error code as every other bad time spec: {resp:?}"
        );
    }
}

/// 暦の単位はすべて受理される（境界値を1つずつ確認）。
#[tokio::test]
async fn every_calendar_interval_is_accepted() {
    let test_app = TestApp::new();
    setup(&test_app).await;
    for i in [1u64, 60, 3600, 86400, 34359738368] {
        let body = serde_json::json!({
            "value": 1,
            "spatial_ids": [{"type":"singleId","z":20,"f":0,"x":1,"y":1,"i":i,"t":0}]
        });
        let (status, resp) = put_status(&test_app, &body).await;
        assert_eq!(
            status,
            StatusCode::OK,
            "i={i} should have been accepted: {resp:?}"
        );
    }
}

#[tokio::test]
async fn flex_id_t_zoomlevel_out_of_range_is_rejected() {
    let test_app = TestApp::new();
    setup(&test_app).await;
    let body = serde_json::json!({
        "value": 1,
        "spatial_ids": [{
            "type":"flexId",
            "fZoomlevel":20,"fIndex":0,"xZoomlevel":20,"xIndex":1,"yZoomlevel":20,"yIndex":1,
            "tZoomlevel":36,"tIndex":0
        }]
    });
    let (status, _) = put_status(&test_app, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// `tZoomlevel=1` はSegmentが2つ（インデックス0,1）しかないので、`tIndex=5` は範囲外。
#[tokio::test]
async fn flex_id_t_index_out_of_range_is_rejected() {
    let test_app = TestApp::new();
    setup(&test_app).await;
    let body = serde_json::json!({
        "value": 1,
        "spatial_ids": [{
            "type":"flexId",
            "fZoomlevel":20,"fIndex":0,"xZoomlevel":20,"xIndex":1,"yZoomlevel":20,"yIndex":1,
            "tZoomlevel":1,"tIndex":5
        }]
    });
    let (status, _) = put_status(&test_app, &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// 過剰結合・過小結合
// ---------------------------------------------------------------------------

/// 同じ値でも、間に隙間がある2つのSegmentは結合されてはならない
/// （t=0とt=5をひとまとめの範囲[0,5]にしてしまうと、挿入していないt=1..4も
/// 含まれることになり、存在しないデータを存在するかのように返してしまう）。
#[tokio::test]
async fn non_adjacent_segments_do_not_coalesce() {
    let test_app = TestApp::new();
    setup(&test_app).await;

    for t in [0, 5] {
        let query = serde_json::json!([{
            "type":"singleId","z":20,"f":0,"x":1,"y":1,"i":3600,"t":t
        }]);
        put_data(
            &test_app,
            "test_table",
            &serde_json::json!({ "value": 42, "spatial_ids": query }),
        )
        .await;
    }

    let query_all = serde_json::json!([{
        "type":"rangeId","z":20,"f":[0,0],"x":[1,1],"y":[1,1],"i":3600,"t":[0,5]
    }]);
    let result = search_range(&test_app, &query_all).await;
    let data = result["data"].as_array().expect("no data");
    assert_eq!(
        data.len(),
        1,
        "both segments share the same value, so one dictionary entry"
    );
    let spatial_ids = data[0]["spatialIds"].as_array().expect("no spatialIds");
    assert_eq!(
        spatial_ids.len(),
        2,
        "gapped segments must stay as two separate entries, got {spatial_ids:?}"
    );
    let ts: std::collections::BTreeSet<i64> = spatial_ids
        .iter()
        .map(|id| id["t"][0].as_i64().unwrap())
        .collect();
    assert_eq!(ts, [0, 5].into_iter().collect());
}

/// 隣接していても値が異なるSegmentは結合されてはならない。
#[tokio::test]
async fn different_values_do_not_coalesce_even_if_adjacent() {
    let test_app = TestApp::new();
    setup(&test_app).await;

    for (t, value) in [(0, 1), (1, 2)] {
        let query = serde_json::json!([{
            "type":"singleId","z":20,"f":0,"x":1,"y":1,"i":3600,"t":t
        }]);
        put_data(
            &test_app,
            "test_table",
            &serde_json::json!({ "value": value, "spatial_ids": query }),
        )
        .await;
    }

    let query_all = serde_json::json!([{
        "type":"rangeId","z":20,"f":[0,0],"x":[1,1],"y":[1,1],"i":3600,"t":[0,1]
    }]);
    let result = search_range(&test_app, &query_all).await;
    let data = result["data"].as_array().expect("no data");
    assert_eq!(
        data.len(),
        2,
        "different values must stay in separate dictionary groups"
    );
    for group in data {
        let spatial_ids = group["spatialIds"].as_array().unwrap();
        assert_eq!(
            spatial_ids.len(),
            1,
            "each group should hold exactly its own segment"
        );
        assert_eq!(spatial_ids[0]["i"], serde_json::json!(3600));
    }
}

/// 1時間 (`HOUR`) を24個ぶん連続で挿入すると、読み出し時は暦の中で最も粗い単位
/// （`DAY` = 86400秒）へ丸め込まれ、単一のエントリとして戻る。
#[tokio::test]
async fn twenty_four_adjacent_hours_coalesce_into_a_day() {
    let test_app = TestApp::new();
    setup(&test_app).await;

    for t in 0..24 {
        let query = serde_json::json!([{
            "type":"singleId","z":20,"f":0,"x":1,"y":1,"i":3600,"t":t
        }]);
        put_data(
            &test_app,
            "test_table",
            &serde_json::json!({ "value": 42, "spatial_ids": query }),
        )
        .await;
    }

    let query_all = serde_json::json!([{
        "type":"rangeId","z":20,"f":[0,0],"x":[1,1],"y":[1,1],"i":3600,"t":[0,23]
    }]);
    let result = search_range(&test_app, &query_all).await;
    let data = result["data"].as_array().expect("no data");
    assert_eq!(data.len(), 1);
    let spatial_ids = data[0]["spatialIds"].as_array().expect("no spatialIds");
    assert_eq!(
        spatial_ids.len(),
        1,
        "24 contiguous hours spanning exactly one day should coalesce into a single entry, got {spatial_ids:?}"
    );
    assert_eq!(
        spatial_ids[0]["i"],
        serde_json::json!(86400),
        "should roll up to the coarsest calendar unit that exactly covers the range (DAY), got {:?}",
        spatial_ids[0]
    );
    // RangeId の t は常に [min, max] なので、単一点でも [0, 0] になる。
    assert_eq!(spatial_ids[0]["t"], serde_json::json!([0, 0]));
}

/// `flexId` フォーマットでは、時間方向の暦への結合は行わない（木の生Segmentをそのまま返す）。
///
/// `i=3600`（時、2の冪ではない）で挿入すると、挿入時点で複数の生Segmentへ分解される。
/// `rangeId`/`singleId` 出力はこれを暦の単位へ結合し直して1件にするが、`flexId` 出力は
/// 結合を行わないため、生Segment数（1件より多い）がそのまま見える。
/// （`i=1` のような2の冪の間隔だと、隣接Segmentは木自体がストレージ時点で1つの物理
/// ノードへ圧縮してしまうため、このテストの意図には使えない。）
#[tokio::test]
async fn flex_id_format_does_not_coalesce_across_segments() {
    let test_app = TestApp::new();
    setup(&test_app).await;

    for t in [0, 1] {
        let query = serde_json::json!([{
            "type":"singleId","z":20,"f":0,"x":1,"y":1,"i":3600,"t":t
        }]);
        put_data(
            &test_app,
            "test_table",
            &serde_json::json!({ "value": 42, "spatial_ids": query }),
        )
        .await;
    }

    let query_all = serde_json::json!([{
        "type":"rangeId","z":20,"f":[0,0],"x":[1,1],"y":[1,1],"i":3600,"t":[0,1]
    }]);

    // rangeId: 暦の単位へ結合され1件になる。
    let range_result = search_range(&test_app, &query_all).await;
    let range_data = range_result["data"].as_array().expect("no data");
    assert_eq!(range_data.len(), 1);
    let range_ids = range_data[0]["spatialIds"]
        .as_array()
        .expect("no spatialIds");
    assert_eq!(
        range_ids.len(),
        1,
        "rangeId output should coalesce the two adjacent hours into one entry, got {range_ids:?}"
    );
    assert_eq!(range_ids[0]["i"], serde_json::json!(3600));
    assert_eq!(range_ids[0]["t"], serde_json::json!([0, 1]));

    // flexId: 生Segmentのままなので、rangeId側より多いエントリ数になる（結合しない）。
    let flex_result = search_flex(&test_app, &query_all).await;
    let flex_data = flex_result["data"].as_array().expect("no data");
    assert_eq!(flex_data.len(), 1);
    let flex_ids = flex_data[0]["spatialIds"]
        .as_array()
        .expect("no spatialIds");
    assert!(
        flex_ids.len() > 1,
        "flexId output must not be coalesced down to a single entry like rangeId is, got {flex_ids:?}"
    );
}

// ---------------------------------------------------------------------------
// 上書き・削除
// ---------------------------------------------------------------------------

/// 同じ時間Segmentへの上書きは、新しい値だけが残る（古い値が残留しない）。
#[tokio::test]
async fn overwrite_replaces_the_value_at_the_same_time_segment() {
    let test_app = TestApp::new();
    setup(&test_app).await;

    let query = serde_json::json!([{
        "type":"singleId","z":20,"f":0,"x":1,"y":1,"i":3600,"t":0
    }]);
    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 1, "spatial_ids": query }),
    )
    .await;
    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 2, "spatial_ids": query }),
    )
    .await;

    let result = search_data(&test_app, "test_table", &query).await;
    let data = result["data"].as_array().unwrap();
    assert_eq!(
        data.len(),
        1,
        "only the new value should remain, got {data:?}"
    );
    let dict = result["dictionary"].as_array().unwrap();
    assert_eq!(
        dict[data[0]["valueRef"].as_u64().unwrap() as usize],
        serde_json::json!(2)
    );
}

/// 隣接する時間Segmentの片方を削除しても、もう片方には影響しない。
#[tokio::test]
async fn removing_one_segment_leaves_the_adjacent_segment_intact() {
    let test_app = TestApp::new();
    setup(&test_app).await;

    for t in [0, 1] {
        let query = serde_json::json!([{
            "type":"singleId","z":20,"f":0,"x":1,"y":1,"i":3600,"t":t
        }]);
        put_data(
            &test_app,
            "test_table",
            &serde_json::json!({ "value": 42, "spatial_ids": query }),
        )
        .await;
    }

    let remove_query = serde_json::json!({
        "spatial_ids": [{"type":"singleId","z":20,"f":0,"x":1,"y":1,"i":3600,"t":0}]
    });
    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/test_db/tables/test_table/data")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&remove_query).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let remaining_query = serde_json::json!([{
        "type":"singleId","z":20,"f":0,"x":1,"y":1,"i":3600,"t":1
    }]);
    let result = search_data(&test_app, "test_table", &remaining_query).await;
    let data = result["data"].as_array().unwrap();
    assert_eq!(
        data.len(),
        1,
        "t=1 must still be present after removing t=0"
    );

    let removed_query = serde_json::json!([{
        "type":"singleId","z":20,"f":0,"x":1,"y":1,"i":3600,"t":0
    }]);
    let result = search_data(&test_app, "test_table", &removed_query).await;
    let data = result["data"].as_array().unwrap();
    assert!(
        data.is_empty(),
        "t=0 must be gone after removal, got {data:?}"
    );
}

// ---------------------------------------------------------------------------
// ラウンドトリップの境界値
// ---------------------------------------------------------------------------

/// `i=1`（秒、2の冪）は分解が起きないため、常に厳密にラウンドトリップする。
#[tokio::test]
async fn second_granularity_round_trips_exactly() {
    let test_app = TestApp::new();
    setup(&test_app).await;

    let query = serde_json::json!([{
        "type":"singleId","z":20,"f":0,"x":1,"y":1,"i":1,"t":1_700_000_000u64
    }]);
    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 42, "spatial_ids": query }),
    )
    .await;

    let result = search_data(&test_app, "test_table", &query).await;
    let data = result["data"].as_array().unwrap();
    assert_eq!(data.len(), 1);
    let spatial_ids = data[0]["spatialIds"].as_array().unwrap();
    assert_eq!(spatial_ids.len(), 1);
    assert_eq!(spatial_ids[0]["i"], serde_json::json!(1));
    assert_eq!(spatial_ids[0]["t"], serde_json::json!(1_700_000_000u64));
}

/// 時間を指定しない場合は「全時間」を表し、出力にも `i`/`t` は現れない。
#[tokio::test]
async fn whole_time_has_no_i_or_t_in_output() {
    let test_app = TestApp::new();
    setup(&test_app).await;

    let query = serde_json::json!([{"type":"singleId","z":20,"f":0,"x":1,"y":1}]);
    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 42, "spatial_ids": query }),
    )
    .await;

    let result = search_data(&test_app, "test_table", &query).await;
    let data = result["data"].as_array().unwrap();
    assert_eq!(data.len(), 1);
    let spatial_ids = data[0]["spatialIds"].as_array().unwrap();
    assert_eq!(spatial_ids.len(), 1);
    assert!(
        spatial_ids[0].get("i").is_none(),
        "i must be absent for whole time, got {:?}",
        spatial_ids[0]
    );
    assert!(
        spatial_ids[0].get("t").is_none(),
        "t must be absent for whole time, got {:?}",
        spatial_ids[0]
    );
}

/// `RangeId` として時間の範囲を直接指定して挿入した場合も正しくラウンドトリップする。
#[tokio::test]
async fn range_id_with_a_time_range_round_trips() {
    let test_app = TestApp::new();
    setup(&test_app).await;

    let query = serde_json::json!([{
        "type":"rangeId","z":20,"f":[0,0],"x":[1,1],"y":[1,1],"i":3600,"t":[0,1]
    }]);
    put_data(
        &test_app,
        "test_table",
        &serde_json::json!({ "value": 42, "spatial_ids": query }),
    )
    .await;

    let result = search_range(&test_app, &query).await;
    let data = result["data"].as_array().unwrap();
    assert_eq!(data.len(), 1);
    let spatial_ids = data[0]["spatialIds"].as_array().unwrap();
    assert_eq!(
        spatial_ids.len(),
        1,
        "a single RangeId insert covering [0,1] must read back as one entry, got {spatial_ids:?}"
    );
    assert_eq!(spatial_ids[0]["i"], serde_json::json!(3600));
    assert_eq!(spatial_ids[0]["t"], serde_json::json!([0, 1]));
}
