// use crate::{api::ws, domain::prelude::*};
use crate::domain::prelude::*;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    /// The sender for the transactions channel.
    /// Used to broadcast transactions to the websocket clients.
    pub transactions_tx: broadcast::Sender<Transaction>,
    // pub clients: HashSet<ws::client::WsClient>,
}

impl AppState {
    pub fn new(transactions_tx: broadcast::Sender<Transaction>) -> Self {
        Self {
            transactions_tx,
            // clients: HashSet::new(),
        }
    }
}
