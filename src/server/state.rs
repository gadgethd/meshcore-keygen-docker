use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use super::db::DbPool;

/// Live progress for an active benchmark.
#[derive(Clone, Debug, Serialize)]
pub struct ActiveBenchmark {
    pub id: String,
    pub target_prefix: String,
    pub prefix_length: u32,
    pub attempts: u64,
    pub keys_per_second: f64,
    pub elapsed_seconds: f64,
    pub backend: String,
    pub found: bool,
}

/// Application state shared across all API handlers.
pub struct AppState {
    pub db: DbPool,
    pub shutdown_tx: broadcast::Sender<()>,
    pub active_benchmark: Arc<Mutex<Option<ActiveBenchmark>>>,
    pub cancel_benchmark: Arc<AtomicBool>,
    pub active_job_cancel: Arc<AtomicBool>,
    pub active_job_id: Arc<Mutex<Option<String>>>,
}

impl AppState {
    pub fn new(db: DbPool) -> (Arc<Self>, broadcast::Receiver<()>) {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        (
            Arc::new(AppState {
                db,
                shutdown_tx,
                active_benchmark: Arc::new(Mutex::new(None)),
                cancel_benchmark: Arc::new(AtomicBool::new(false)),
                active_job_cancel: Arc::new(AtomicBool::new(false)),
                active_job_id: Arc::new(Mutex::new(None)),
            }),
            shutdown_rx,
        )
    }

    /// Send a shutdown signal to all connected clients.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }
}
