use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream, StreamExt},
};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use txapi::models::{HeartbeatData, SubscribeReq, Transaction, TxResponse};

#[derive(Clone)]
struct AppState {
    tx: Arc<Mutex<broadcast::Sender<String>>>,
}

#[tokio::main]
async fn main() {
    // Create a channel for broadcasting messages.  Holds 16 messages in the buffer.
    let (tx, _) = broadcast::channel(16);
    let app_state = AppState {
        tx: Arc::new(Mutex::new(tx)),
    };

    let app = Router::new()
        .route("/v1", get(websocket_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:9999")
        .await
        .unwrap();

    println!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

async fn websocket(socket: WebSocket, state: AppState) {
    let (sender, receiver) = socket.split();
    let tx = state.tx.lock().unwrap().clone();

    let rx = tx.subscribe();

    // Spawn a task to handle sending messages.
    tokio::spawn(write(sender, tx, rx));

    // Spawn a task to handle receiving messages.
    tokio::spawn(read(receiver, state));
}

async fn read(mut receiver: SplitStream<WebSocket>, state: AppState) {
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            println!("Received: {}", text);

            // Handle the subscription request
            if let Ok(req) = serde_json::from_str::<SubscribeReq>(&text) {
                if req.method == "subscribe" && req.params.channel == "transactions" {
                    // Send some mock transactions.  In a real application, you'd
                    // fetch these from a database or other source.
                    let transactions = vec![
                        Transaction {
                            id: "11df919988c134d97bbff2678eb68e22".to_string(),
                            timestamp: "2024-01-01T00:00:00Z".to_string(),
                            cc_number: "4473593503484549".to_string(),
                            category: "Grocery".to_string(),
                            amount_usd_cents: 10000,
                            latitude: 37.774929,
                            longitude: -122.419418,
                            country_iso: "US".to_string(),
                            city: "San Francisco".to_string(),
                        },
                        // Add more mock transactions as needed
                    ];

                    let response = TxResponse::Transactions { data: transactions };

                    if let Ok(serialized_response) = serde_json::to_string(&response) {
                        let _ = state.tx.lock().unwrap().send(serialized_response);
                    }
                } else if req.method == "subscribe" && req.params.channel == "heartbeat" {
                    let heartbeat = TxResponse::Heartbeat {
                        data: HeartbeatData {
                            status: "ok".to_string(),
                        },
                    };

                    if let Ok(serialized_response) = serde_json::to_string(&heartbeat) {
                        let _ = state.tx.lock().unwrap().send(serialized_response);
                    }
                }
            }
        }
    }
}

async fn write(
    mut sender: SplitSink<WebSocket, Message>,
    _tx: broadcast::Sender<String>,
    mut rx: broadcast::Receiver<String>,
) {
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(msg) => {
                        if sender.send(Message::Text(msg.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // If the receiver has lagged behind, it will have missed messages.
                        // You might want to handle this case, e.g., by sending a special
                        // "resync" message or closing the connection.  Here, we just continue.
                        continue;
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        }
    }
}
