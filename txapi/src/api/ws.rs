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
use tokio::sync::broadcast;

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
mod client {
    use std::collections::HashSet;

    use crate::core::prelude::*;
    use crate::domain::prelude::*;

    use tokio::sync::broadcast;

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
    #[derive(Debug)]
    pub struct WsClient {
        pub channels: HashSet<Channel>,
        pub transaction_rx: Option<broadcast::Receiver<Transaction>>,
    }

    impl WsClient {
        /// Creates a new websocket client with no channel subscriptions.
        ///
        pub fn new() -> Self {
            Self {
                channels: HashSet::new(),
                transaction_rx: None,
            }
        }

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
        pub fn subscribe(&mut self, channel: Channel, app_state: &AppState) -> &Self {
            self.channels.insert(channel.clone());

            match channel {
                Channel::Transactions => {
                    self.transaction_rx = Some(app_state.transactions_tx.subscribe());
                }
                Channel::Heartbeat => {}
            };
            self
        }
        pub fn unsubscribe(&mut self, channel: Channel) -> &Self {
            self.channels.remove(&channel);
            match channel {
                Channel::Transactions => {
                    self.transaction_rx = None;
                }
                Channel::Heartbeat => {}
            };
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
    async fn handle(socket: WebSocket, state: AppState) {
        let (ws_sink, ws_stream) = socket.split();

        // channel for messages between the websocket and the server
        let (ws_tx, ws_rx) = tokio::sync::mpsc::channel(1);

        let write_task = tokio::spawn(write(ws_sink, ws_rx, state));
        let read_task = tokio::spawn(read(ws_stream, ws_tx));

        tokio::select! {
            _ = write_task => {},
            _ = read_task => {},
        }
    }

    // upgrade the websocket connection using the ws handler
    ws.on_upgrade(|socket| handle(socket, state))
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
async fn read(mut ws_stream: SplitStream<WebSocket>, ws_tx: tokio::sync::mpsc::Sender<WsMessage>) {
    while let Some(Ok(msg)) = ws_stream.next().await {
        if let Message::Text(text) = msg {
            // TODO: add proper logging / tracing

            match serde_json::from_str::<WsMessage>(&text) {
                Ok(request) => {
                    let _ = ws_tx.send(request).await;
                }
                Err(_) => {
                    // TODO: handle this gracefully
                }
            }
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
    mut ws_sink: SplitSink<WebSocket, Message>,
    mut ws_rx: tokio::sync::mpsc::Receiver<WsMessage>,
    state: AppState,
) {
    let mut client = client::WsClient::new();

    loop {
        // branch out the incoming message handling and message sending concurrently
        tokio::select! {
            // branch 1: handle incoming messages
            Some(msg) = ws_rx.recv() => {
                let response = handle_message(&msg, &mut client, &state).await;
                send_message(&response, &mut ws_sink).await;
            }

            // branch 2: send transactions from the transactions channel
            Some(transaction) = recv_from_channel(client.transaction_rx.as_mut()) => {
                let transactions = vec![transaction];
                let msg = ChannelMsg::Transactions { data: transactions };
                send_message(&msg, &mut ws_sink).await;

                // TODO: consider sending multiple transactions at once by batching
            }
        }
    }

    /// Handles the incoming messages from the websocket.
    ///
    /// This function handles the incoming messages from the websocket and
    /// returns the appropriate response.
    ///
    async fn handle_message(
        msg: &WsMessage,
        client: &mut client::WsClient,
        state: &AppState,
    ) -> ChannelMsg {
        // handle the incoming message

        let status = match msg {
            // subscribe to a channel
            WsMessage::Subscribe { params } => match params.channel.parse().ok() {
                None => format!("Invalid channel: {}", params.channel),
                Some(channel) => {
                    client.subscribe(channel, state);
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
    /// * `ws_sink` - The websocket sink to send the message to.
    ///
    async fn send_message(msg: &ChannelMsg, ws_sink: &mut SplitSink<WebSocket, Message>) {
        if let Ok(serialized) = serde_json::to_string(&msg) {
            match ws_sink.send(Message::Text(serialized.into())).await {
                Ok(_) => {
                    // TODO: add logging
                }
                Err(_) => {
                    // TODO: handle this gracefully
                }
            }
        }
    }

    /// Receives a message from a given channel stream, such as the transactions channel.
    ///
    /// # Arguments
    ///
    /// * `channel_rx` - The channel receiver to receive the message from.
    ///
    /// # Returns
    ///
    /// An `Option<T>` containing the received message if successful, or `None` if the channel is not subscribed
    /// or if receiving fails.
    async fn recv_from_channel<T: Clone>(
        channel_rx: Option<&mut broadcast::Receiver<T>>,
    ) -> Option<T> {
        match channel_rx {
            // receive transactions from the transactions channel if subscribed
            Some(channel_rx) => channel_rx.recv().await.ok(),
            None => None,
        }
    }
}
