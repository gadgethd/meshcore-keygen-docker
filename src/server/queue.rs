use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use uuid::Uuid;

use crate::deterministic::DeterministicState;
use crate::search::SearchHandle;
use crate::server::api::logs;
use crate::server::db::{self as dbmod};
use crate::server::models::{JobStatus, ResultRecord};

type DbPool = Arc<Mutex<rusqlite::Connection>>;

pub struct QueueManager {
    db: DbPool,
    running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    active_job_id: Arc<Mutex<Option<String>>>,
}

impl QueueManager {
    pub fn new(
        db: DbPool,
        cancel: Arc<AtomicBool>,
        active_job_id: Arc<Mutex<Option<String>>>,
    ) -> Self {
        QueueManager {
            db,
            running: Arc::new(AtomicBool::new(false)),
            cancel,
            active_job_id,
        }
    }

    pub async fn run(&self) {
        loop {
            sleep(Duration::from_secs(2)).await;

            if self.running.load(Ordering::Relaxed) {
                continue;
            }

            let next_job = {
                let db = lock_db(&self.db);
                dbmod::get_next_queued_job(&db).ok().flatten()
            };

            if let Some(job) = next_job {
                {
                    let db = lock_db(&self.db);
                    let settings = dbmod::load_settings(&db);
                    if settings.schedule_enabled && !is_in_window(&settings) {
                        continue;
                    }
                }

                // Validate limits
                if job.max_attempts == Some(0) || job.max_runtime == Some(0) {
                    mark_failed(
                        &self.db,
                        &job.id,
                        "max_attempts or max_runtime cannot be zero",
                    );
                    continue;
                }

                self.running.store(true, Ordering::Relaxed);
                {
                    let mut lock = self.active_job_id.lock().unwrap_or_else(|e| e.into_inner());
                    *lock = Some(job.id.clone());
                }
                self.cancel.store(false, Ordering::Relaxed);

                let db = self.db.clone();
                let running = self.running.clone();
                let cancel = self.cancel.clone();
                let active_id = self.active_job_id.clone();
                let job_id = job.id.clone();
                let prefixes = job.prefixes.clone();
                let backend = job.backend.clone();
                let max_attempts = job.max_attempts;
                let max_runtime = job.max_runtime;

                tokio::task::spawn_blocking(move || {
                    run_job_sync(
                        db,
                        running,
                        cancel,
                        active_id,
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
    running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    active_job_id: Arc<Mutex<Option<String>>>,
    job_id: String,
    prefixes: Vec<String>,
    backend: String,
    max_attempts: Option<u64>,
    max_runtime: Option<u64>,
) {
    let db_pool = &db;

    // Mark as running in DB
    {
        let db = lock_db(&db);
        if let Ok(Some(mut j)) = dbmod::get_job(&db, &job_id) {
            j.status = JobStatus::Running;
            let _ = dbmod::update_job(&db, &j);
        }
    }
    logs::log(
        db_pool,
        "info",
        Some(&job_id),
        &format!("Job started: {}", backend),
    );

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
    let checkpoint_path = std::path::PathBuf::from(format!("/data/checkpoints/{}.json", job_id));

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1))
        .unwrap_or(1);

    let gpu_searchers: Vec<Box<dyn crate::search::GpuSearcher>> = if backend == "cuda" {
        #[cfg(feature = "cuda")]
        {
            let searchers = crate::gpu::try_init_gpu(&prefixes);
            if searchers.is_empty() {
                mark_failed(db_pool, &job_id, "CUDA GPU unavailable");
                logs::log(db_pool, "error", Some(&job_id), "CUDA GPU unavailable");
                finalize(running, cancel, active_job_id, &job_id);
                return;
            }
            searchers
        }
        #[cfg(not(feature = "cuda"))]
        {
            mark_failed(
                db_pool,
                &job_id,
                "CUDA not available (built without cuda feature)",
            );
            finalize(running, cancel, active_job_id, &job_id);
            return;
        }
    } else {
        vec![]
    };

    let has_gpu = !gpu_searchers.is_empty();
    let handle = if has_gpu {
        SearchHandle::start_hybrid(&prefixes, num_threads, gpu_searchers)
    } else {
        SearchHandle::start_deterministic(
            &prefixes,
            deterministic_state,
            num_threads,
            Some(checkpoint_path),
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
        if cancel.load(Ordering::Relaxed) {
            handle.request_stop();
            break;
        }
        if let Some(limit) = max_attempts {
            if handle.stats(0).attempts >= limit {
                handle.request_stop();
                break;
            }
        }
        if let Some(limit) = max_runtime {
            if start.elapsed().as_secs() >= limit {
                handle.request_stop();
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

            // Save checkpoint
            if !has_gpu {
                let _ = handle.save_checkpoint(&job_id, &prefixes, &backend, "", num_threads, 1);
            }
        }
    }

    let elapsed = start.elapsed();
    let stats = handle.stats(0);
    let found = handle.is_done();
    let cancelled = cancel.load(Ordering::Relaxed);
    if !found {
        handle.request_stop();
    }
    let finish_result = handle.finish();

    if let Ok(result) = finish_result {
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
            backend: backend.clone(),
            device: if has_gpu {
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
    } else if cancelled {
        let db = lock_db(&db);
        if let Ok(Some(mut j)) = dbmod::get_job(&db, &job_id) {
            let was_stopped = j.status == JobStatus::Stopped;
            j.status = if was_stopped {
                JobStatus::Stopped
            } else {
                JobStatus::Paused
            };
            j.attempts_done = stats.attempts;
            j.keys_per_second = stats.keys_per_sec;
            j.elapsed_seconds = elapsed.as_secs_f64();
            let _ = dbmod::update_job(&db, &j);
            logs::log(
                db_pool,
                "info",
                Some(&job_id),
                if was_stopped {
                    "Job stopped by user"
                } else {
                    "Job paused by user"
                },
            );
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

    finalize(running, cancel, active_job_id, &job_id);
}

fn finalize(
    running: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    active_job_id: Arc<Mutex<Option<String>>>,
    job_id: &str,
) {
    running.store(false, Ordering::Relaxed);
    cancel.store(false, Ordering::Relaxed);
    if let Ok(mut lock) = active_job_id.lock() {
        *lock = None;
    }
    // Suppress unused warning
    let _ = job_id;
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

fn is_in_window(settings: &crate::server::models::Settings) -> bool {
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = epoch.as_secs();
    let hours = ((secs / 3600) % 24) as u32;
    let minutes = ((secs / 60) % 60) as u32;
    let current = hours * 60 + minutes;
    let parse = |s: &str| -> Option<u32> {
        let p: Vec<&str> = s.split(':').collect();
        if p.len() == 2 {
            let h: u32 = p[0].parse().ok()?;
            let m: u32 = p[1].parse().ok()?;
            Some(h * 60 + m)
        } else {
            None
        }
    };
    let start = parse(&settings.schedule_start).unwrap_or(23 * 60);
    let end = parse(&settings.schedule_end).unwrap_or(7 * 60);
    if start <= end {
        current >= start && current < end
    } else {
        current >= start || current < end
    }
}
