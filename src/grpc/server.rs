use std::future::Future;
use std::net::SocketAddr;
use std::time::Duration;

use tonic::transport::Server;
use tonic::transport::server::TcpIncoming;
use tonic_web::GrpcWebLayer;

use super::{auth, pb};
use crate::Kasane;

impl Kasane {
    /// 指定アドレスへバインドする。`serve` と分けてあるのは、テストがポート `0` （OS 割り当て）で実際に選ばれたポートを、サーブ開始前に知る必要があるため。
    pub fn bind(self, addr: SocketAddr) -> std::io::Result<BoundServer> {
        let incoming = TcpIncoming::bind(addr)?;
        let local_addr = incoming.local_addr()?;
        Ok(BoundServer {
            kasane: self,
            incoming,
            local_addr,
        })
    }
}

pub struct BoundServer {
    kasane: Kasane,
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
        let kasane = self.kasane;
        let (health_reporter, health_service) = tonic_health::server::health_reporter();
        tokio::join!(
            health_reporter.set_serving::<pb::system_service_server::SystemServiceServer<Kasane>>(),
            health_reporter.set_serving::<pb::auth_service_server::AuthServiceServer<Kasane>>(),
            health_reporter
                .set_serving::<pb::database_service_server::DatabaseServiceServer<Kasane>>(),
            health_reporter.set_serving::<pb::table_service_server::TableServiceServer<Kasane>>(),
            health_reporter.set_serving::<pb::data_service_server::DataServiceServer<Kasane>>(),
            health_reporter.set_serving::<pb::query_service_server::QueryServiceServer<Kasane>>(),
            health_reporter.set_serving::<pb::user_service_server::UserServiceServer<Kasane>>(),
        );

        let reflection_service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(pb::FILE_DESCRIPTOR_SET)
            .register_encoded_file_descriptor_set(tonic_health::pb::FILE_DESCRIPTOR_SET)
            .build_v1()
            .expect("gRPC reflection descriptor is malformed");

        let system_service = pb::system_service_server::SystemServiceServer::with_interceptor(
            kasane.clone(),
            auth::require_auth,
        );
        let auth_service = pb::auth_service_server::AuthServiceServer::new(kasane.clone());
        let database_service = pb::database_service_server::DatabaseServiceServer::with_interceptor(
            kasane.clone(),
            auth::require_auth,
        );
        let table_service = pb::table_service_server::TableServiceServer::with_interceptor(
            kasane.clone(),
            auth::require_auth,
        );
        let data_service = pb::data_service_server::DataServiceServer::with_interceptor(
            kasane.clone(),
            auth::require_auth,
        );
        let query_service = pb::query_service_server::QueryServiceServer::with_interceptor(
            kasane.clone(),
            auth::require_auth,
        );
        let user_service = pb::user_service_server::UserServiceServer::with_interceptor(
            kasane.clone(),
            auth::require_auth,
        );

        Server::builder()
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

/// ブラウザから grpc-web で呼べるよう、すべてのオリジンを許可する。
fn grpc_web_cors_layer() -> tower_http::cors::CorsLayer {
    tower_http::cors::CorsLayer::permissive()
        .expose_headers([
            http::HeaderName::from_static("grpc-status"),
            http::HeaderName::from_static("grpc-message"),
            http::HeaderName::from_static("grpc-status-details-bin"),
        ])
        .max_age(Duration::from_secs(3600))
}
