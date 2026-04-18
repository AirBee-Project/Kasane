use kasane::{AppState, kasane};

#[tokio::main]
async fn main() {
    let app = kasane(AppState {});

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
