use std::net::SocketAddr;
use std::time::Duration;

use kasane::Kasane;
use tonic::transport::Server;
use tonic::transport::server::TcpIncoming;

#[cfg(feature = "production")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    kasane::logging::init();
    let database_path =
        std::env::var("DATABASE_DIR").unwrap_or_else(|_| "default_kasane_db".to_string());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(5172);
    let kasane = Kasane::open(&database_path).expect("failed to open the storage backend");

    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let incoming = match TcpIncoming::bind(address) {
        Ok(incoming) => incoming,
        Err(e) => {
            tracing::error!("cannot listen on {address}: {e}");
            std::process::exit(1);
        }
    };

    let local_addr = incoming.local_addr().unwrap_or(address);
    tracing::info!("Kasane is running on grpc://{local_addr} (target: {database_path})");

    let result = Server::builder()
        .accept_http1(true)
        .layer(tower_http::trace::TraceLayer::new_for_grpc())
        .layer(grpc_web_cors_layer())
        .layer(tonic_web::GrpcWebLayer::new())
        .add_routes(kasane::grpc::routes(kasane).await)
        .serve_with_incoming_shutdown(incoming, shutdown_signal())
        .await;
    if let Err(e) = result {
        tracing::error!("the gRPC server stopped with an error: {e}");
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

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("Shutting down gracefully...");
}
