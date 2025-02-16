use crate::domain::prelude::*;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub broadcaster_tx: broadcast::Sender<Tx>,
}
