use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::server::models::BenchmarkRecord;
use crate::server::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/benchmarks", axum::routing::get(list).post(create))
        .route(
            "/api/benchmarks/{id}",
            axum::routing::delete(delete_benchmark),
        )
        .route(
            "/api/benchmarks/{id}/set-default",
            axum::routing::post(set_default),
        )
}

#[derive(Deserialize)]
pub struct CreateBenchmarkRequest {
    pub backend: Option<String>,
    pub device: Option<String>,
    pub prefix_length: Option<u32>,
    pub target_prefix: String,
    pub attempts: u64,
    pub elapsed_seconds: f64,
    pub keys_per_second: f64,
    pub found: bool,
    pub timeout_seconds: Option<u64>,
}

async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateBenchmarkRequest>,
) -> Result<Json<BenchmarkRecord>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let cfg = crate::cpu::CpuConfig::detect();
    let settings = crate::server::db::load_settings(&db);

    let bm = BenchmarkRecord {
        id: Uuid::new_v4().to_string(),
        created_at: chrono_now(),
        backend: req.backend.unwrap_or_else(|| "cpu".to_string()),
        device: req.device.unwrap_or_default(),
        prefix_length: req.prefix_length.unwrap_or(6),
        target_prefix: req.target_prefix,
        attempts: req.attempts,
        elapsed_seconds: req.elapsed_seconds,
        keys_per_second: req.keys_per_second,
        found: req.found,
        timeout_seconds: req.timeout_seconds.unwrap_or(0),
        cpu_total_cores: cfg.total_logical_cores,
        cpu_reserved_cores: settings.reserved_cpu_cores,
        cpu_worker_threads: cfg.available_workers(),
        is_default: false,
    };

    crate::server::db::insert_benchmark(&db, &bm).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(bm))
}

async fn list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<BenchmarkRecord>>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let benchmarks =
        crate::server::db::list_benchmarks(&db).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(benchmarks))
}

async fn delete_benchmark(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> StatusCode {
    let db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    crate::server::db::delete_benchmark(&db, &id).ok();
    StatusCode::NO_CONTENT
}

async fn set_default(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> StatusCode {
    let db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    crate::server::db::set_default_benchmark(&db, &id).ok();
    StatusCode::NO_CONTENT
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}
