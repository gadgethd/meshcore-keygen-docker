use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json, Router,
};
use rand::Rng;
use serde::Deserialize;
use uuid::Uuid;

use crate::deterministic::DeterministicState;
use crate::search::SearchHandle;
use crate::server::models::BenchmarkRecord;
use crate::server::state::{ActiveBenchmark, AppState};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/benchmarks", axum::routing::get(list).post(create))
        .route("/api/benchmarks/active", axum::routing::get(active))
        .route("/api/benchmarks/run", axum::routing::post(run))
        .route("/api/benchmarks/stop", axum::routing::post(stop))
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

#[derive(Deserialize)]
pub struct RunBenchmarkRequest {
    pub backend: Option<String>,
    pub prefix_length: Option<u32>,
    pub timeout_seconds: Option<u64>,
}

async fn run(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunBenchmarkRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Check if benchmark already running
    {
        let active = state
            .active_benchmark
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if active.is_some() {
            return Err(StatusCode::CONFLICT);
        }
    }

    let backend = req.backend.unwrap_or_else(|| "cpu".to_string());
    let prefix_length = req.prefix_length.unwrap_or(6).clamp(2, 16);

    // Generate random hex prefix
    let hex_chars: &[u8] = b"0123456789ABCDEF";
    let mut rng = rand::thread_rng();
    let target_prefix: String = (0..prefix_length)
        .map(|_| hex_chars[rng.gen_range(0..16) as usize] as char)
        .collect();

    let id = Uuid::new_v4().to_string();
    let id_clone = id.clone();
    let target_clone = target_prefix.clone();

    let state_clone = state.clone();
    let active = state.active_benchmark.clone();
    let cancel = state.cancel_benchmark.clone();

    // Reset cancel flag
    cancel.store(false, std::sync::atomic::Ordering::SeqCst);

    // Set initial active state
    {
        let mut lock = active.lock().unwrap_or_else(|e| e.into_inner());
        *lock = Some(ActiveBenchmark {
            id: id_clone.clone(),
            target_prefix: target_clone.clone(),
            prefix_length,
            attempts: 0,
            keys_per_second: 0.0,
            elapsed_seconds: 0.0,
            backend: backend.clone(),
            found: false,
        });
    }

    tokio::task::spawn_blocking(move || {
        run_benchmark_sync(
            state_clone,
            active,
            cancel,
            id_clone,
            target_clone,
            prefix_length,
            backend,
        );
    });

    Ok(Json(serde_json::json!({
        "id": id,
        "target_prefix": target_prefix,
        "prefix_length": prefix_length,
        "status": "running"
    })))
}

fn run_benchmark_sync(
    state: Arc<AppState>,
    active: Arc<Mutex<Option<ActiveBenchmark>>>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    id: String,
    target_prefix: String,
    prefix_length: u32,
    backend: String,
) {
    let prefixes = vec![target_prefix.clone()];

    // Try GPU if CUDA is requested and available
    let using_gpu = backend == "cuda";
    let gpu_searchers: Vec<Box<dyn crate::search::GpuSearcher>> = if using_gpu {
        #[cfg(feature = "cuda")]
        {
            crate::gpu::try_init_gpu(&prefixes)
        }
        #[cfg(not(feature = "cuda"))]
        {
            vec![]
        }
    } else {
        vec![]
    };

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1))
        .unwrap_or(1);

    let has_gpu = !gpu_searchers.is_empty();
    let handle = if has_gpu {
        crate::search::SearchHandle::start_hybrid(&prefixes, num_threads, gpu_searchers)
    } else {
        let deterministic_state = crate::deterministic::DeterministicState::new();
        crate::search::SearchHandle::start_deterministic(
            &prefixes,
            deterministic_state,
            num_threads,
            None,
            10,
            false,
        )
    };

    let actual_backend = if has_gpu {
        backend.clone()
    } else {
        "cpu".to_string()
    };

    let start = Instant::now();
    let mut last_update = Instant::now();

    loop {
        if handle.is_done() {
            break;
        }
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));

        if last_update.elapsed() >= std::time::Duration::from_millis(500) {
            last_update = Instant::now();
            let stats = handle.stats(0);
            let mut lock = active.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(ref mut a) = *lock {
                a.attempts = stats.attempts;
                a.keys_per_second = stats.keys_per_sec;
                a.elapsed_seconds = stats.elapsed_secs;
                a.backend = actual_backend.clone();
            }
        }
    }

    let elapsed = start.elapsed();
    let stats = handle.stats(0);
    let found = handle.is_done();
    let kps = if elapsed.as_secs_f64() > 0.0 {
        stats.attempts as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };

    {
        let mut lock = active.lock().unwrap_or_else(|e| e.into_inner());
        *lock = None;
    }

    {
        let db = state.db.lock().unwrap_or_else(|e| e.into_inner());
        let cfg = crate::cpu::CpuConfig::detect();
        let settings = crate::server::db::load_settings(&db);
        let bm = BenchmarkRecord {
            id: id.clone(),
            created_at: chrono_now(),
            backend: actual_backend,
            device: if has_gpu && using_gpu {
                #[cfg(feature = "cuda")]
                {
                    crate::gpu::detect_cuda().1.unwrap_or_default()
                }
                #[cfg(not(feature = "cuda"))]
                {
                    String::new()
                }
            } else {
                String::new()
            },
            prefix_length,
            target_prefix,
            attempts: stats.attempts,
            elapsed_seconds: elapsed.as_secs_f64(),
            keys_per_second: kps,
            found,
            timeout_seconds: 0,
            cpu_total_cores: cfg.total_logical_cores,
            cpu_reserved_cores: settings.reserved_cpu_cores,
            cpu_worker_threads: cfg.available_workers(),
            is_default: false,
        };
        let _ = crate::server::db::insert_benchmark(&db, &bm);
    }

    let _ = handle.finish();
}

async fn stop(State(state): State<Arc<AppState>>) -> StatusCode {
    state
        .cancel_benchmark
        .store(true, std::sync::atomic::Ordering::SeqCst);
    StatusCode::OK
}

async fn active(State(state): State<Arc<AppState>>) -> Json<Option<ActiveBenchmark>> {
    let lock = state
        .active_benchmark
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    Json(lock.clone())
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
