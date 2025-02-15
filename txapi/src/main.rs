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
    Stream,
};
use std::pin::Pin;
use std::time::Duration;
use txapi::models::{ChannelMsg, Heartbeat, Request, Transaction};

#[derive(Clone)]
struct AppState {
    // nothing for now...
}

#[tokio::main]
async fn main() {
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
    let mut subscriptions: Vec<String> = Vec::new();
    let mut tx_stream: Option<Pin<Box<dyn Stream<Item = Transaction> + Send>>> = None;

    loop {
        tokio::select! {
            Some(request) = rx.recv() => {
                match request {
                    // handle subscribe
                    Request::Subscribe { params } => {
                        if !subscriptions.contains(&params.channel) {
                            subscriptions.push(params.channel.clone());

                            match params.channel.as_str() {
                                "transactions" => {
                                    // start a stream for the channel
                                    tx_stream = Some(stream_transactions());
                                }
                                "heartbeat" => {
                                    // nothing to do here
                                }
                                _ => {}
                            }
                        }
                        // send ack
                        let status = format!("Successfully subscribed to {} channel", params.channel);
                        let ack = ChannelMsg::Heartbeat {
                            data: Heartbeat {
                                status
                            },
                        };
                        if let Ok(serialized) = serde_json::to_string(&ack) {
                            if sender.send(Message::Text(serialized.into())).await.is_err() {
                                break;
                            }
                        }
                    }

                    // handle unsubscribe
                    Request::Unsubscribe { params } => {
                        subscriptions.retain(|c| c != &params.channel);
                        match params.channel.as_str() {
                            "transactions" => {
                                // stop the stream
                                tx_stream = None;
                            }
                            "heartbeat" => {
                                // nothing to do here
                            }
                            _ => {}
                        }

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

            Some(transaction) = async {
                match &mut tx_stream {
                    Some(stream) => stream.next().await,
                    None => None,
                }
            } => {
                let msg = ChannelMsg::Transactions {
                    data: vec![transaction],
                };
                println!("Sending transactions: {:?}", msg);
                if let Ok(serialized) = serde_json::to_string(&msg) {
                    if sender.send(Message::Text(serialized.into())).await.is_err() {
                        break;
                    }
                }
            }


        }
    }
}

// A stream that generates transactions
fn stream_transactions() -> Pin<Box<dyn Stream<Item = Transaction> + Send>> {
    let stream = futures::stream::unfold((), |()| async {
        tokio::time::sleep(Duration::from_millis(100)).await;

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
        Some((transaction, ()))
    });

    Box::pin(stream)
}
