//! テストバイナリ全体で共有するヘルパ。
//!
//! `TestApp` は実際に ephemeral ポートへ bind した gRPC サーバーを裏で走らせ、
//! 各サービスの tonic クライアントを提供する。既定のクライアント（`database()` 等）は
//! root のトークンを差し込み済み。認可そのものを試すテスト（`permissions.rs`）は
//! `_as` 系またはトークン省略のクライアントで個別に組み立てる。
//!
//! 各テストバイナリ（`api`, `permissions` 等）はこのモジュール全体を共有するが、
//! 使う関数の部分集合はバイナリごとに異なるため、個別に見れば未使用な関数が出る。
#![allow(dead_code)]

pub mod builders;
pub mod data;

use tonic::service::Interceptor;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::{Channel, Endpoint};

use kasane::grpc::pb;

#[derive(Clone)]
pub struct TokenInterceptor(pub Option<String>);

impl Interceptor for TokenInterceptor {
    fn call(&mut self, mut req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        if let Some(value) = &self.0
            && let Ok(v) = value.parse()
        {
            req.metadata_mut().insert("authorization", v);
        }
        Ok(req)
    }
}

pub struct TestApp {
    channel: Channel,
    root_token: String,
    app_state: kasane::AppState,
    _temp_dir: tempfile::TempDir,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

macro_rules! client_pair {
    ($default_name:ident, $as_name:ident, $module:ident, $client:ident) => {
        pub fn $as_name(
            &self,
            auth_header: Option<&str>,
        ) -> pb::$module::$client<InterceptedService<Channel, TokenInterceptor>> {
            pb::$module::$client::with_interceptor(
                self.channel.clone(),
                TokenInterceptor(auth_header.map(str::to_string)),
            )
        }

        pub fn $default_name(
            &self,
        ) -> pb::$module::$client<InterceptedService<Channel, TokenInterceptor>> {
            self.$as_name(Some(&format!("Bearer {}", self.root_token)))
        }
    };
}

impl TestApp {
    pub async fn new() -> Self {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let db = kasane::repositories::lmdb::initialize_database(temp_dir.path().to_str().unwrap())
            .unwrap();

        let app_state = kasane::AppState::new(db);
        let root_token = kasane::services::auth::generate_jwt(&app_state, "root")
            .await
            .unwrap();

        let bound = kasane::grpc::bind(app_state.clone(), "127.0.0.1:0".parse().unwrap())
            .expect("failed to bind an ephemeral port for the test gRPC server");
        let addr = bound.local_addr();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = bound
                .serve(async {
                    let _ = shutdown_rx.await;
                })
                .await;
        });

        let channel = Endpoint::from_shared(format!("http://{addr}"))
            .unwrap()
            .connect()
            .await
            .expect("failed to connect to the test gRPC server");

        Self {
            channel,
            root_token,
            app_state,
            _temp_dir: temp_dir,
            shutdown: Some(shutdown_tx),
        }
    }

    pub fn channel(&self) -> Channel {
        self.channel.clone()
    }

    pub fn root_token(&self) -> &str {
        &self.root_token
    }

    pub fn app_state(&self) -> &kasane::AppState {
        &self.app_state
    }

    pub fn auth(&self) -> pb::auth_service_client::AuthServiceClient<Channel> {
        pb::auth_service_client::AuthServiceClient::new(self.channel.clone())
    }

    client_pair!(
        system,
        system_as,
        system_service_client,
        SystemServiceClient
    );
    client_pair!(
        database,
        database_as,
        database_service_client,
        DatabaseServiceClient
    );
    client_pair!(table, table_as, table_service_client, TableServiceClient);
    client_pair!(data, data_as, data_service_client, DataServiceClient);
    client_pair!(query, query_as, query_service_client, QueryServiceClient);
    client_pair!(user, user_as, user_service_client, UserServiceClient);

    /// テスト用のデータベースを作成する。
    pub async fn create_database(&self, name: &str) {
        self.database()
            .create(pb::CreateDatabaseRequest {
                name: name.to_string(),
                description: None,
            })
            .await
            .unwrap();
    }

    pub async fn create_table(
        &self,
        db_name: &str,
        name: &str,
        data_type: &str,
        max_zoom_level: u8,
    ) {
        let data_type = builders::table_data_type(data_type) as i32;
        self.table()
            .create(pb::CreateTableRequest {
                db_name: db_name.to_string(),
                name: name.to_string(),
                data_type,
                max_zoom_level: max_zoom_level as u32,
                constraints: None,
                description: None,
                value_index: false,
                is_temporal: true,
            })
            .await
            .unwrap();
    }
}
