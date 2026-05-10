use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::server::models::*;
use crate::server::state::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/jobs", axum::routing::get(list).post(create))
        .route(
            "/api/jobs/{id}",
            axum::routing::get(get_one).patch(update).delete(delete),
        )
        .route("/api/jobs/{id}/pause", axum::routing::post(pause))
        .route("/api/jobs/{id}/resume", axum::routing::post(resume))
        .route("/api/jobs/{id}/stop", axum::routing::post(stop))
        .route("/api/jobs/{id}/restart", axum::routing::post(restart))
        .route("/api/jobs/{id}/duplicate", axum::routing::post(duplicate))
}

#[derive(Deserialize)]
pub struct CreateJobRequest {
    pub name: Option<String>,
    pub prefixes: Vec<String>,
    pub backend: Option<String>,
    pub max_attempts: Option<u64>,
    pub max_runtime: Option<u64>,
    pub notes: Option<String>,
}

async fn create(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateJobRequest>,
) -> Result<Json<Job>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let settings = crate::server::db::load_settings(&db);

    let job = Job {
        id: Uuid::new_v4().to_string(),
        name: req.name.unwrap_or_default(),
        prefixes: req.prefixes,
        backend: req.backend.unwrap_or(settings.default_backend),
        device: String::new(),
        status: JobStatus::Queued,
        priority: 0,
        created_at: now_str(),
        updated_at: now_str(),
        master_seed: None,
        next_counter: None,
        attempts_done: 0,
        keys_per_second: 0.0,
        elapsed_seconds: 0.0,
        cpu_reserved_cores: settings.reserved_cpu_cores,
        cpu_worker_threads: 0,
        max_attempts: req.max_attempts,
        max_runtime: req.max_runtime,
        schedule_enabled: false,
        schedule_start: None,
        schedule_end: None,
        notes: req.notes,
    };

    crate::server::db::insert_job(&db, &job).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(job))
}

async fn list(State(state): State<Arc<AppState>>) -> Result<Json<Vec<Job>>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let jobs = crate::server::db::list_jobs(&db).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(jobs))
}

async fn get_one(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Job>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let job = crate::server::db::get_job(&db, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(job))
}

#[derive(Deserialize)]
pub struct UpdateJobRequest {
    pub name: Option<String>,
    pub priority: Option<i32>,
    pub status: Option<String>,
    pub max_attempts: Option<u64>,
    pub max_runtime: Option<u64>,
    pub notes: Option<String>,
    pub schedule_enabled: Option<bool>,
    pub schedule_start: Option<String>,
    pub schedule_end: Option<String>,
}

async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateJobRequest>,
) -> Result<Json<Job>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut job = crate::server::db::get_job(&db, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if let Some(name) = req.name {
        job.name = name;
    }
    if let Some(priority) = req.priority {
        job.priority = priority;
    }
    if let Some(status) = req.status {
        job.status = match status.as_str() {
            "queued" => JobStatus::Queued,
            "running" => JobStatus::Running,
            "paused" => JobStatus::Paused,
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            "stopped" => JobStatus::Stopped,
            _ => job.status,
        };
    }
    if let Some(ma) = req.max_attempts {
        job.max_attempts = Some(ma);
    }
    if let Some(mr) = req.max_runtime {
        job.max_runtime = Some(mr);
    }
    if let Some(notes) = req.notes {
        job.notes = Some(notes);
    }
    if let Some(enabled) = req.schedule_enabled {
        job.schedule_enabled = enabled;
    }
    if req.schedule_start.is_some() {
        job.schedule_start = req.schedule_start;
    }
    if req.schedule_end.is_some() {
        job.schedule_end = req.schedule_end;
    }
    job.updated_at = now_str();

    crate::server::db::update_job(&db, &job).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(job))
}

async fn delete(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> StatusCode {
    let db = state.db.lock().unwrap_or_else(|e| e.into_inner());
    crate::server::db::delete_job(&db, &id).ok();
    StatusCode::NO_CONTENT
}

async fn pause(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Job>, StatusCode> {
    // Signal the queue manager to cancel
    if let Ok(lock) = state.active_job_id.lock() {
        if lock.as_deref() == Some(&id) {
            state
                .active_job_cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut job = crate::server::db::get_job(&db, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    job.status = JobStatus::Paused;
    job.updated_at = now_str();
    crate::server::db::update_job(&db, &job).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(job))
}

async fn resume(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Job>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut job = crate::server::db::get_job(&db, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    job.status = JobStatus::Queued;
    job.updated_at = now_str();
    crate::server::db::update_job(&db, &job).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(job))
}

async fn stop(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Job>, StatusCode> {
    // Signal the queue manager to cancel
    if let Ok(lock) = state.active_job_id.lock() {
        if lock.as_deref() == Some(&id) {
            state
                .active_job_cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut job = crate::server::db::get_job(&db, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    job.status = JobStatus::Stopped;
    job.updated_at = now_str();
    crate::server::db::update_job(&db, &job).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(job))
}

async fn restart(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Job>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let job = crate::server::db::get_job(&db, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    // Reject restart if job is currently running
    if job.status == JobStatus::Running {
        return Err(StatusCode::CONFLICT);
    }
    let mut job = job;
    job.master_seed = None;
    job.next_counter = None;
    job.attempts_done = 0;
    job.keys_per_second = 0.0;
    job.elapsed_seconds = 0.0;
    job.status = JobStatus::Queued;
    job.updated_at = now_str();
    crate::server::db::update_job(&db, &job).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(job))
}

async fn duplicate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Job>, StatusCode> {
    let db = state
        .db
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut job = crate::server::db::get_job(&db, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    // New ID, reset state, new master seed
    job.id = Uuid::new_v4().to_string();
    job.name = format!("{} (copy)", job.name);
    job.status = JobStatus::Queued;
    job.master_seed = None;
    job.next_counter = None;
    job.attempts_done = 0;
    job.keys_per_second = 0.0;
    job.elapsed_seconds = 0.0;
    job.created_at = now_str();
    job.updated_at = now_str();
    crate::server::db::insert_job(&db, &job).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(job))
}

fn now_str() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}
