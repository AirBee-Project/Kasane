use std::{net::SocketAddr, sync::Arc};

use clap::Parser;
use kasane::{AppState, auth_cache::AuthCache, db_init, kasane};

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
        .unwrap_or(5173)
}

struct TracerShutdownGuard(Option<opentelemetry_sdk::trace::SdkTracerProvider>);

impl Drop for TracerShutdownGuard {
    fn drop(&mut self) {
        let Some(provider) = self.0.take() else {
            return;
        };

        // shutdown() は内部でブロッキング待機する。バッチ処理は Tokio 上のタスクとして動くため、
        // ワーカースレッドをそのまま止めずに block_in_place で退避させてから待つ。
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
    let _tracer_provider = TracerShutdownGuard(kasane::telemetry::init_telemetry());

    let args = Args::parse();
    let db = db_init::initialize_database(&args.database_path);

    let app = kasane(AppState {
        db,
        auth_cache: Arc::new(AuthCache::new()),
    });

    let address = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    tracing::info!(
        "Kasane is running on http://{} (database: {})",
        listener.local_addr().unwrap(),
        args.database_path,
    );
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();

    // _tracer_provider がドロップされる際（panic時含む）にトレースが確実に送信される
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
