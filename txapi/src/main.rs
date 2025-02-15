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
    stream::{select_all, SplitSink, SplitStream, StreamExt},
    Stream,
};
use std::time::Duration;
use tokio::sync::broadcast;
use txapi::models::{ChannelMsg, Heartbeat, Transaction, WsMessage};

#[derive(Clone)]
struct AppState {
    broadcaster_tx: broadcast::Sender<Transaction>,
}

#[tokio::main]
async fn main() {
    let app_state = AppState {
        broadcaster_tx: init_broadcaster().await,
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
    let (ws_sink, ws_stream) = socket.split();

    // channel for messages between the websocket and the server
    let (msg_tx, msg_rx) = tokio::sync::mpsc::channel(1);

    let write_task = tokio::spawn(write(ws_sink, msg_rx, state));
    let read_task = tokio::spawn(read(ws_stream, msg_tx));

    tokio::select! {
        _ = write_task => {},
        _ = read_task => {},
    }
}

async fn init_broadcaster() -> broadcast::Sender<Transaction> {
    let buffer_size = 100;
    let buffer_size = std::env::var("BROADCAST_BUFFER_SIZE")
        .map(|s| s.parse::<usize>().unwrap_or(buffer_size))
        .unwrap_or(buffer_size);

    let (broadcaster_tx, _) = broadcast::channel(buffer_size);
    let broadcaster_tx_clone = broadcaster_tx.clone();

    let mock_tx_stream = stream_tx_from_mocks();
    // add more streams here (ex. kafka, mongodb, etc.)

    // combine all streams into one
    let mut broadcast_stream = select_all(vec![
        Box::pin(mock_tx_stream),
        // add other streams here...
    ]);

    // apawn the transaction stream processor
    tokio::spawn(async move {
        while let Some(transaction) = broadcast_stream.next().await {
            // ignore send errors (occurs when no receivers)
            // TODO: handle this gracefully
            let _ = broadcaster_tx_clone.send(transaction);
        }
    });
    broadcaster_tx
}

async fn read(mut ws_stream: SplitStream<WebSocket>, msg_tx: tokio::sync::mpsc::Sender<WsMessage>) {
    while let Some(Ok(msg)) = ws_stream.next().await {
        if let Message::Text(text) = msg {
            println!("Received: {}", text);

            match serde_json::from_str::<WsMessage>(&text) {
                Ok(request) => {
                    let _ = msg_tx.send(request).await;
                }
                Err(_) => {
                    println!("Failed to parse message: {}", text);
                }
            }
        }
    }
}

async fn write(
    mut ws_sink: SplitSink<WebSocket, Message>,
    mut msg_rx: tokio::sync::mpsc::Receiver<WsMessage>,
    state: AppState,
) {
    // store the channel subscriptions in-memory
    let mut subscriptions: Vec<String> = Vec::new();

    // receiver for the transaction stream
    let mut transaction_rx: Option<broadcast::Receiver<Transaction>> = None;

    loop {
        tokio::select! {
            // handle incoming messages
            Some(request) = msg_rx.recv() => {
                match request {
                    // subscribe to a channel
                    WsMessage::Subscribe { params } => {
                        if !subscriptions.contains(&params.channel) {
                            subscriptions.push(params.channel.clone());

                            match params.channel.as_str() {
                                "transactions" => {
                                    // subscribe to the broadcast channel
                                    transaction_rx = Some(state.broadcaster_tx.subscribe());
                                }
                                "heartbeat" => {
                                    // nothing to do here
                                }
                                _ => {}
                            }
                        }
                        // send ack
                        let heartbeat = Heartbeat { status:format!("Successfully subscribed to {} channel", params.channel) };
                        let ack = ChannelMsg::Heartbeat { data: heartbeat };

                        // send the ack message to the client
                        if let Ok(serialized) = serde_json::to_string(&ack) {
                            let msg = Message::Text(serialized.into());
                            if ws_sink.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }

                    // unsubscribe from a channel
                    WsMessage::Unsubscribe { params } => {
                        subscriptions.retain(|c| c != &params.channel);
                        match params.channel.as_str() {
                            "transactions" => {
                                // unsubscribe from the broadcast channel
                                transaction_rx = None;
                            }
                            "heartbeat" => {
                                // nothing to do here
                            }
                            _ => {}
                        }

                        let heartbeat = Heartbeat { status:format!("Successfully unsubscribed from {} channel", params.channel) };
                        let ack = ChannelMsg::Heartbeat { data: heartbeat };

                        if let Ok(serialized) = serde_json::to_string(&ack) {
                            let msg = Message::Text(serialized.into());
                            if ws_sink.send(msg).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }

            result = async {
                match transaction_rx.as_mut() {
                    Some(rx) => rx.recv().await.ok(),
                    None => None
                }
            } => {
                if let Some(transaction) = result {
                    let msg = ChannelMsg::Transactions { data: vec![transaction] };
                    if let Ok(serialized) = serde_json::to_string(&msg) {
                        let msg = Message::Text(serialized.into());
                        if ws_sink.send(msg).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// A stream that generates mock transactions
fn stream_tx_from_mocks() -> impl Stream<Item = Transaction> + Send {
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
