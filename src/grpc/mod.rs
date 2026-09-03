use std::future::Future;
use std::net::SocketAddr;
use std::time::Duration;

use tonic::transport::Server;
use tonic::transport::server::TcpIncoming;
use tonic_web::GrpcWebLayer;

use crate::AppState;

pub mod auth;
pub mod auth_ctx;
pub mod convert;
pub mod convert_data;
pub mod convert_query;
pub mod convert_users;
pub mod data;
pub mod database;
pub mod interceptor;
pub mod query;
pub mod system;
pub mod table;
pub mod users;

pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/kasane.rs"));

    pub const FILE_DESCRIPTOR_SET: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/kasane_descriptor.bin"));
}

/// 指定アドレスへバインドする。`serve` と分けてあるのは、テストがポート `0`
/// （OS 割り当て）で実際に選ばれたポートを、サーブ開始前に知る必要があるため。
pub fn bind(app_state: AppState, addr: SocketAddr) -> std::io::Result<BoundServer> {
    let incoming = TcpIncoming::bind(addr)?;
    let local_addr = incoming.local_addr()?;
    Ok(BoundServer {
        app_state,
        incoming,
        local_addr,
    })
}

pub struct BoundServer {
    app_state: AppState,
    incoming: TcpIncoming,
    local_addr: SocketAddr,
}

impl BoundServer {
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// `shutdown` が完了するとリスナーを閉じて戻る。
    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()>,
    ) -> Result<(), tonic::transport::Error> {
        let app_state = self.app_state;
        let (health_reporter, health_service) = tonic_health::server::health_reporter();
        // 互いに独立な更新なので、7回直列に待たず並行に走らせる。
        tokio::join!(
            health_reporter
                .set_serving::<pb::system_service_server::SystemServiceServer<system::SystemServiceImpl>>(),
            health_reporter
                .set_serving::<pb::auth_service_server::AuthServiceServer<auth::AuthServiceImpl>>(),
            health_reporter
                .set_serving::<pb::database_service_server::DatabaseServiceServer<database::DatabaseServiceImpl>>(),
            health_reporter
                .set_serving::<pb::table_service_server::TableServiceServer<table::TableServiceImpl>>(),
            health_reporter
                .set_serving::<pb::data_service_server::DataServiceServer<data::DataServiceImpl>>(),
            health_reporter
                .set_serving::<pb::query_service_server::QueryServiceServer<query::QueryServiceImpl>>(),
            health_reporter
                .set_serving::<pb::user_service_server::UserServiceServer<users::UserServiceImpl>>(),
        );

        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(pb::FILE_DESCRIPTOR_SET)
            .register_encoded_file_descriptor_set(tonic_health::pb::FILE_DESCRIPTOR_SET)
            .build_v1()
            .expect("gRPC reflection descriptor is malformed");

        let system_service = pb::system_service_server::SystemServiceServer::with_interceptor(
            system::SystemServiceImpl {
                app_state: app_state.clone(),
            },
            interceptor::require_auth,
        );
        let auth_service = pb::auth_service_server::AuthServiceServer::new(auth::AuthServiceImpl {
            app_state: app_state.clone(),
        });
        let database_service = pb::database_service_server::DatabaseServiceServer::with_interceptor(
            database::DatabaseServiceImpl {
                app_state: app_state.clone(),
            },
            interceptor::require_auth,
        );
        let table_service = pb::table_service_server::TableServiceServer::with_interceptor(
            table::TableServiceImpl {
                app_state: app_state.clone(),
            },
            interceptor::require_auth,
        );
        let data_service = pb::data_service_server::DataServiceServer::with_interceptor(
            data::DataServiceImpl {
                app_state: app_state.clone(),
            },
            interceptor::require_auth,
        );
        let query_service = pb::query_service_server::QueryServiceServer::with_interceptor(
            query::QueryServiceImpl {
                app_state: app_state.clone(),
            },
            interceptor::require_auth,
        );
        let user_service = pb::user_service_server::UserServiceServer::with_interceptor(
            users::UserServiceImpl {
                app_state: app_state.clone(),
            },
            interceptor::require_auth,
        );

        Server::builder()
            // grpc-web はブラウザの fetch/XHR で叩けるよう HTTP/1.1 でも受ける。
            .accept_http1(true)
            .layer(tower_http::trace::TraceLayer::new_for_grpc())
            .layer(grpc_web_cors_layer())
            .layer(GrpcWebLayer::new())
            .add_service(health_service)
            .add_service(reflection_service)
            .add_service(system_service)
            .add_service(auth_service)
            .add_service(database_service)
            .add_service(table_service)
            .add_service(data_service)
            .add_service(query_service)
            .add_service(user_service)
            .serve_with_incoming_shutdown(self.incoming, shutdown)
            .await
    }
}

/// `KASANE_CORS_ALLOWED_ORIGINS`（カンマ区切り）で許可オリジンを絞れる。未設定なら全オリジン
/// 許可（Bearer トークン方式で Cookie を使わないため、絞らなくてもただちに悪用できるわけでは
/// ない）。`grpc-status`/`grpc-message` はブラウザの grpc-web クライアントが読めるよう公開する。
fn grpc_web_cors_layer() -> tower_http::cors::CorsLayer {
    use tower_http::cors::{AllowOrigin, CorsLayer};

    let layer = CorsLayer::permissive()
        .expose_headers([
            http::HeaderName::from_static("grpc-status"),
            http::HeaderName::from_static("grpc-message"),
            http::HeaderName::from_static("grpc-status-details-bin"),
        ])
        .max_age(Duration::from_secs(3600));

    let origins: Vec<http::HeaderValue> = std::env::var("KASANE_CORS_ALLOWED_ORIGINS")
        .ok()
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse().ok())
                .collect()
        })
        .unwrap_or_default();

    if origins.is_empty() {
        layer
    } else {
        layer.allow_origin(AllowOrigin::list(origins))
    }
}
