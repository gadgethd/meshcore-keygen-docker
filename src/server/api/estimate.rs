use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json, Router,
};
use serde::Deserialize;

use crate::server::models::EstimateResponse;
use crate::server::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/estimate", axum::routing::post(estimate))
        .route("/api/system/cpu", axum::routing::get(cpu_info))
}

#[derive(Deserialize)]
pub struct EstimateRequest {
    pub prefixes: Vec<String>,
    pub backend: Option<String>,
}

async fn estimate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EstimateRequest>,
) -> Result<Json<EstimateResponse>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if req.prefixes.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let min_len = req.prefixes.iter().map(|p| p.len()).min().unwrap();
    let expected_attempts: f64 = 16_f64.powi(min_len as i32);

    let backend = req.backend.unwrap_or_else(|| "cpu".to_string());

    // Get benchmark keys/s if available
    let (keys_per_second, device, benchmark_id) =
        match crate::server::db::get_default_benchmark(&db) {
            Ok(Some(bm)) => (bm.keys_per_second, bm.device, Some(bm.id)),
            _ => {
                // Fallback: use a rough CPU estimate
                let cpu_cores = crate::server::db::load_settings(&db).reserved_cpu_cores;
                let threads = std::thread::available_parallelism()
                    .map(|n| n.get().saturating_sub(cpu_cores).max(1))
                    .unwrap_or(1);
                (
                    (threads as f64) * 80_000.0,
                    "cpu (estimated)".to_string(),
                    None,
                )
            }
        };

    let estimated_seconds = expected_attempts / keys_per_second;

    Ok(Json(EstimateResponse {
        prefix_length: min_len,
        expected_attempts,
        keys_per_second,
        estimated_seconds,
        milestone_50pct_seconds: milestone(0.50, expected_attempts, keys_per_second),
        milestone_90pct_seconds: milestone(0.90, expected_attempts, keys_per_second),
        milestone_95pct_seconds: milestone(0.95, expected_attempts, keys_per_second),
        milestone_99pct_seconds: milestone(0.99, expected_attempts, keys_per_second),
        backend,
        device,
        benchmark_id,
        benchmark_age: None,
    }))
}

fn milestone(p: f64, expected: f64, kps: f64) -> f64 {
    let attempts = -(1.0 - p).ln() * expected;
    attempts / kps
}

async fn cpu_info(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let cfg = crate::cpu::CpuConfig::detect();
    let settings = crate::server::db::load_settings(&db);
    Ok(Json(serde_json::json!({
        "total_logical_cores": cfg.total_logical_cores,
        "reserved_cores": settings.reserved_cpu_cores,
        "available_workers": cfg.available_workers(),
        "max_worker_threads": settings.max_worker_threads,
    })))
}
