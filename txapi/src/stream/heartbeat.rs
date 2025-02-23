use futures::{Stream, StreamExt};
use std::time::Duration;
use tokio::sync::broadcast;

use crate::domain::prelude::*;

/// Initialize the heartbeat channel.
/// This channel is used to broadcast heartbeats to the websocket clients.
///
/// This initializer is meant to be used to create a broadcaster at App State level,
/// in order to make it available to the websocket handler.
///
pub async fn channel() -> (broadcast::Sender<Heartbeat>, broadcast::Receiver<Heartbeat>) {
    let (tx, rx) = broadcast::channel(16);
    let tx_clone = tx.clone();

    let mut stream = stream_heartbeats_every_10_secs();
    tokio::spawn(async move {
        while let Some(heartbeat) = stream.next().await {
            let _ = tx_clone.send(heartbeat);
        }
    });
    (tx, rx)
}

/// A stream that generates heartbeats every 10 seconds
///
fn stream_heartbeats_every_10_secs() -> impl Stream<Item = Heartbeat> + Send {
    let stream = futures::stream::unfold((), |()| async {
        tokio::time::sleep(Duration::from_secs(10)).await;
        let heartbeat = Heartbeat {
            status: "ok".to_string(),
        };
        Some((heartbeat, ()))
    });

    Box::pin(stream)
}
