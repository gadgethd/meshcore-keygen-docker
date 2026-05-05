use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Mutex as TokioMutex;
use tokio::time::sleep;
use uuid::Uuid;

use crate::deterministic::DeterministicState;
use crate::search::SearchHandle;
use crate::server::api::logs;
use crate::server::db::{self as dbmod};
use crate::server::models::{JobStatus, ResultRecord};

/// Alias for the database pool type used by the queue manager.
type DbPool = Arc<Mutex<rusqlite::Connection>>;

pub struct QueueManager {
    db: DbPool,
    running: Arc<TokioMutex<bool>>,
}

impl QueueManager {
    pub fn new(db: DbPool) -> Self {
        QueueManager {
            db,
            running: Arc::new(TokioMutex::new(false)),
        }
    }

    pub async fn run(&self) {
        loop {
            sleep(Duration::from_secs(2)).await;

            let mut is_running = self.running.lock().await;
            if *is_running {
                continue;
            }

            let next_job = {
                let db = lock_db(&self.db);
                dbmod::get_next_queued_job(&db).ok().flatten()
            };

            if let Some(job) = next_job {
                // Check global schedule
                {
                    let db = lock_db(&self.db);
                    let settings = dbmod::load_settings(&db);
                    if settings.schedule_enabled && !is_in_window(&settings) {
                        // Outside allowed window, don't start
                        continue;
                    }
                }

                *is_running = true;
                drop(is_running);

                let db = self.db.clone();
                let running = self.running.clone();
                let job_id = job.id.clone();
                let prefixes = job.prefixes.clone();
                let backend = job.backend.clone();
                let max_attempts = job.max_attempts;
                let max_runtime = job.max_runtime;

                tokio::task::spawn_blocking(move || {
                    run_job_sync(
                        db,
                        running,
                        job_id,
                        prefixes,
                        backend,
                        max_attempts,
                        max_runtime,
                    );
                });
            }
        }
    }
}

/// Lock the DB, recovering from poison.
fn lock_db(db: &DbPool) -> std::sync::MutexGuard<rusqlite::Connection> {
    db.lock().unwrap_or_else(|e| e.into_inner())
}

fn mark_failed(db: &DbPool, job_id: &str, msg: &str) {
    let db = lock_db(db);
    if let Ok(Some(mut job)) = dbmod::get_job(&db, job_id) {
        job.status = JobStatus::Failed;
        job.notes = Some(msg.to_string());
        let _ = dbmod::update_job(&db, &job);
    }
}

fn run_job_sync(
    db: DbPool,
    running: Arc<TokioMutex<bool>>,
    job_id: String,
    prefixes: Vec<String>,
    backend: String,
    max_attempts: Option<u64>,
    max_runtime: Option<u64>,
) {
    // Ensure running lock is always released
    let _release = RunningGuard {
        running: running.clone(),
    };
    let db_pool = &db; // keep reference to DbPool for logging

    // Mark as running
    {
        let db = lock_db(&db);
        if let Ok(Some(mut j)) = dbmod::get_job(&db, &job_id) {
            j.status = JobStatus::Running;
            let _ = dbmod::update_job(&db, &j);
        }
    }
    logs::log(
        &db,
        "info",
        Some(&job_id),
        &format!("Job started with backend: {}", backend),
    );

    // Create state and store master seed
    let deterministic_state = DeterministicState::new();
    let master_seed_hex = deterministic_state.master_seed_hex();
    {
        let db = lock_db(&db);
        if let Ok(Some(mut j)) = dbmod::get_job(&db, &job_id) {
            j.master_seed = Some(master_seed_hex);
            let _ = dbmod::update_job(&db, &j);
        }
    }

    let _ = std::fs::create_dir_all("/data/checkpoints");
    let checkpoint_path = Some(std::path::PathBuf::from(format!(
        "/data/checkpoints/{}.json",
        job_id
    )));

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1))
        .unwrap_or(1);

    // Try GPU if CUDA backend and available
    let gpu_searchers: Vec<Box<dyn crate::search::GpuSearcher>> = if backend == "cuda" {
        #[cfg(feature = "cuda")]
        {
            let searchers = crate::gpu::try_init_gpu(&prefixes);
            if searchers.is_empty() {
                mark_failed(&db, &job_id, "CUDA GPU unavailable — check that NVIDIA drivers and container toolkit are configured");
                logs::log(&db, "error", Some(&job_id), "CUDA GPU unavailable");
                return;
            }
            searchers
        }
        #[cfg(not(feature = "cuda"))]
        {
            mark_failed(
                &db,
                &job_id,
                "CUDA backend not available (binary built without cuda feature)",
            );
            return;
        }
    } else {
        vec![]
    };

    let handle = if !gpu_searchers.is_empty() {
        SearchHandle::start_hybrid(&prefixes, num_threads, gpu_searchers)
    } else {
        SearchHandle::start_deterministic(
            &prefixes,
            deterministic_state,
            num_threads,
            checkpoint_path,
            10,
            false,
        )
    };

    let start = Instant::now();
    let mut last_update = Instant::now();

    loop {
        if handle.is_done() {
            break;
        }
        if let Some(limit) = max_attempts {
            if handle.stats(0).attempts >= limit {
                break;
            }
        }
        if let Some(limit) = max_runtime {
            if start.elapsed().as_secs() >= limit {
                break;
            }
        }

        std::thread::sleep(Duration::from_millis(200));

        if last_update.elapsed() >= Duration::from_secs(2) {
            last_update = Instant::now();
            let db = lock_db(&db);
            if let Ok(Some(mut j)) = dbmod::get_job(&db, &job_id) {
                let stats = handle.stats(0);
                j.attempts_done = stats.attempts;
                j.keys_per_second = stats.keys_per_sec;
                j.elapsed_seconds = stats.elapsed_secs;
                if let Some(s) = handle.get_deterministic_state() {
                    j.next_counter = Some(s.counter);
                }
                let _ = dbmod::update_job(&db, &j);
            }
        }
    }

    let elapsed = start.elapsed();
    let stats = handle.stats(0);
    let found = handle.is_done();

    if found {
        match handle.finish() {
            Ok(result) => {
                let db = lock_db(&db);
                let record = ResultRecord {
                    id: Uuid::new_v4().to_string(),
                    job_id: job_id.clone(),
                    prefix: result.matched_prefix.clone(),
                    public_key: result.public_key.clone(),
                    private_key: result.private_key.clone(),
                    candidate_seed: result.seed.clone(),
                    master_seed: result.master_seed.clone(),
                    counter: result.counter,
                    attempts: result.attempts,
                    elapsed_seconds: result.elapsed_secs,
                    keys_per_second: if result.elapsed_secs > 0.0 {
                        result.attempts as f64 / result.elapsed_secs
                    } else {
                        0.0
                    },
                    backend: "cpu".to_string(),
                    device: String::new(),
                    created_at: chrono_now(),
                };
                let _ = dbmod::insert_result(&db, &record);
                if let Ok(Some(mut j)) = dbmod::get_job(&db, &job_id) {
                    j.status = JobStatus::Completed;
                    j.attempts_done = stats.attempts;
                    j.keys_per_second = stats.keys_per_sec;
                    j.elapsed_seconds = elapsed.as_secs_f64();
                    let _ = dbmod::update_job(&db, &j);
                }
                logs::log(
                    db_pool,
                    "info",
                    Some(&job_id),
                    &format!("Match found: {}", result.matched_prefix),
                );
            }
            Err(e) => {
                let db = lock_db(&db);
                if let Ok(Some(mut j)) = dbmod::get_job(&db, &job_id) {
                    j.status = JobStatus::Failed;
                    j.notes = Some(format!("Search error: {}", e));
                    let _ = dbmod::update_job(&db, &j);
                }
                logs::log(
                    db_pool,
                    "error",
                    Some(&job_id),
                    &format!("Search failed: {}", e),
                );
            }
        }
    } else {
        let db = lock_db(&db);
        if let Ok(Some(mut j)) = dbmod::get_job(&db, &job_id) {
            j.status = JobStatus::Stopped;
            j.attempts_done = stats.attempts;
            j.keys_per_second = stats.keys_per_sec;
            j.elapsed_seconds = elapsed.as_secs_f64();
            let mut reasons = Vec::new();
            if let Some(limit) = max_attempts {
                if stats.attempts >= limit {
                    reasons.push(format!("reached max_attempts ({})", limit));
                }
            }
            if let Some(limit) = max_runtime {
                if elapsed.as_secs() >= limit {
                    reasons.push(format!("reached max_runtime ({}s)", limit));
                }
            }
            if !reasons.is_empty() {
                j.notes = Some(reasons.join(", "));
            }
            let _ = dbmod::update_job(&db, &j);
        }
    }
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

/// Check if current time is within the schedule window.
/// Format: start/end are "HH:MM" strings (e.g. "23:00", "07:00").
/// If start > end, the window crosses midnight (e.g. 23:00-07:00 = overnight).
fn is_in_window(settings: &crate::server::models::Settings) -> bool {
    let now = std::time::SystemTime::now();
    // Convert UTC to approximate local: get TZ from settings, fall back to UTC
    let epoch = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = epoch.as_secs();

    // Use a simple offset-based approach; for UTC we ignore DST
    // The timezone setting is stored as e.g. "UTC", "Europe/Zurich", etc.
    // For now use UTC; full tz support would need the chrono crate
    let total_secs = secs;
    let hours = ((total_secs / 3600) % 24) as u32;
    let minutes = ((total_secs / 60) % 60) as u32;
    let current_minutes = hours * 60 + minutes;

    let parse = |s: &str| -> Option<u32> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 2 {
            let h: u32 = parts[0].parse().ok()?;
            let m: u32 = parts[1].parse().ok()?;
            Some(h * 60 + m)
        } else {
            None
        }
    };

    let start = parse(&settings.schedule_start).unwrap_or(23 * 60);
    let end = parse(&settings.schedule_end).unwrap_or(7 * 60);

    if start <= end {
        current_minutes >= start && current_minutes < end
    } else {
        current_minutes >= start || current_minutes < end
    }
}

struct RunningGuard {
    running: Arc<TokioMutex<bool>>,
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        let rt = tokio::runtime::Handle::try_current();
        if let Ok(rt) = rt {
            rt.block_on(async {
                let mut r = self.running.lock().await;
                *r = false;
            });
        }
    }
}
