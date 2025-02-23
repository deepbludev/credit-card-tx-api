use axum::{routing::get, Router};
use txapi::{api, core::prelude::*, stream};
/// Initialize the application state.
///
/// This function initializes the application state by injecting all the
/// necessary dependencies into the AppState struct.
///
/// The main dependencies are the websocket channel senders, which are used to broadcast
/// messages to the websocket clients.
///
async fn init_app_state() -> AppState {
    let (transactions_tx, _) = stream::transactions::channel().await;
    let (heartbeat_tx, _) = stream::heartbeat::channel().await;

    AppState {
        heartbeat_tx,
        transactions_tx,
    }
}
fn init_logger() {
    use tracing_subscriber;

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_file(true)
        .init();
}

#[tokio::main]
async fn main() {
    init_logger();
    let app_state = init_app_state().await;

    let app = Router::new()
        .route("/ws/v1", get(api::ws::endpoint))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:9999")
        .await
        .unwrap();

    println!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
