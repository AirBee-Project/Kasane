//! データベース単位の CRUD（作成・一覧・改名・複製・説明文・削除）。

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::common::TestApp;

#[tokio::test]
/// データベースの作成と一覧取得が正常に行えるかを検証する。
async fn test_create_and_list_database() {
    let test_app = TestApp::new().await;

    let req = Request::builder()
        .method("GET")
        .uri("/databases")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    let req = Request::builder()
        .method("GET")
        .uri("/databases")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json[0]["name"], "test_db");
}

#[tokio::test]
/// データベースおよび配下のテーブルが正しく削除されるかを検証する。
async fn test_remove_database() {
    let test_app = TestApp::new().await;

    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/databases/test_db/tables")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"name": "test_table", "data_type": "Int", "max_zoom_level": 25}"#,
        ))
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/test_db")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
/// データベースの名前変更が正常に行えるかを検証する。
async fn test_database_rename_success() {
    let test_app = TestApp::new().await;

    // 1. データベースを作成する
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 2. データベースの名前を変更する
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/test_db")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"new_name": "renamed_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. 変更後のデータベースが存在することを確認する
    let req = Request::builder()
        .method("GET")
        .uri("/databases/renamed_db")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. 変更前のデータベースが存在しないことを確認する
    let req = Request::builder()
        .method("GET")
        .uri("/databases/test_db")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
/// データベースのコピーが正常に行えるかを検証する。
async fn test_database_copy_success() {
    let test_app = TestApp::new().await;

    // 1. コピー元データベースを作成する
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "src_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 2. データベースをコピーする
    let req = Request::builder()
        .method("POST")
        .uri("/databases/src_db/copy")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"copy_name": "copied_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 3. コピー先データベースが存在することを確認する
    let req = Request::builder()
        .method("GET")
        .uri("/databases/copied_db")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
/// データベースのdescription付与と更新が正常に行えるかを検証する。
async fn test_database_description() {
    let test_app = TestApp::new().await;

    // 1. description付きでデータベースを作成
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"name": "desc_db", "description": "This is a test database."}"#,
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 2. 情報を取得してdescriptionを確認
    let req = Request::builder()
        .method("GET")
        .uri("/databases/desc_db")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["description"], "This is a test database.");

    // 3. descriptionを更新
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/desc_db")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"description": "Updated description."}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. 更新後の情報を取得して確認
    let req = Request::builder()
        .method("GET")
        .uri("/databases/desc_db")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["description"], "Updated description.");

    // 5. descriptionをnullで更新（削除）
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/desc_db")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"description": null}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 6. 更新後の情報を取得してnullになっているか確認
    let req = Request::builder()
        .method("GET")
        .uri("/databases/desc_db")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json.get("description").is_none() || json["description"].is_null());

    // 7. descriptionなしで名前のみ更新（descriptionが維持されるか確認）
    // まず再設定
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/desc_db")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"description": "Temp desc"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 名前のみ更新
    let req = Request::builder()
        .method("PATCH")
        .uri("/databases/desc_db")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"new_name": "desc_db_renamed"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let req = Request::builder()
        .method("GET")
        .uri("/databases/desc_db_renamed")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["description"], "Temp desc");
}

#[tokio::test]
/// データベースのdescriptionが4096文字を超える場合にエラーになるかを検証する。
async fn test_database_description_too_long() {
    let test_app = TestApp::new().await;

    // 制限文字数+1文字のdescriptionを作成
    let long_desc = "a".repeat(kasane::models::database::MAX_DESCRIPTION_LENGTH + 1);

    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(
            serde_json::to_string(&serde_json::json!({
                "name": "desc_db_too_long",
                "description": long_desc
            }))
            .unwrap(),
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
