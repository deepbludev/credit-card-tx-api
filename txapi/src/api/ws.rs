use crate::core::prelude::*;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use futures::{
    sink::SinkExt,
    stream::{SplitSink, SplitStream, StreamExt},
};
use models::{ChannelMsg, WsMessage};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

/// The endpoint for the websocket API.
///
/// This function upgrades the websocket connection and handles the incoming
/// messages.
pub async fn endpoint(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle(socket, state))
}

/// Handles the incoming messages from the websocket.
///
/// This function splits the websocket into a sink and stream, and then
/// creates a channel for messages between the websocket and the server.
///
/// It then spawns two tasks to handle the reading and writing of messages.
async fn handle(socket: WebSocket, state: AppState) {
    let (sender, receiver) = socket.split();

    let client = Arc::new(Mutex::new(client::WsClient::default()));
    let sender = Arc::new(Mutex::new(sender));

    let read_task = tokio::spawn(read(receiver, client.clone()));
    let write_task = tokio::spawn(write(sender, client, state.clone()));

    tokio::pin!(read_task);
    tokio::pin!(write_task);

    tokio::select! {
        res = &mut read_task => {
            write_task.abort();
            match res {
                Ok(_) => info!("read task completed successfully."),
                Err(err) => error!("read task encountered an error: {:?}", err),
            }
        },
        res = &mut write_task => {
            read_task.abort();
            match res {
                Ok(_) => info!("write task completed successfully."),
                Err(err) => error!("write task encountered an error: {:?}", err),
            }
        },
    }
}

/// Read side of the websocket connection.
///
/// This function reads messages from the websocket and handles
/// the received messages.
async fn read(mut receiver: SplitStream<WebSocket>, client: Arc<Mutex<client::WsClient>>) {
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<WsMessage>(&text) {
                Err(e) => error!("Invalid message: {}", e),
                Ok(ws_msg) => {
                    let mut client = client.lock().await;
                    handle_incoming(&ws_msg, &mut client).await;
                }
            }
        }
    }
}

/// Write side of the websocket connection.
///
/// This function handles the writing of messages to the websocket. It streams
/// the data for each of the client's subscribed channels.
async fn write(
    sender: Arc<Mutex<SplitSink<WebSocket, Message>>>,
    client: Arc<Mutex<client::WsClient>>,
    state: AppState,
) {
    use client::Channel;

    // Create subscriptions for heartbeat and transactions channels.
    let mut heartbeat_rx = state.heartbeat_tx.subscribe();
    let mut transactions_rx = state.transactions_tx.subscribe();

    loop {
        tokio::select! {
            // heartbeat channel - all clients
            heartbeat = heartbeat_rx.recv() => {
                match heartbeat {
                    Err(_) => error!("Error receiving heartbeat from channel"),
                    Ok(heartbeat) => {
                        let mut sender = sender.lock().await;
                        send(&mut sender, ChannelMsg::Heartbeat { data: heartbeat }).await;
                    }
                }
            }

            // transactions channel
            transaction = transactions_rx.recv() => {
                match transaction {
                    Err(_) => error!("Error receiving transaction from channel"),
                    Ok(transaction) => {
                        let client = client.lock().await;
                        if client.is_subscribed(&Channel::Transactions) {
                            let mut sender = sender.lock().await;
                            send(&mut sender, ChannelMsg::Transactions { data: vec![transaction] }).await;
                        }
                    }
                }
            }
        }
    }
}

/// Handles the incoming messages from the websocket.
///
/// This function handles the incoming messages from the websocket and
/// returns the appropriate response.
async fn handle_incoming(msg: &WsMessage, client: &mut client::WsClient) {
    // handle the incoming message
    match msg {
        // subscribe to a channel
        WsMessage::Subscribe { params } => match params.channel.parse() {
            Err(e) => error!("Invalid channel: {}", e),
            Ok(channel) => {
                client.subscribe(channel);
                info!("Successfully subscribed to {} channel", params.channel)
            }
        },

        // unsubscribe from a channel
        WsMessage::Unsubscribe { params } => match params.channel.parse() {
            Err(e) => error!("Invalid channel: {}", e),
            Ok(channel) => {
                client.unsubscribe(channel);
                info!("Successfully unsubscribed from {} channel", params.channel)
            }
        },
    }
}

/// Sends a message by serializing the message and sending it to the websocket.
async fn send(tx: &mut SplitSink<WebSocket, Message>, msg: ChannelMsg) {
    if let Ok(serialized) = serde_json::to_string(&msg) {
        match tx.send(Message::Text(serialized.into())).await {
            Ok(_) => info!("sent message: {:?}", msg),
            Err(e) => error!("error sending message: {:?}", e),
        }
    }
}

/// Module for models for the websocket API.
///
/// This module includes the message types for the websocket API such as
/// subscribe, unsubscribe, and heartbeat messages.
mod models {
    use crate::domain::prelude::*;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize, Debug)]
    #[serde(tag = "method")]
    pub enum WsMessage {
        #[serde(rename = "subscribe")]
        Subscribe { params: SubscribeParams },
        #[serde(rename = "unsubscribe")]
        Unsubscribe { params: UnsubscribeParams },
    }

    #[derive(Deserialize, Serialize, Debug)]
    pub struct SubscribeParams {
        pub channel: String,
    }
    #[derive(Deserialize, Serialize, Debug)]
    pub struct UnsubscribeParams {
        pub channel: String,
    }

    #[derive(Deserialize, Serialize, Debug)]
    #[serde(tag = "channel")]
    pub enum ChannelMsg {
        #[serde(rename = "transactions")]
        Transactions { data: Vec<Transaction> },

        #[serde(rename = "heartbeat")]
        Heartbeat { data: Heartbeat },
    }
}

/// Module for the websocket client.
///
/// This module includes the client struct and methods for the websocket client
/// which handles the subscription and unsubscription to channels.
///
pub mod client {
    use std::collections::HashSet;

    /// Channel enum for the websocket client.
    ///
    /// This enum contains the available channels for the websocket client, such as
    /// the transactions channel.
    ///
    #[derive(Debug, PartialEq, Eq, Hash, Clone)]
    pub enum Channel {
        Heartbeat,
        Transactions,
    }

    impl std::str::FromStr for Channel {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "heartbeat" => Ok(Self::Heartbeat),
                "transactions" => Ok(Self::Transactions),
                _ => Err(format!("Invalid channel: {}", s)),
            }
        }
    }

    /// The websocket client struct.
    ///
    /// This struct handles the subscription and unsubscription to channels
    /// for a given websocket connection.
    #[derive(Debug, Default)]
    pub struct WsClient {
        pub channels: HashSet<Channel>,
    }

    impl WsClient {
        /// Subscribes to a websocket channel.
        pub fn subscribe(&mut self, channel: Channel) -> &Self {
            self.channels.insert(channel.clone());
            self
        }

        /// Unsubscribes from a websocket channel.
        pub fn unsubscribe(&mut self, channel: Channel) -> &Self {
            self.channels.remove(&channel);
            self
        }

        /// Checks if the client is subscribed to a given channel.
        pub fn is_subscribed(&self, channel: &Channel) -> bool {
            self.channels.contains(channel)
        }
    }
}
