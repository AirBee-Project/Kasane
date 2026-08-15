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

/// 落ちるときに必ずテレメトリを送り切るための番人。
///
/// バッチ処理は溜めてから送るので、ここを通さないと最後のリクエストが丸ごと消える。
struct TelemetryGuard(kasane::telemetry::Providers);

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        // マルチスレッドランタイムの中から同期的に flush するとワーカーを塞ぐので、
        // ブロッキング可能な文脈へ移してから待つ。
        match tokio::runtime::Handle::try_current() {
            Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| self.0.shutdown());
            }
            _ => self.0.shutdown(),
        }
    }
}

#[tokio::main]
async fn main() {
    // 環境変数の読み込み
    dotenvy::dotenv().ok();

    // ログおよびテレメトリの初期化
    let _telemetry = TelemetryGuard(kasane::telemetry::init_telemetry());

    let args = Args::parse();

    // バックエンドはビルド時に 1 つへ確定している（kasane::backend を参照）。
    let db = backend::open(&args.database_path)
        .await
        .expect("failed to open the storage backend");

    let app = kasane(AppState::new(db));

    let address = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = match tokio::net::TcpListener::bind(address).await {
        Ok(listener) => listener,
        // ポート衝突は運用で普通に起きる。バックトレース付きの panic ではなく理由を出す。
        Err(e) => {
            tracing::error!("cannot listen on {address}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!(
        "Kasane is running on http://{} (backend: {}, target: {})",
        address,
        backend::NAME,
        args.database_path,
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
