use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    Router,
};
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;

use crate::server::models::JobStatus;
use crate::server::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/ws", axum::routing::get(ws_handler))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut _receiver) = socket.split();
    let mut shutdown_rx = state.shutdown_tx.subscribe();

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                let _ = sender.send(Message::Text("{\"type\":\"shutdown\"}".into())).await;
                break;
            }
            _ = tokio::time::sleep(Duration::from_secs(1)) => {
                // Send status update every second
                let update = get_status_update(&state);
                if let Ok(json) = serde_json::to_string(&update) {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

fn get_status_update(state: &AppState) -> serde_json::Value {
    let db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let jobs = crate::server::db::list_jobs(&db).unwrap_or_default();
    let active_job = jobs
        .iter()
        .find(|j| j.status == crate::server::models::JobStatus::Running);
    let queue_len = jobs
        .iter()
        .filter(|j| j.status == crate::server::models::JobStatus::Queued)
        .count();
    let cfg = crate::cpu::CpuConfig::detect();
    let last_bm = crate::server::db::get_default_benchmark(&db).ok().flatten();

    serde_json::json!({
        "type": "status_update",
        "cpu_total_cores": cfg.total_logical_cores,
        "cpu_available_workers": cfg.available_workers(),
        "queue_length": queue_len,
        "last_benchmark_keys_per_second": last_bm.as_ref().map(|b| b.keys_per_second),
        "active_job": active_job.map(|j| serde_json::json!({
            "id": j.id,
            "prefixes": j.prefixes,
            "status": j.status.as_str(),
            "attempts_done": j.attempts_done,
            "keys_per_second": j.keys_per_second,
            "elapsed_seconds": j.elapsed_seconds,
            "backend": j.backend,
            "device": j.device,
            "cpu_worker_threads": j.cpu_worker_threads,
        })),
    })
}
