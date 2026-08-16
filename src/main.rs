use kasane::{AppState, backend, kasane};
use std::net::SocketAddr;

#[cfg(feature = "production")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let _telemetry = kasane::telemetry::init_telemetry();
    let database_path = backend::default_target();
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(5172);
    let db = backend::open(&database_path)
        .await
        .expect("failed to open the storage backend");

    let app = kasane(AppState::new(db));

    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match tokio::net::TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!("cannot listen on {address}: {e}");
            std::process::exit(1);
        }
    };

    tracing::info!(
        "Kasane is running on http://{} (backend: {}, target: {})",
        address,
        backend::NAME,
        database_path,
    );

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        tracing::error!("the HTTP server stopped with an error: {e}");
    }
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
    tracing::info!("Shutting down gracefuly, flushing traces...");
}
