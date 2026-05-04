use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex as TokioMutex;
use tokio::time::sleep;
use uuid::Uuid;

use crate::deterministic::DeterministicState;
use crate::search::SearchHandle;
use crate::server::db::DbPool;
use crate::server::models::{Job, JobStatus, ResultRecord};

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
                let db = self.db.lock().unwrap();
                crate::server::db::get_next_queued_job(&db).ok().flatten()
            };

            if let Some(job) = next_job {
                *is_running = true;
                drop(is_running);

                let db = self.db.clone();
                let running = self.running.clone();
                let job_id = job.id.clone();
                let prefixes = job.prefixes.clone();
                let backend = job.backend.clone();

                tokio::task::spawn_blocking(move || {
                    run_job_in_thread(db, running, job_id, prefixes, backend);
                });
            }
        }
    }
}

fn run_job_in_thread(
    db: DbPool,
    running: Arc<TokioMutex<bool>>,
    job_id: String,
    prefixes: Vec<String>,
    _backend: String,
) {
    // Mark job as running
    {
        let db = db.lock().unwrap();
        if let Ok(Some(mut job)) = crate::server::db::get_job(&db, &job_id) {
            job.status = JobStatus::Running;
            let _ = crate::server::db::update_job(&db, &job);
        }
    }

    // Create deterministic state
    let deterministic_state = DeterministicState::new();
    let master_seed_hex = deterministic_state.master_seed_hex();

    // Store master seed on job
    {
        let db = db.lock().unwrap();
        if let Ok(Some(mut job)) = crate::server::db::get_job(&db, &job_id) {
            job.master_seed = Some(master_seed_hex.clone());
            let _ = crate::server::db::update_job(&db, &job);
        }
    }

    // Checkpoint path
    std::fs::create_dir_all("/data/checkpoints").ok();
    let checkpoint_path = Some(std::path::PathBuf::from(format!(
        "/data/checkpoints/{}.json",
        job_id
    )));

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get().saturating_sub(1).max(1))
        .unwrap_or(1);

    let handle = SearchHandle::start_deterministic(
        &prefixes,
        deterministic_state,
        num_threads,
        checkpoint_path,
        10,
        false,
    );

    let start = Instant::now();
    let mut last_progress_update = Instant::now();

    // Poll for results with progress updates
    loop {
        if handle.is_done() {
            break;
        }

        std::thread::sleep(Duration::from_millis(200));

        // Update progress every 2 seconds
        if last_progress_update.elapsed() >= Duration::from_secs(2) {
            last_progress_update = Instant::now();
            if let Ok(db) = db.lock() {
                if let Ok(Some(mut job)) = crate::server::db::get_job(&db, &job_id) {
                    let stats = handle.stats(0);
                    job.attempts_done = stats.attempts;
                    job.keys_per_second = stats.keys_per_sec;
                    job.elapsed_seconds = stats.elapsed_secs;
                    if let Some(state) = handle.get_deterministic_state() {
                        job.next_counter = Some(state.counter);
                    }
                    let _ = crate::server::db::update_job(&db, &job);
                }
            }
        }
    }

    let elapsed = start.elapsed();
    let stats = handle.stats(0);

    match handle.finish() {
        Ok(result) => {
            // Save result
            let db = db.lock().unwrap();
            let result_record = ResultRecord {
                id: Uuid::new_v4().to_string(),
                job_id: job_id.clone(),
                prefix: result.matched_prefix,
                public_key: result.public_key,
                private_key: result.private_key,
                candidate_seed: result.seed,
                master_seed: result.master_seed,
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
            let _ = crate::server::db::insert_result(&db, &result_record);

            // Mark job completed
            if let Ok(Some(mut job)) = crate::server::db::get_job(&db, &job_id) {
                job.status = JobStatus::Completed;
                job.attempts_done = stats.attempts;
                job.keys_per_second = stats.keys_per_sec;
                job.elapsed_seconds = elapsed.as_secs_f64();
                let _ = crate::server::db::update_job(&db, &job);
            }
        }
        Err(e) => {
            let db = db.lock().unwrap();
            if let Ok(Some(mut job)) = crate::server::db::get_job(&db, &job_id) {
                job.status = JobStatus::Failed;
                job.notes = Some(format!("Search error: {}", e));
                let _ = crate::server::db::update_job(&db, &job);
            }
        }
    }

    // Release the running lock
    let rt = tokio::runtime::Handle::current();
    rt.block_on(async {
        let mut r = running.lock().await;
        *r = false;
    });
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}
