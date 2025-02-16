use crate::domain::prelude::*;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    /// The sender for the transactions channel.
    /// Used to broadcast transactions to the websocket clients.
    pub transactions_tx: broadcast::Sender<Transaction>,
}
