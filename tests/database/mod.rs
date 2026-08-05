use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use kasane::{AppState, db_init, kasane};
use std::sync::Arc;
use tower::ServiceExt;

pub struct DbTestApp {
    pub app: axum::Router,
    _temp_dir: tempfile::TempDir,
}

impl DbTestApp {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db = db_init::initialize_database(temp_dir.path().to_str().unwrap());

        let app_state = AppState {
            db: db.clone(),
            auth_cache: Arc::new(kasane::auth_cache::AuthCache::new()),
        };
        let token = kasane::services::auth::generate_jwt(&app_state, "root").unwrap();
        let auth_header = axum::http::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap();

        let app = kasane(app_state).layer(axum::middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                let auth_header = auth_header.clone();
                async move {
                    req.headers_mut()
                        .insert(axum::http::header::AUTHORIZATION, auth_header);
                    next.run(req).await
                }
            },
        ));
        Self {
            app,
            _temp_dir: temp_dir,
        }
    }
}

#[tokio::test]
/// データベースの作成と一覧取得が正常に行えるかを検証する。
async fn test_create_and_list_database() {
    let test_app = DbTestApp::new();

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
    let test_app = DbTestApp::new();

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

/// データベースの削除時に関連するユーザー権限もクリーンアップされるかを検証する。
#[tokio::test]
async fn test_database_remove_cleans_up_privileges() {
    let test_app = DbTestApp::new();

    // 1. ユーザーを作成する
    let req = Request::builder()
        .method("POST")
        .uri("/users")
        .header("Content-Type", "application/json")
        .body(Body::from(
            r#"{"username": "test_user", "password": "password", "is_global_admin": false}"#,
        ))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 2. データベースを作成する
    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"name": "test_db"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 3. ユーザーに権限を付与する
    let req = Request::builder()
        .method("PUT")
        .uri("/users/test_user/privileges/test_db")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"role": "Manage"}"#))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    // 権限が存在することを確認する
    let req = Request::builder()
        .method("GET")
        .uri("/users/test_user/privileges")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 1);

    // 4. データベースを削除する
    let req = Request::builder()
        .method("DELETE")
        .uri("/databases/test_db")
        .body(Body::empty())
        .unwrap();
    test_app.app.clone().oneshot(req).await.unwrap();

    // 5. 権限がクリーンアップされたことを確認する
    let req = Request::builder()
        .method("GET")
        .uri("/users/test_user/privileges")
        .body(Body::empty())
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 0);
}

#[tokio::test]
/// データベースの名前変更が正常に行えるかを検証する。
async fn test_database_rename_success() {
    let test_app = DbTestApp::new();

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
    let test_app = DbTestApp::new();

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

pub mod table;

#[tokio::test]
/// データベースのdescription付与と更新が正常に行えるかを検証する。
async fn test_database_description() {
    let test_app = DbTestApp::new();

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
}

#[tokio::test]
/// データベースのdescriptionが4096文字を超える場合にエラーになるかを検証する。
async fn test_database_description_too_long() {
    let test_app = DbTestApp::new();

    // 制限文字数+1文字のdescriptionを作成
    let long_desc = "a".repeat(kasane::models::database::MAX_DESCRIPTION_LENGTH + 1);

    let req = Request::builder()
        .method("POST")
        .uri("/databases")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&serde_json::json!({
            "name": "desc_db_too_long",
            "description": long_desc
        })).unwrap()))
        .unwrap();
    let res = test_app.app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
