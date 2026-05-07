use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};

use kasane::{db_init, kasane, AppState};
use std::sync::Arc;
use tempfile::NamedTempFile;
use tower::ServiceExt;

pub struct TestApp {
    pub app: Router,
    // temp_fileを保持しておくことで、テスト終了時までDBファイルを維持する
    _temp_file: NamedTempFile,
}

impl TestApp {
    pub fn new() -> Self {
        let temp_file = NamedTempFile::new().unwrap();
        let db = db_init::initialize_database(temp_file.path().to_str().unwrap());
        let app_state = AppState {
            redb: Arc::new(db),
        };
        let app = kasane(app_state);

        Self {
            app,
            _temp_file: temp_file,
        }
    }

    /// テスト用にレイヤーを初期作成するヘルパーメソッド
    pub async fn create_layer(&self, name: &str, data_type: &str, max_zoom_level: u8) {
        let create_body = serde_json::json!({
            "name": name,
            "data_type": data_type,
            "max_zoom_level": max_zoom_level
        });

        let req = Request::builder()
            .method("POST")
            .uri("/layers")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&create_body).unwrap()))
            .unwrap();

        let response = self.app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
    }
}
