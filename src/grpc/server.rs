use std::sync::Arc;

use tonic::service::Routes;
use tonic::service::interceptor::InterceptedService;

use super::{Service, auth, pb};
use crate::Kasane;

/// 全サービスを載せたルート。待ち受けと HTTP のレイヤー（grpc-web、CORS など）は呼び出し側が付ける。
pub async fn routes(kasane: Kasane) -> Routes {
    use pb::{
        auth_service_server::AuthServiceServer, data_service_server::DataServiceServer,
        database_service_server::DatabaseServiceServer, query_service_server::QueryServiceServer,
        system_service_server::SystemServiceServer, table_service_server::TableServiceServer,
        user_service_server::UserServiceServer,
    };

    let (health_reporter, health_service) = tonic_health::server::health_reporter();
    tokio::join!(
        health_reporter.set_serving::<SystemServiceServer<Service>>(),
        health_reporter.set_serving::<AuthServiceServer<Service>>(),
        health_reporter.set_serving::<DatabaseServiceServer<Service>>(),
        health_reporter.set_serving::<TableServiceServer<Service>>(),
        health_reporter.set_serving::<DataServiceServer<Service>>(),
        health_reporter.set_serving::<QueryServiceServer<Service>>(),
        health_reporter.set_serving::<UserServiceServer<Service>>(),
    );

    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(pb::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(tonic_health::pb::FILE_DESCRIPTOR_SET)
        .build_v1()
        .expect("gRPC reflection descriptor is malformed");

    // 全サービスで 1 つのインスタンスを共有する。
    let service = Arc::new(Service(kasane));

    Routes::new(health_service)
        .add_service(reflection_service)
        .add_service(AuthServiceServer::from_arc(service.clone()))
        .add_service(guarded(SystemServiceServer::from_arc(service.clone())))
        .add_service(guarded(DatabaseServiceServer::from_arc(service.clone())))
        .add_service(guarded(TableServiceServer::from_arc(service.clone())))
        .add_service(guarded(DataServiceServer::from_arc(service.clone())))
        .add_service(guarded(QueryServiceServer::from_arc(service.clone())))
        .add_service(guarded(UserServiceServer::from_arc(service)))
}

type AuthInterceptor = fn(tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status>;

fn guarded<S>(server: S) -> InterceptedService<S, AuthInterceptor> {
    InterceptedService::new(server, auth::require_auth)
}
