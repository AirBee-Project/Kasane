use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};

use tempfile::NamedTempFile;
use tower::ServiceExt;

pub struct TestApp {
    pub app: Router,
    _temp_file: NamedTempFile,
}

impl TestApp {
    pub fn new() -> Self {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let db = kasane::db_init::initialize_database(temp_file.path().to_str().unwrap());
        let app_state = kasane::AppState {
            redb: std::sync::Arc::new(db),
        };
        let app = kasane::kasane(app_state);
        Self {
            app,
            _temp_file: temp_file,
        }
    }

    /// テスト用にデータベースを初期作成する
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
        max_zoom_level: u8,
    ) {
        let create_body = serde_json::json!({
            "name": name,
            "data_type": data_type,
            "max_zoom_level": max_zoom_level
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
