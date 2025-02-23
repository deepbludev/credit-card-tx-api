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
use models::{ChannelMsg, Heartbeat, WsMessage};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Module for models for the websocket API.
///
/// This module includes the message types for the websocket API such as
/// subscribe, unsubscribe, and heartbeat messages.
///
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

    #[derive(Deserialize, Serialize, Debug)]
    pub struct Heartbeat {
        pub status: String,
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
        Transactions,
        Heartbeat,
    }

    impl std::str::FromStr for Channel {
        type Err = ();

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "transactions" => Ok(Self::Transactions),
                "heartbeat" => Ok(Self::Heartbeat),
                _ => Err(()),
            }
        }
    }

    /// The websocket client struct.
    ///
    /// This struct handles the subscription and unsubscription to channels
    /// for a given websocket connection.
    ///
    #[derive(Debug, Default)]
    pub struct WsClient {
        pub channels: HashSet<Channel>,
    }

    impl WsClient {
        /// Subscribes to a websocket channel.
        ///
        /// This functions updates the list of subscribed channels and the receiver
        /// for the transactions channel.
        ///
        /// # Arguments
        ///
        /// * `channel` - The channel to subscribe to (ex. "transactions")
        /// * `app_state` - The application state to get the channel senders from.
        ///
        ///
        pub fn subscribe(&mut self, channel: Channel) -> &Self {
            self.channels.insert(channel.clone());
            self
        }

        /// Unsubscribes from a websocket channel.
        ///
        /// This functions updates the list of subscribed channels and the receiver
        /// for the transactions channel.
        ///
        pub fn unsubscribe(&mut self, channel: Channel) -> &Self {
            self.channels.remove(&channel);

            self
        }
    }
}

/// The endpoint for the websocket API.
///
/// This function upgrades the websocket connection and handles the incoming
/// messages.
///
pub async fn endpoint(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    /// Handles the incoming messages from the websocket.
    ///
    /// This function splits the websocket into a sink and stream, and then
    /// creates a channel for messages between the websocket and the server.
    ///
    /// It then spawns two tasks to handle the reading and writing of messages.
    ///
    async fn handle(socket: WebSocket, state: AppState) {
        let (sender, receiver) = socket.split();
        let client = Arc::new(RwLock::new(client::WsClient::default()));

        let read_task = tokio::spawn(read(receiver, client.clone()));
        let write_task = tokio::spawn(write(sender, client, state.clone()));

        tokio::select! {
            _ = read_task => {},
            _ = write_task => {},
        }
    }
    // upgrade the websocket connection using the ws handler
    ws.on_upgrade(move |socket| handle(socket, state))
}

/// Read side of the websocket connection.
///
/// This function reads messages from the websocket and sends them to the server
/// in a loop using the given mpsc channel sender.
///
/// # Arguments
///
/// * `ws_stream` - The websocket stream to read messages from.
/// * `ws_tx` - The mpsc channel sender to send messages to the server.
///
async fn read(mut receiver: SplitStream<WebSocket>, client: Arc<RwLock<client::WsClient>>) {
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            let ws_msg = serde_json::from_str::<WsMessage>(&text).unwrap();
            let mut client = client.write().await;
            handle_message(&ws_msg, &mut client).await;
        }
    }
}

/// Write side of the websocket connection.
///
/// This function handles the writing of messages to the websocket. It also
/// sends transactions from the transactions channel to the websocket.
///
/// # Arguments
///
/// * `ws_sink` - The websocket sink to write messages to.
/// * `ws_rx` - The mpsc channel receiver to receive messages from the websocket.
/// * `state` - The application state to get the channel senders from.
///
async fn write(
    mut sender: SplitSink<WebSocket, Message>,
    client: Arc<RwLock<client::WsClient>>,
    state: AppState,
) {
    let mut transactions_rx = state.transactions_tx.subscribe();

    while let Ok(transaction) = transactions_rx.recv().await {
        let client = client.read().await;
        if client.channels.contains(&client::Channel::Transactions) {
            let msg = ChannelMsg::Transactions {
                data: vec![transaction],
            };
            send_message(&msg, &mut sender).await;
        }
    }
}

/// Handles the incoming messages from the websocket.
///
/// This function handles the incoming messages from the websocket and
/// returns the appropriate response.
///
async fn handle_message(msg: &WsMessage, client: &mut client::WsClient) -> ChannelMsg {
    // handle the incoming message
    let status = match msg {
        // subscribe to a channel
        WsMessage::Subscribe { params } => match params.channel.parse().ok() {
            None => format!("Invalid channel: {}", params.channel),
            Some(channel) => {
                client.subscribe(channel);
                format!("Successfully subscribed to {} channel", params.channel)
            }
        },

        // unsubscribe from a channel
        WsMessage::Unsubscribe { params } => match params.channel.parse().ok() {
            None => format!("Invalid channel: {}", params.channel),
            Some(channel) => {
                client.unsubscribe(channel);
                format!("Successfully unsubscribed from {} channel", params.channel)
            }
        },
    };

    // send the status back to the client
    ChannelMsg::Heartbeat {
        data: Heartbeat { status },
    }
}

/// Sends a message by serializing the message and sending it to the websocket.
///
/// # Arguments
///
/// * `msg` - The message to send to the websocket.
/// * `sender` - The websocket sender to send the message to.
///
async fn send_message(msg: &ChannelMsg, sender: &mut SplitSink<WebSocket, Message>) {
    if let Ok(serialized) = serde_json::to_string(&msg) {
        match sender.send(Message::Text(serialized.into())).await {
            Ok(_) => {
                // TODO: add logging
            }
            Err(_) => {
                // TODO: handle this gracefully
            }
        }
    }
}
