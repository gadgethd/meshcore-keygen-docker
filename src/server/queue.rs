use std::io::BufRead;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::sleep;

use crate::server::db::DbPool;
use crate::server::models::{Job, JobStatus, ResultRecord};
use uuid::Uuid;

/// The queue manager watches the jobs table and runs queued jobs.
/// Only one job runs at a time (per GPU).
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

    /// Start the queue manager loop. Runs forever, polling for work.
    pub async fn run(&self) {
        loop {
            sleep(Duration::from_secs(2)).await;

            // Check if we should start a new job
            let mut is_running = self.running.lock().await;
            if *is_running {
                continue; // already running a job
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

                tokio::task::spawn_blocking(move || {
                    run_job(db, running, job_id, prefixes);
                });
            }
        }
    }
}

fn run_job(db: DbPool, running: Arc<TokioMutex<bool>>, job_id: String, prefixes: Vec<String>) {
    // Mark job as running
    {
        let db = db.lock().unwrap();
        if let Ok(Some(mut job)) = crate::server::db::get_job(&db, &job_id) {
            job.status = JobStatus::Running;
            let _ = crate::server::db::update_job(&db, &job);
        }
    }

    // Build checkpoint path
    std::fs::create_dir_all("/data/checkpoints").ok();
    let checkpoint_path = format!("/data/checkpoints/{}.json", job_id);

    let mut args: Vec<String> = vec![
        "--deterministic".into(),
        "--checkpoint".into(),
        checkpoint_path.clone(),
        "--checkpoint-interval".into(),
        "10".into(),
        "--json-progress".into(),
    ];
    args.extend(prefixes.iter().map(|p| p.clone()));

    let mut child = match Command::new("/app/mc-keygen")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to start keygen for job {}: {}", job_id, e);
            mark_failed(&db, &job_id, &e.to_string());
            return;
        }
    };

    // Read JSON progress from stdout
    if let Some(stdout) = child.stdout.take() {
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                        let typ = val["type"].as_str().unwrap_or("");
                        match typ {
                            "progress" => {
                                let _ = update_progress(&db, &job_id, &val);
                            }
                            "result" => {
                                let _ = save_result(&db, &job_id, &val);
                                // Mark job completed
                                let db = db.lock().unwrap();
                                if let Ok(Some(mut job)) = crate::server::db::get_job(&db, &job_id)
                                {
                                    job.status = JobStatus::Completed;
                                    let _ = crate::server::db::update_job(&db, &job);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }

    let status = child.wait();
    match status {
        Ok(s) if !s.success() => {
            eprintln!("Keygen for job {} exited with {:?}", job_id, s.code());
        }
        Err(e) => {
            eprintln!("Keygen for job {} failed: {}", job_id, e);
        }
        _ => {}
    }

    // Ensure job is marked completed/failed if process exits without result
    {
        let db = db.lock().unwrap();
        if let Ok(Some(job)) = crate::server::db::get_job(&db, &job_id) {
            if job.status == JobStatus::Running {
                let _ = crate::server::db::update_job(
                    &db,
                    &Job {
                        status: JobStatus::Failed,
                        ..job
                    },
                );
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

fn update_progress(db: &DbPool, job_id: &str, val: &serde_json::Value) -> Result<(), String> {
    let db = db.lock().map_err(|e| e.to_string())?;
    let mut job = crate::server::db::get_job(&db, job_id)
        .map_err(|e| e.to_string())?
        .ok_or("job not found")?;

    if let Some(attempts) = val["attempts"].as_u64() {
        job.attempts_done = attempts;
    }
    if let Some(kps) = val["keys_per_second"].as_f64() {
        job.keys_per_second = kps;
    }
    if let Some(elapsed) = val["elapsed_seconds"].as_f64() {
        job.elapsed_seconds = elapsed;
    }
    if let Some(counter) = val["next_counter"].as_u64() {
        job.next_counter = Some(counter);
    }

    crate::server::db::update_job(&db, &job).map_err(|e| e.to_string())
}

fn save_result(db: &DbPool, job_id: &str, val: &serde_json::Value) -> Result<(), String> {
    let db = db.lock().map_err(|e| e.to_string())?;
    let result = ResultRecord {
        id: Uuid::new_v4().to_string(),
        job_id: job_id.to_string(),
        prefix: val["prefix"].as_str().unwrap_or("").to_string(),
        public_key: val["public_key"].as_str().unwrap_or("").to_string(),
        private_key: val["private_key"].as_str().unwrap_or("").to_string(),
        candidate_seed: val["candidate_seed"].as_str().map(|s| s.to_string()),
        master_seed: val["master_seed"].as_str().map(|s| s.to_string()),
        counter: val["counter"].as_u64(),
        attempts: val["attempts"].as_u64().unwrap_or(0),
        elapsed_seconds: val["elapsed_seconds"].as_f64().unwrap_or(0.0),
        keys_per_second: 0.0,
        backend: "cpu".to_string(),
        device: "".to_string(),
        created_at: chrono_now(),
    };
    crate::server::db::insert_result(&db, &result).map_err(|e| e.to_string())
}

fn mark_failed(db: &DbPool, job_id: &str, error: &str) {
    let db = db.lock().unwrap();
    if let Ok(Some(mut job)) = crate::server::db::get_job(&db, job_id) {
        job.status = JobStatus::Failed;
        job.notes = Some(format!("Error: {}", error));
        let _ = crate::server::db::update_job(&db, &job);
    }
}

fn chrono_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}
