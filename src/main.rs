use std::{net::SocketAddr, sync::Arc};

use tokio::sync::RwLock;

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
    std::env::var("FILE").unwrap_or_else(|_| "default.kasane".to_string())
}

fn default_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(5173)
}

fn log_filter() -> String {
    std::env::var("LOG_MODE").unwrap_or_else(|_| "kasane=info,tower_http=info".to_string())
}

#[tokio::main]
async fn main() {
    // 環境変数の読み込み
    dotenvy::dotenv().ok();

    // ログの初期化
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(log_filter()))
        .init();

    let args = Args::parse();
    let redb = db_init::initialize_database(&args.database_path);

    let app = kasane(AppState {
        redb: Arc::new(redb),
        auth_cache: Arc::new(RwLock::new(AuthCache::new())),
    });

    let address = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    tracing::info!(
        "Kasane is running on http://{} (database: {})",
        listener.local_addr().unwrap(),
        args.database_path,
    );
    axum::serve(listener, app).await.unwrap();
}
