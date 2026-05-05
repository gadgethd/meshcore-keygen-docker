use serde::{Deserialize, Serialize};

/// A job in the vanity key search queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub name: String,
    pub prefixes: Vec<String>,
    pub backend: String,
    pub device: String,
    pub status: JobStatus,
    pub priority: i32,
    pub created_at: String,
    pub updated_at: String,
    pub master_seed: Option<String>,
    pub next_counter: Option<u64>,
    pub attempts_done: u64,
    pub keys_per_second: f64,
    pub elapsed_seconds: f64,
    pub cpu_reserved_cores: usize,
    pub cpu_worker_threads: usize,
    pub max_attempts: Option<u64>,
    pub max_runtime: Option<u64>,
    pub schedule_enabled: bool,
    pub schedule_start: Option<String>,
    pub schedule_end: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Stopped,
    Scheduled,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Paused => "paused",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Stopped => "stopped",
            JobStatus::Scheduled => "scheduled",
        }
    }
}

/// A found vanity key result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultRecord {
    pub id: String,
    pub job_id: String,
    pub prefix: String,
    pub public_key: String,
    pub private_key: String,
    pub candidate_seed: Option<String>,
    pub master_seed: Option<String>,
    pub counter: Option<u64>,
    pub attempts: u64,
    pub elapsed_seconds: f64,
    pub keys_per_second: f64,
    pub backend: String,
    pub device: String,
    pub created_at: String,
}

/// A benchmark result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRecord {
    pub id: String,
    pub created_at: String,
    pub backend: String,
    pub device: String,
    pub prefix_length: u32,
    pub target_prefix: String,
    pub attempts: u64,
    pub elapsed_seconds: f64,
    pub keys_per_second: f64,
    pub found: bool,
    pub timeout_seconds: u64,
    pub cpu_total_cores: usize,
    pub cpu_reserved_cores: usize,
    pub cpu_worker_threads: usize,
    pub is_default: bool,
}

/// Application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub reserved_cpu_cores: usize,
    pub max_worker_threads: Option<usize>,
    pub checkpoint_interval_secs: u64,
    pub default_backend: String,
    pub default_benchmark_id: Option<String>,
    pub timezone: String,
    pub hide_secrets: bool,
    pub max_log_lines: usize,
    pub bind_address: String,
    pub password_hash: Option<String>,
    pub schedule_enabled: bool,
    pub schedule_start: String,
    pub schedule_end: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            reserved_cpu_cores: 1,
            max_worker_threads: None,
            checkpoint_interval_secs: 10,
            default_backend: "cpu".to_string(),
            default_benchmark_id: None,
            timezone: "UTC".to_string(),
            hide_secrets: true,
            max_log_lines: 10000,
            bind_address: "0.0.0.0:8080".to_string(),
            password_hash: None,
            schedule_enabled: false,
            schedule_start: "23:00".to_string(),
            schedule_end: "07:00".to_string(),
        }
    }
}

/// Log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: i64,
    pub timestamp: String,
    pub level: String,
    pub job_id: Option<String>,
    pub message: String,
}

/// Response from /api/estimate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EstimateResponse {
    pub prefix_length: usize,
    pub expected_attempts: f64,
    pub keys_per_second: f64,
    pub estimated_seconds: f64,
    pub milestone_50pct_seconds: f64,
    pub milestone_90pct_seconds: f64,
    pub milestone_95pct_seconds: f64,
    pub milestone_99pct_seconds: f64,
    pub backend: String,
    pub device: String,
    pub benchmark_id: Option<String>,
    pub benchmark_age: Option<String>,
}

/// System status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub cpu_total_cores: usize,
    pub cpu_reserved_cores: usize,
    pub cpu_available_workers: usize,
    pub gpu_available: bool,
    pub gpu_name: Option<String>,
    pub active_job: Option<Job>,
    pub queue_length: usize,
    pub results_count: usize,
    pub last_benchmark_keys_per_second: Option<f64>,
}
