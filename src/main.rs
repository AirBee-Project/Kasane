use std::net::SocketAddr;

use clap::Parser;
use kasane::{AppState, backend, kasane};

#[cfg(feature = "production")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    /// ストレージの接続先。何を指すかはビルド時に選ばれたバックエンドによる
    /// （LMDB はデータディレクトリ、TiKV は PD エンドポイントのカンマ区切り）。
    #[arg(long, default_value_t = backend::default_target())]
    database_path: String,

    #[arg(long, default_value_t = default_port())]
    port: u16,
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
    let db = backend::open(&args.database_path)
        .await
        .expect("failed to open the storage backend");

    let app = kasane(AppState { db });

    let address = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    tracing::info!(
        "Kasane is running on http://{} (backend: {}, target: {})",
        listener.local_addr().unwrap(),
        backend::NAME,
        args.database_path,
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
