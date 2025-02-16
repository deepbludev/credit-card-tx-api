use axum::{routing::get, Router};
use txapi::{api, core::prelude::*};

#[tokio::main]
async fn main() {
    let app_state = AppState {
        broadcaster_tx: api::ws::init_broadcaster().await,
    };

    let app = Router::new()
        .route("/ws/v1", get(api::ws::websocket_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:9999")
        .await
        .unwrap();

    println!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
