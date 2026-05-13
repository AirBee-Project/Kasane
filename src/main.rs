use std::{net::SocketAddr, sync::Arc};

use clap::Parser;
use kasane::{AppState, db_init, kasane};

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

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let args = Args::parse();
    let redb = db_init::initialize_database(&args.database_path);

    let app = kasane(AppState {
        redb: Arc::new(redb),
    });

    let address = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = tokio::net::TcpListener::bind(address).await.unwrap();
    println!(
        "Kasane is running on http://{} (save on: {})",
        listener.local_addr().unwrap(),
        args.database_path,
    );
    axum::serve(listener, app).await.unwrap();
}
