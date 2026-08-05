use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};

use tower::ServiceExt;

pub struct TestApp {
    pub app: Router,
    _temp_dir: tempfile::TempDir,
}

impl TestApp {
    pub fn new() -> Self {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db = kasane::db_init::initialize_database(temp_dir.path().to_str().unwrap());

        let app_state = kasane::AppState { db };
        let token = kasane::services::auth::generate_jwt(&app_state, "root").unwrap();
        let auth_header = axum::http::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap();

        let app = kasane::kasane(app_state).layer(axum::middleware::from_fn(
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

    /// テスト用のデータベースを作成する。
    pub async fn create_database(&self, name: &str) {
        let create_body = serde_json::json!({
            "name": name,
        });

        let req = axum::http::Request::builder()
            .method("POST")
            .uri("/databases")
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(
                serde_json::to_string(&create_body).unwrap(),
            ))
            .unwrap();

        let response = tower::ServiceExt::oneshot(self.app.clone(), req)
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::CREATED);
    }

    pub async fn create_table(
        &self,
        db_name: &str,
        name: &str,
        data_type: &str,
    ) {
        let create_body = serde_json::json!({
            "name": name,
            "data_type": data_type
        });

        let req = Request::builder()
            .method("POST")
            .uri(format!("/databases/{}/tables", db_name))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&create_body).unwrap()))
            .unwrap();

        let response = self.app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }
}

impl Default for TestApp {
    fn default() -> Self {
        Self::new()
    }
}
