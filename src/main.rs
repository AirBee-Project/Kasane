use std::sync::Arc;

use kasane::{AppState, db_init, kasane};

#[tokio::main]
async fn main() {
    let redb = db_init::initialize_database("default.kasane");

    let app = kasane(AppState {
        redb: Arc::new(redb),
    });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
