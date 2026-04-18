use std::sync::Arc;

use kasane::{AppState, kasane};
use redb::Database;

#[tokio::main]
async fn main() {
    let redb = Database::create("default.kasane").unwrap();

    let app = kasane(AppState {
        redb: Arc::new(redb),
    });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
