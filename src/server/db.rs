use rusqlite::{params, Connection, Result};
use std::sync::{Arc, Mutex};

use super::models::*;

pub type DbPool = Arc<Mutex<Connection>>;

/// Create a new database connection and run migrations.
pub fn open(path: &str) -> Result<DbPool> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    run_migrations(&conn)?;
    Ok(Arc::new(Mutex::new(conn)))
}

/// Run schema migrations.
fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS jobs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL DEFAULT '',
            prefixes TEXT NOT NULL DEFAULT '[]',
            backend TEXT NOT NULL DEFAULT 'cpu',
            device TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'queued',
            priority INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL DEFAULT '',
            master_seed TEXT,
            next_counter INTEGER,
            attempts_done INTEGER NOT NULL DEFAULT 0,
            keys_per_second REAL NOT NULL DEFAULT 0,
            elapsed_seconds REAL NOT NULL DEFAULT 0,
            cpu_reserved_cores INTEGER NOT NULL DEFAULT 1,
            cpu_worker_threads INTEGER NOT NULL DEFAULT 0,
            max_attempts INTEGER,
            max_runtime INTEGER,
            schedule_enabled INTEGER NOT NULL DEFAULT 0,
            schedule_start TEXT,
            schedule_end TEXT,
            notes TEXT
        );

        CREATE TABLE IF NOT EXISTS results (
            id TEXT PRIMARY KEY,
            job_id TEXT NOT NULL,
            prefix TEXT NOT NULL,
            public_key TEXT NOT NULL,
            private_key TEXT NOT NULL,
            candidate_seed TEXT,
            master_seed TEXT,
            counter INTEGER,
            attempts INTEGER NOT NULL DEFAULT 0,
            elapsed_seconds REAL NOT NULL DEFAULT 0,
            keys_per_second REAL NOT NULL DEFAULT 0,
            backend TEXT NOT NULL DEFAULT 'cpu',
            device TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL DEFAULT '',
            FOREIGN KEY (job_id) REFERENCES jobs(id)
        );

        CREATE TABLE IF NOT EXISTS benchmarks (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL DEFAULT '',
            backend TEXT NOT NULL DEFAULT 'cpu',
            device TEXT NOT NULL DEFAULT '',
            prefix_length INTEGER NOT NULL DEFAULT 6,
            target_prefix TEXT NOT NULL DEFAULT '',
            attempts INTEGER NOT NULL DEFAULT 0,
            elapsed_seconds REAL NOT NULL DEFAULT 0,
            keys_per_second REAL NOT NULL DEFAULT 0,
            found INTEGER NOT NULL DEFAULT 0,
            timeout_seconds INTEGER NOT NULL DEFAULT 0,
            cpu_total_cores INTEGER NOT NULL DEFAULT 1,
            cpu_reserved_cores INTEGER NOT NULL DEFAULT 1,
            cpu_worker_threads INTEGER NOT NULL DEFAULT 0,
            is_default INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL DEFAULT '',
            level TEXT NOT NULL DEFAULT 'info',
            job_id TEXT,
            message TEXT NOT NULL DEFAULT ''
        );

        CREATE INDEX IF NOT EXISTS idx_results_job_id ON results(job_id);
        CREATE INDEX IF NOT EXISTS idx_results_created ON results(created_at);
        CREATE INDEX IF NOT EXISTS idx_benchmarks_created ON benchmarks(created_at);
        CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON logs(timestamp);
        CREATE INDEX IF NOT EXISTS idx_logs_job_id ON logs(job_id);
        ",
    )?;

    // Load default settings if table is empty
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM settings", [], |r| r.get(0))?;
    if count == 0 {
        let defaults = Settings::default();
        save_setting(
            conn,
            "reserved_cpu_cores",
            &defaults.reserved_cpu_cores.to_string(),
        )?;
        save_setting(
            conn,
            "checkpoint_interval_secs",
            &defaults.checkpoint_interval_secs.to_string(),
        )?;
        save_setting(conn, "default_backend", &defaults.default_backend)?;
        save_setting(conn, "timezone", &defaults.timezone)?;
        save_setting(conn, "hide_secrets", &defaults.hide_secrets.to_string())?;
        save_setting(conn, "max_log_lines", &defaults.max_log_lines.to_string())?;
        save_setting(conn, "bind_address", &defaults.bind_address)?;
        save_setting(
            conn,
            "schedule_enabled",
            &defaults.schedule_enabled.to_string(),
        )?;
        save_setting(conn, "schedule_start", &defaults.schedule_start)?;
        save_setting(conn, "schedule_end", &defaults.schedule_end)?;
    }

    Ok(())
}

// --- Settings helpers ---

pub fn save_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .ok()
}

pub fn load_settings(conn: &Connection) -> Settings {
    Settings {
        reserved_cpu_cores: get_setting(conn, "reserved_cpu_cores")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        max_worker_threads: get_setting(conn, "max_worker_threads").and_then(|v| v.parse().ok()),
        checkpoint_interval_secs: get_setting(conn, "checkpoint_interval_secs")
            .and_then(|v| v.parse().ok())
            .unwrap_or(10),
        default_backend: get_setting(conn, "default_backend").unwrap_or_else(|| "cpu".to_string()),
        default_benchmark_id: get_setting(conn, "default_benchmark_id"),
        timezone: get_setting(conn, "timezone").unwrap_or_else(|| "UTC".to_string()),
        hide_secrets: get_setting(conn, "hide_secrets")
            .and_then(|v| v.parse().ok())
            .unwrap_or(true),
        max_log_lines: get_setting(conn, "max_log_lines")
            .and_then(|v| v.parse().ok())
            .unwrap_or(10000),
        bind_address: get_setting(conn, "bind_address")
            .unwrap_or_else(|| "0.0.0.0:8080".to_string()),
        password_hash: get_setting(conn, "password_hash"),
        schedule_enabled: get_setting(conn, "schedule_enabled")
            .and_then(|v| v.parse().ok())
            .unwrap_or(false),
        schedule_start: get_setting(conn, "schedule_start").unwrap_or_else(|| "23:00".to_string()),
        schedule_end: get_setting(conn, "schedule_end").unwrap_or_else(|| "07:00".to_string()),
    }
}

// --- Job helpers ---

pub fn insert_job(conn: &Connection, job: &Job) -> Result<()> {
    conn.execute(
        "INSERT INTO jobs (id, name, prefixes, backend, device, status, priority,
         created_at, updated_at, master_seed, next_counter, attempts_done,
         keys_per_second, elapsed_seconds, cpu_reserved_cores, cpu_worker_threads,
         max_attempts, max_runtime, schedule_enabled, schedule_start, schedule_end, notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)",
        params![
            job.id,
            job.name,
            serde_json::to_string(&job.prefixes).unwrap_or_default(),
            job.backend,
            job.device,
            job.status.as_str(),
            job.priority,
            job.created_at,
            job.updated_at,
            job.master_seed,
            job.next_counter,
            job.attempts_done,
            job.keys_per_second,
            job.elapsed_seconds,
            job.cpu_reserved_cores,
            job.cpu_worker_threads,
            job.max_attempts,
            job.max_runtime,
            job.schedule_enabled as i32,
            job.schedule_start,
            job.schedule_end,
            job.notes,
        ],
    )?;
    Ok(())
}

pub fn update_job(conn: &Connection, job: &Job) -> Result<()> {
    conn.execute(
        "UPDATE jobs SET name=?1, prefixes=?2, backend=?3, device=?4, status=?5,
         priority=?6, updated_at=?7, master_seed=?8, next_counter=?9,
         attempts_done=?10, keys_per_second=?11, elapsed_seconds=?12,
         cpu_reserved_cores=?13, cpu_worker_threads=?14, max_attempts=?15,
         max_runtime=?16, schedule_enabled=?17, schedule_start=?18,
         schedule_end=?19, notes=?20 WHERE id=?21",
        params![
            job.name,
            serde_json::to_string(&job.prefixes).unwrap_or_default(),
            job.backend,
            job.device,
            job.status.as_str(),
            job.priority,
            job.updated_at,
            job.master_seed,
            job.next_counter,
            job.attempts_done,
            job.keys_per_second,
            job.elapsed_seconds,
            job.cpu_reserved_cores,
            job.cpu_worker_threads,
            job.max_attempts,
            job.max_runtime,
            job.schedule_enabled as i32,
            job.schedule_start,
            job.schedule_end,
            job.notes,
            job.id,
        ],
    )?;
    Ok(())
}

pub fn get_job(conn: &Connection, id: &str) -> Result<Option<Job>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, prefixes, backend, device, status, priority,
         created_at, updated_at, master_seed, next_counter, attempts_done,
         keys_per_second, elapsed_seconds, cpu_reserved_cores, cpu_worker_threads,
         max_attempts, max_runtime, schedule_enabled, schedule_start, schedule_end, notes
         FROM jobs WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![id], row_to_job)?;
    match rows.next() {
        Some(Ok(job)) => Ok(Some(job)),
        _ => Ok(None),
    }
}

pub fn list_jobs(conn: &Connection) -> Result<Vec<Job>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, prefixes, backend, device, status, priority,
         created_at, updated_at, master_seed, next_counter, attempts_done,
         keys_per_second, elapsed_seconds, cpu_reserved_cores, cpu_worker_threads,
         max_attempts, max_runtime, schedule_enabled, schedule_start, schedule_end, notes
         FROM jobs ORDER BY priority DESC, created_at ASC",
    )?;
    let jobs = stmt
        .query_map([], row_to_job)?
        .collect::<Result<Vec<_>>>()?;
    Ok(jobs)
}

pub fn delete_job(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM jobs WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn get_next_queued_job(conn: &Connection) -> Result<Option<Job>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, prefixes, backend, device, status, priority,
         created_at, updated_at, master_seed, next_counter, attempts_done,
         keys_per_second, elapsed_seconds, cpu_reserved_cores, cpu_worker_threads,
         max_attempts, max_runtime, schedule_enabled, schedule_start, schedule_end, notes
         FROM jobs WHERE status = 'queued'
         ORDER BY priority DESC, created_at ASC LIMIT 1",
    )?;
    let mut rows = stmt.query_map([], row_to_job)?;
    match rows.next() {
        Some(Ok(job)) => Ok(Some(job)),
        _ => Ok(None),
    }
}

fn row_to_job(row: &rusqlite::Row) -> rusqlite::Result<Job> {
    let prefixes_str: String = row.get(2)?;
    let status_str: String = row.get(5)?;
    Ok(Job {
        id: row.get(0)?,
        name: row.get(1)?,
        prefixes: serde_json::from_str(&prefixes_str).unwrap_or_default(),
        backend: row.get(3)?,
        device: row.get(4)?,
        status: match status_str.as_str() {
            "queued" => JobStatus::Queued,
            "running" => JobStatus::Running,
            "paused" => JobStatus::Paused,
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            "stopped" => JobStatus::Stopped,
            "scheduled" => JobStatus::Scheduled,
            _ => JobStatus::Queued,
        },
        priority: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        master_seed: row.get(9)?,
        next_counter: row.get(10)?,
        attempts_done: row.get(11)?,
        keys_per_second: row.get(12)?,
        elapsed_seconds: row.get(13)?,
        cpu_reserved_cores: row.get(14)?,
        cpu_worker_threads: row.get(15)?,
        max_attempts: row.get(16)?,
        max_runtime: row.get(17)?,
        schedule_enabled: row.get::<_, i32>(18)? != 0,
        schedule_start: row.get(19)?,
        schedule_end: row.get(20)?,
        notes: row.get(21)?,
    })
}

// --- Result helpers ---

pub fn insert_result(conn: &Connection, r: &ResultRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO results (id, job_id, prefix, public_key, private_key,
         candidate_seed, master_seed, counter, attempts, elapsed_seconds,
         keys_per_second, backend, device, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![
            r.id,
            r.job_id,
            r.prefix,
            r.public_key,
            r.private_key,
            r.candidate_seed,
            r.master_seed,
            r.counter,
            r.attempts,
            r.elapsed_seconds,
            r.keys_per_second,
            r.backend,
            r.device,
            r.created_at,
        ],
    )?;
    Ok(())
}

pub fn list_results(conn: &Connection) -> Result<Vec<ResultRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, job_id, prefix, public_key, private_key, candidate_seed,
         master_seed, counter, attempts, elapsed_seconds, keys_per_second,
         backend, device, created_at
         FROM results ORDER BY created_at DESC",
    )?;
    let results = stmt
        .query_map([], |row| {
            Ok(ResultRecord {
                id: row.get(0)?,
                job_id: row.get(1)?,
                prefix: row.get(2)?,
                public_key: row.get(3)?,
                private_key: row.get(4)?,
                candidate_seed: row.get(5)?,
                master_seed: row.get(6)?,
                counter: row.get(7)?,
                attempts: row.get(8)?,
                elapsed_seconds: row.get(9)?,
                keys_per_second: row.get(10)?,
                backend: row.get(11)?,
                device: row.get(12)?,
                created_at: row.get(13)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;
    Ok(results)
}

pub fn delete_result(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM results WHERE id = ?1", params![id])?;
    Ok(())
}

// --- Benchmark helpers ---

pub fn insert_benchmark(conn: &Connection, b: &BenchmarkRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO benchmarks (id, created_at, backend, device, prefix_length,
         target_prefix, attempts, elapsed_seconds, keys_per_second, found,
         timeout_seconds, cpu_total_cores, cpu_reserved_cores, cpu_worker_threads, is_default)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![
            b.id,
            b.created_at,
            b.backend,
            b.device,
            b.prefix_length,
            b.target_prefix,
            b.attempts,
            b.elapsed_seconds,
            b.keys_per_second,
            b.found as i32,
            b.timeout_seconds,
            b.cpu_total_cores,
            b.cpu_reserved_cores,
            b.cpu_worker_threads,
            b.is_default as i32,
        ],
    )?;
    Ok(())
}

pub fn list_benchmarks(conn: &Connection) -> Result<Vec<BenchmarkRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at, backend, device, prefix_length, target_prefix,
         attempts, elapsed_seconds, keys_per_second, found, timeout_seconds,
         cpu_total_cores, cpu_reserved_cores, cpu_worker_threads, is_default
         FROM benchmarks ORDER BY created_at DESC",
    )?;
    let results = stmt
        .query_map([], |row| {
            Ok(BenchmarkRecord {
                id: row.get(0)?,
                created_at: row.get(1)?,
                backend: row.get(2)?,
                device: row.get(3)?,
                prefix_length: row.get(4)?,
                target_prefix: row.get(5)?,
                attempts: row.get(6)?,
                elapsed_seconds: row.get(7)?,
                keys_per_second: row.get(8)?,
                found: row.get::<_, i32>(9)? != 0,
                timeout_seconds: row.get(10)?,
                cpu_total_cores: row.get(11)?,
                cpu_reserved_cores: row.get(12)?,
                cpu_worker_threads: row.get(13)?,
                is_default: row.get::<_, i32>(14)? != 0,
            })
        })?
        .collect::<Result<Vec<_>>>()?;
    Ok(results)
}

pub fn delete_benchmark(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM benchmarks WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_default_benchmark(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("UPDATE benchmarks SET is_default = 0", [])?;
    conn.execute(
        "UPDATE benchmarks SET is_default = 1 WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn get_default_benchmark(conn: &Connection) -> Result<Option<BenchmarkRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, created_at, backend, device, prefix_length, target_prefix,
         attempts, elapsed_seconds, keys_per_second, found, timeout_seconds,
         cpu_total_cores, cpu_reserved_cores, cpu_worker_threads, is_default
         FROM benchmarks WHERE is_default = 1 ORDER BY created_at DESC LIMIT 1",
    )?;
    let mut rows = stmt.query_map([], |row| {
        Ok(BenchmarkRecord {
            id: row.get(0)?,
            created_at: row.get(1)?,
            backend: row.get(2)?,
            device: row.get(3)?,
            prefix_length: row.get(4)?,
            target_prefix: row.get(5)?,
            attempts: row.get(6)?,
            elapsed_seconds: row.get(7)?,
            keys_per_second: row.get(8)?,
            found: row.get::<_, i32>(9)? != 0,
            timeout_seconds: row.get(10)?,
            cpu_total_cores: row.get(11)?,
            cpu_reserved_cores: row.get(12)?,
            cpu_worker_threads: row.get(13)?,
            is_default: row.get::<_, i32>(14)? != 0,
        })
    })?;
    match rows.next() {
        Some(Ok(b)) => Ok(Some(b)),
        _ => Ok(None),
    }
}

// --- Log helpers ---

pub fn insert_log(conn: &Connection, entry: &LogEntry) -> Result<()> {
    conn.execute(
        "INSERT INTO logs (timestamp, level, job_id, message) VALUES (?1,?2,?3,?4)",
        params![entry.timestamp, entry.level, entry.job_id, entry.message],
    )?;
    Ok(())
}

pub fn list_logs(conn: &Connection, limit: usize, job_id: Option<&str>) -> Result<Vec<LogEntry>> {
    if let Some(jid) = job_id {
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, level, job_id, message FROM logs WHERE job_id = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![jid, limit as i64], |row| row_to_log_entry(row))?;
        rows.collect::<Result<Vec<_>>>()
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, level, job_id, message FROM logs ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| row_to_log_entry(row))?;
        rows.collect::<Result<Vec<_>>>()
    }
}

fn row_to_log_entry(row: &rusqlite::Row) -> rusqlite::Result<LogEntry> {
    Ok(LogEntry {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        level: row.get(2)?,
        job_id: row.get(3)?,
        message: row.get(4)?,
    })
}
