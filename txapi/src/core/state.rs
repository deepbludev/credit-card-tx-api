// use crate::{api::ws, domain::prelude::*};
use crate::domain::prelude::*;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    /// The sender for the heartbeat channel.
    /// Used to broadcast heartbeats to the websocket clients.
    pub heartbeat_tx: broadcast::Sender<Heartbeat>,

    /// The sender for the transactions channel.
    /// Used to broadcast transactions to the websocket clients.
    pub transactions_tx: broadcast::Sender<Transaction>,
}

impl AppState {
    pub fn new(
        heartbeat_tx: broadcast::Sender<Heartbeat>,
        transactions_tx: broadcast::Sender<Transaction>,
    ) -> Self {
        Self {
            transactions_tx,
            heartbeat_tx,
        }
    }
}
