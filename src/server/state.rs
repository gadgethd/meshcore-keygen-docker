use std::sync::Arc;
use tokio::sync::broadcast;

use super::db::DbPool;

/// Application state shared across all API handlers.
pub struct AppState {
    pub db: DbPool,
    pub shutdown_tx: broadcast::Sender<()>,
}

impl AppState {
    pub fn new(db: DbPool) -> (Arc<Self>, broadcast::Receiver<()>) {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        (Arc::new(AppState { db, shutdown_tx }), shutdown_rx)
    }

    /// Send a shutdown signal to all connected clients.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }
}
