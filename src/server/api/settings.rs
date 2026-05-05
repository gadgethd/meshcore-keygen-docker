use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json, Router};
use serde::Deserialize;

use crate::server::models::{JobStatus, Settings};
use crate::server::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/settings",
            axum::routing::get(get_settings).patch(update_settings),
        )
        .route("/api/status", axum::routing::get(status))
        .route("/api/devices", axum::routing::get(devices))
}

fn detect_gpu() -> (bool, Option<String>) {
    #[cfg(feature = "cuda")]
    {
        match crate::gpu::detect_cuda() {
            (true, name) => return (true, name),
            (false, _) => {}
        }
    }
    #[cfg(feature = "metal")]
    {
        // Metal detection would go here
    }
    (false, None)
}

async fn get_settings(State(state): State<Arc<AppState>>) -> Result<Json<Settings>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let settings = crate::server::db::load_settings(&db);
    // Don't expose password hash
    Ok(Json(Settings {
        password_hash: None,
        ..settings
    }))
}

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    pub reserved_cpu_cores: Option<usize>,
    pub max_worker_threads: Option<usize>,
    pub checkpoint_interval_secs: Option<u64>,
    pub default_backend: Option<String>,
    pub default_benchmark_id: Option<String>,
    pub timezone: Option<String>,
    pub hide_secrets: Option<bool>,
    pub max_log_lines: Option<usize>,
    pub schedule_enabled: Option<bool>,
    pub schedule_start: Option<String>,
    pub schedule_end: Option<String>,
}

async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateSettingsRequest>,
) -> Result<Json<Settings>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(v) = req.reserved_cpu_cores {
        crate::server::db::save_setting(&db, "reserved_cpu_cores", &v.to_string()).ok();
    }
    if let Some(v) = req.max_worker_threads {
        crate::server::db::save_setting(&db, "max_worker_threads", &v.to_string()).ok();
    }
    if let Some(v) = req.checkpoint_interval_secs {
        crate::server::db::save_setting(&db, "checkpoint_interval_secs", &v.to_string()).ok();
    }
    if let Some(ref v) = req.default_backend {
        crate::server::db::save_setting(&db, "default_backend", v).ok();
    }
    if let Some(ref v) = req.default_benchmark_id {
        crate::server::db::save_setting(&db, "default_benchmark_id", v).ok();
    }
    if let Some(ref v) = req.timezone {
        crate::server::db::save_setting(&db, "timezone", v).ok();
    }
    if let Some(v) = req.hide_secrets {
        crate::server::db::save_setting(&db, "hide_secrets", &v.to_string()).ok();
    }
    if let Some(v) = req.max_log_lines {
        crate::server::db::save_setting(&db, "max_log_lines", &v.to_string()).ok();
    }
    if let Some(v) = req.schedule_enabled {
        crate::server::db::save_setting(&db, "schedule_enabled", &v.to_string()).ok();
    }
    if let Some(ref v) = req.schedule_start {
        crate::server::db::save_setting(&db, "schedule_start", v).ok();
    }
    if let Some(ref v) = req.schedule_end {
        crate::server::db::save_setting(&db, "schedule_end", v).ok();
    }

    let settings = crate::server::db::load_settings(&db);
    Ok(Json(Settings {
        password_hash: None,
        ..settings
    }))
}

async fn status(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let cfg = crate::cpu::CpuConfig::detect();
    let jobs = crate::server::db::list_jobs(&db).unwrap_or_default();
    let results = crate::server::db::list_results(&db).unwrap_or_default();
    let active_job = jobs
        .iter()
        .find(|j| j.status == JobStatus::Running)
        .cloned();
    let queue_len = jobs
        .iter()
        .filter(|j| j.status == JobStatus::Queued)
        .count();
    let last_bm = crate::server::db::get_default_benchmark(&db).ok().flatten();
    let settings = crate::server::db::load_settings(&db);

    // GPU detection
    let (gpu_available, gpu_name) = detect_gpu();

    Ok(Json(serde_json::json!({
        "cpu_total_cores": cfg.total_logical_cores,
        "cpu_reserved_cores": 1,
        "cpu_available_workers": cfg.available_workers(),
        "gpu_available": gpu_available,
        "gpu_name": gpu_name,
        "active_job": active_job,
        "queue_length": queue_len,
        "results_count": results.len(),
        "last_benchmark_keys_per_second": last_bm.map(|b| b.keys_per_second),
        "schedule_enabled": settings.schedule_enabled,
        "schedule_start": settings.schedule_start,
        "schedule_end": settings.schedule_end,
    })))
}

async fn devices(State(_state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut backends = vec![serde_json::json!({
        "name": "cpu",
        "type": "cpu",
        "available": true,
        "threads": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1),
    })];

    #[cfg(feature = "cuda")]
    {
        let (available, name) = crate::gpu::detect_cuda();
        backends.push(serde_json::json!({
            "name": "cuda",
            "type": "gpu",
            "available": available,
            "description": name.unwrap_or_else(|| "CUDA GPU".to_string()),
        }));
    }

    Json(serde_json::json!({ "backends": backends }))
}
