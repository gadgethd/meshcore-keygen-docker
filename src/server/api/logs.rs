use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json, Router,
};
use serde::Deserialize;

use crate::server::db::DbPool;
use crate::server::models::LogEntry;
use crate::server::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/logs", axum::routing::get(list))
}

#[derive(Deserialize)]
pub struct LogsQuery {
    pub job_id: Option<String>,
    pub limit: Option<usize>,
}

async fn list(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LogsQuery>,
) -> Result<Json<Vec<LogEntry>>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let limit = q.limit.unwrap_or(200).min(1000);
    let entries = crate::server::db::list_logs(&db, limit, q.job_id.as_deref())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(entries))
}

/// Log a message (used internally by queue manager, server, etc.)
pub fn log(db: &DbPool, level: &str, job_id: Option<&str>, message: &str) {
    let entry = LogEntry {
        id: 0,
        timestamp: chrono_now(),
        level: level.to_string(),
        job_id: job_id.map(|s| s.to_string()),
        message: message.to_string(),
    };
    if let Ok(db) = db.lock() {
        let _ = crate::server::db::insert_log(&db, &entry);
    }
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}
