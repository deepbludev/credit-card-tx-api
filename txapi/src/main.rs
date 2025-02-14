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
use txapi::models::{ChannelMsg, Heartbeat, Request, Transaction};

#[derive(Clone)]
struct AppState {
    // nothing for now...
}

#[tokio::main]
async fn main() {
    // create a channel for broadcasting messages that holds 16 messages in the buffer.
    let app_state = AppState {};

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

async fn websocket(socket: WebSocket, _state: AppState) {
    let (sender, receiver) = socket.split();

    // Update channel type to use Request directly
    let (tx, rx) = tokio::sync::mpsc::channel(1);

    let write_task = tokio::spawn(write(sender, rx));
    let read_task = tokio::spawn(read(receiver, tx));

    tokio::select! {
        _ = write_task => {},
        _ = read_task => {},
    }
}

async fn read(mut receiver: SplitStream<WebSocket>, tx: tokio::sync::mpsc::Sender<Request>) {
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            println!("Received: {}", text);
            if let Ok(request) = serde_json::from_str::<Request>(&text) {
                let _ = tx.send(request).await;
            } else {
                println!("Failed to parse message: {}", text);
            }
        }
    }
}

async fn write(
    mut sender: SplitSink<WebSocket, Message>,
    mut rx: tokio::sync::mpsc::Receiver<Request>,
) {
    let mut subscribed_channels: Vec<String> = Vec::new();
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));

    loop {
        tokio::select! {
            Some(request) = rx.recv() => {
                match request {
                    Request::Subscribe { params } => {
                        if !subscribed_channels.contains(&params.channel) {
                            subscribed_channels.push(params.channel.clone());
                        }
                        let status = format!("Successfully subscribed to {} channel", params.channel);
                        let ack = ChannelMsg::Heartbeat {
                            data: Heartbeat {
                                status,
                            },
                        };
                        if let Ok(serialized) = serde_json::to_string(&ack) {
                            if sender.send(Message::Text(serialized.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Request::Unsubscribe { params } => {
                        subscribed_channels.retain(|c| c != &params.channel);
                        let status = format!("Successfully unsubscribed from {} channel", params.channel);
                        let ack = ChannelMsg::Heartbeat {
                            data: Heartbeat {
                                status,
                            },
                        };
                        if let Ok(serialized) = serde_json::to_string(&ack) {
                            if sender.send(Message::Text(serialized.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }

            _ = interval.tick() => {
                if subscribed_channels.contains(&"transactions".to_string()) {
                    let transaction = Transaction {
                        id: uuid::Uuid::new_v4().to_string().replace("-", ""),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        cc_number: "4473593503484549".to_string(),
                        category: "Grocery".to_string(),
                        amount_usd_cents: rand::random::<u64>() % 10000,
                        latitude: 37.774929,
                        longitude: -122.419418,
                        country_iso: "US".to_string(),
                        city: "San Francisco".to_string(),
                    };

                    let msg = ChannelMsg::Transactions {
                        data: vec![transaction],
                    };

                    if let Ok(serialized) = serde_json::to_string(&msg) {
                        if sender.send(Message::Text(serialized.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }
}
