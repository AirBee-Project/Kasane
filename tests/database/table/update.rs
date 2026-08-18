use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

use crate::common::TestApp;

#[tokio::test]
/// テーブルの名前を正常に変更できるかを検証する。
async fn test_update_table_name_success() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "old_name",
        "data_type": "Int",
        "max_zoom_level": 25
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    let _ = test_app.app.clone().oneshot(req).await.unwrap();

    let update_body = serde_json::json!({
        "name": "new_name"
    });

    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/test_db/tables/old_name")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&update_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 新しい名前で取得できるか
    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/new_name")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 古い名前では取得できないか
    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/old_name")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
/// テーブルの制約を正常に追加できるかを検証する。
async fn test_update_table_constraints_success() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "constrained_table",
        "data_type": "Int",
        "max_zoom_level": 25
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    test_app.app.clone().oneshot(req).await.unwrap();

    let update_body = serde_json::json!({
        "constraints": {
            "type": "Int",
            "min": 10,
            "max": 100
        }
    });

    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/test_db/tables/constrained_table")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&update_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["constraints"]["min"], 10);
    assert_eq!(json["constraints"]["max"], 100);
}

#[tokio::test]
/// 既存のデータが新しい制約に違反する場合、更新が拒否されることを検証する。
async fn test_update_table_constraints_with_existing_data_violation() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    // 制約なしで作成
    let create_body = serde_json::json!({
        "name": "my_table",
        "data_type": "Int",
        "max_zoom_level": 25
    });
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let single_id_query =
        serde_json::json!([{ "z": 20, "f": 0, "x": 0, "y": 0, "type": "singleId" }]);
    let insert_body = serde_json::json!({
        "value": 5,
        "spatial_ids": single_id_query
    });
    let req = Request::builder()
        .method("PUT")
        .uri("/databases/test_db/tables/my_table/data")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&insert_body).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // 新しい制約を設定 (最小値: 10)
    let update_body = serde_json::json!({
        "constraints": {
            "type": "Int",
            "min": 10
        },
        "validate_existing_data": true
    });
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/test_db/tables/my_table")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&update_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    // データが違反しているため更新が拒否される
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
/// 制約の型がテーブルのデータ型と一致しない場合、拒否されることを検証する。
async fn test_update_table_constraints_type_mismatch() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "my_table",
        "data_type": "Int",
        "max_zoom_level": 25
    });
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let update_body = serde_json::json!({
        "constraints": {
            "type": "Text",
            "min_length": 5
        }
    });

    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/test_db/tables/my_table")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&update_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
/// テーブルのdescription更新が正常に行えるかを検証する。
async fn test_update_table_description_success() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "desc_table",
        "data_type": "Int",
        "max_zoom_level": 25,
        "description": "Initial description."
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();

    let _ = test_app.app.clone().oneshot(req).await.unwrap();

    let update_body = serde_json::json!({
        "description": "Updated description."
    });

    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/test_db/tables/desc_table")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&update_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/desc_table")
        .body(Body::empty())
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["description"], "Updated description.");

    // 削除（null 指定）
    let update_body = serde_json::json!({
        "description": null
    });
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/test_db/tables/desc_table")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&update_body).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/desc_table")
        .body(Body::empty())
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("description").is_none() || json["description"].is_null());

    // 再度設定して、nameだけの更新時にdescriptionが維持されるか確認
    let update_body = serde_json::json!({
        "description": "Temp description"
    });
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/test_db/tables/desc_table")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&update_body).unwrap()))
        .unwrap();
    let _ = test_app.app.clone().oneshot(req).await.unwrap();

    let update_body = serde_json::json!({
        "name": "desc_table_renamed"
    });
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/test_db/tables/desc_table")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&update_body).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/desc_table_renamed")
        .body(Body::empty())
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["description"], "Temp description");
}

#[tokio::test]
/// テーブルのdescription更新で4096文字を超える場合にエラーになるかを検証する。
async fn test_update_table_description_too_long() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "desc_table_too_long",
        "data_type": "Int",
        "max_zoom_level": 25
    });

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();
    let _ = test_app.app.clone().oneshot(req).await.unwrap();

    let long_desc = "a".repeat(kasane::models::database::MAX_DESCRIPTION_LENGTH + 1);
    let update_body = serde_json::json!({
        "description": long_desc
    });

    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/test_db/tables/desc_table_too_long")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&update_body).unwrap()))
        .unwrap();

    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
/// `is_temporal: false` で作成したテーブルは時間付きIDの書き込みが拒否されるが、
/// `is_temporal: true` へ緩めれば通るようになることを検証する。
async fn test_update_table_is_temporal_unlock_allows_temporal_write() {
    let test_app = TestApp::new().await;
    test_app.create_database("test_db").await;

    let create_body = serde_json::json!({
        "name": "locked_table",
        "data_type": "Int",
        "max_zoom_level": 25,
        "is_temporal": false
    });
    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&create_body).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // 一覧・詳細の両方で is_temporal が見えること。
    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables/locked_table")
        .body(Body::empty())
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["is_temporal"], false);

    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db/tables")
        .body(Body::empty())
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json[0]["is_temporal"], false);

    // 時間成分付きの書き込みは拒否される。
    let temporal_id = serde_json::json!(
        [{ "z": 20, "f": 0, "x": 0, "y": 0, "i": 3600, "t": 0, "type": "singleId" }]
    );
    let insert_body = serde_json::json!({
        "value": 1,
        "spatial_ids": temporal_id
    });
    let req = Request::builder()
        .method("PUT")
        .uri("/databases/test_db/tables/locked_table/data")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&insert_body).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // false への再ロックは拒否される。
    let update_body = serde_json::json!({ "is_temporal": false });
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/test_db/tables/locked_table")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&update_body).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // true への解除は成功し、応答にも反映される。
    let update_body = serde_json::json!({ "is_temporal": true });
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/test_db/tables/locked_table")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&update_body).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["is_temporal"], true);

    // 解除後は同じ時間付きIDの書き込みが通る。
    let req = Request::builder()
        .method("PUT")
        .uri("/databases/test_db/tables/locked_table/data")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&insert_body).unwrap()))
        .unwrap();
    let response = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
