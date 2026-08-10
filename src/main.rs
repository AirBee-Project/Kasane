use std::net::SocketAddr;

use clap::Parser;
use kasane::{AppState, kasane};

#[cfg(feature = "backend-lmdb")]
use kasane::db_init;

#[cfg(feature = "production")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    #[arg(long, default_value_t = default_database_path())]
    database_path: String,

    #[arg(long, default_value_t = default_port())]
    port: u16,
}

fn default_database_path() -> String {
    std::env::var("DATABASE_DIR").unwrap_or_else(|_| "default_kasane_db".to_string())
}

fn default_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(5172)
}

#[cfg(feature = "production")]
struct TracerShutdownGuard(Option<opentelemetry_sdk::trace::SdkTracerProvider>);

#[cfg(feature = "production")]
impl Drop for TracerShutdownGuard {
    fn drop(&mut self) {
        let Some(provider) = self.0.take() else {
            return;
        };

        let result = match tokio::runtime::Handle::try_current() {
            Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| provider.shutdown())
            }
            _ => provider.shutdown(),
        };

        if let Err(e) = result {
            eprintln!("Failed to shutdown TracerProvider: {:?}", e);
        }
    }
}

#[tokio::main]
async fn main() {
    // 環境変数の読み込み
    dotenvy::dotenv().ok();

    // ログおよびテレメトリの初期化
    #[cfg(feature = "production")]
    let _tracer_provider = TracerShutdownGuard(kasane::telemetry::init_telemetry());
    #[cfg(not(feature = "production"))]
    kasane::telemetry::init_telemetry();

    let args = Args::parse();

    // バックエンドはビルド時に 1 つへ確定している（kasane::backend を参照）。
    #[cfg(feature = "backend-lmdb")]
    let (db, target) = (
        db_init::initialize_database(&args.database_path),
        args.database_path.clone(),
    );

    #[cfg(feature = "backend-tikv")]
    let (db, target) = {
        use kasane::repositories::tikv::{TikvConfig, TikvDb};
        let config = TikvConfig::from_env();
        let target = config.pd_endpoints.join(",");
        (
            TikvDb::connect(config)
                .await
                .expect("failed to connect to TiKV"),
            target,
        )
    };

    let app = kasane(AppState { db });

    let address = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    tracing::info!(
        "Kasane is running on http://{} (backend: {}, target: {})",
        listener.local_addr().unwrap(),
        kasane::backend::NAME,
        target,
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
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
