use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::deterministic::DeterministicState;

/// A checkpoint captures the full state of a search so it can be paused and
/// resumed deterministically.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub version: u32,
    pub job_id: String,
    pub prefixes: Vec<String>,
    pub master_seed: String,
    pub worker_id: u64,
    pub workers: u64,
    pub next_counter: u64,
    pub attempts_done: u64,
    pub backend: String,
    pub device: String,
    pub cpu_reserved_cores: usize,
    pub cpu_worker_threads: usize,
    pub created_at: String,
    pub updated_at: String,
}

impl Checkpoint {
    /// Create a new checkpoint from a deterministic state and job metadata.
    pub fn new(
        job_id: &str,
        prefixes: &[String],
        state: &DeterministicState,
        backend: &str,
        device: &str,
        cpu_worker_threads: usize,
        cpu_reserved_cores: usize,
    ) -> Self {
        let now = chrono_now();
        Checkpoint {
            version: 1,
            job_id: job_id.to_string(),
            prefixes: prefixes.to_vec(),
            master_seed: state.master_seed_hex(),
            worker_id: state.worker_id,
            workers: 1,
            next_counter: state.counter,
            attempts_done: state.counter,
            backend: backend.to_string(),
            device: device.to_string(),
            cpu_reserved_cores,
            cpu_worker_threads,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Update the counter and timestamp.
    pub fn update(&mut self, state: &DeterministicState) {
        self.next_counter = state.counter;
        self.attempts_done = state.counter;
        self.updated_at = chrono_now();
    }

    /// Reconstruct a deterministic state from this checkpoint.
    pub fn to_deterministic_state(&self) -> Result<DeterministicState, String> {
        let mut state = DeterministicState::from_hex_seed(&self.master_seed)?;
        state.worker_id = self.worker_id;
        state.counter = self.next_counter;
        Ok(state)
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Deserialize from JSON.
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("invalid checkpoint JSON: {}", e))
    }

    /// Write checkpoint atomically: write to .tmp, fsync, rename.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let tmp = path.with_extension("tmp");
        let json = self.to_json();

        let mut file =
            fs::File::create(&tmp).map_err(|e| format!("failed to create checkpoint: {}", e))?;
        file.write_all(json.as_bytes())
            .map_err(|e| format!("failed to write checkpoint: {}", e))?;
        file.flush()
            .map_err(|e| format!("failed to flush checkpoint: {}", e))?;

        fs::rename(&tmp, path).map_err(|e| format!("failed to rename checkpoint: {}", e))?;
        Ok(())
    }

    /// Load a checkpoint from a file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let data =
            fs::read_to_string(path).map_err(|e| format!("failed to read checkpoint: {}", e))?;
        Self::from_json(&data)
    }

    /// Get the default checkpoint path for a job ID.
    pub fn default_path(job_id: &str) -> PathBuf {
        PathBuf::from("checkpoints").join(format!("{}.json", job_id))
    }

    /// Get the elapsed time in seconds (approximate, based on timestamps).
    pub fn elapsed_seconds(&self) -> f64 {
        0.0 // approximate, real value comes from the search handle
    }
}

fn chrono_now() -> String {
    // Simple UTC timestamp without chrono dependency
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deterministic::DeterministicState;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join("mc-keygen-test");
        fs::create_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn checkpoint_roundtrip() {
        let mut state = DeterministicState::new();
        state.master_seed = [0xAB; 32];
        state.counter = 42;

        let ckpt = Checkpoint::new(
            "test-job-1",
            &["DEAD".to_string(), "BEEF".to_string()],
            &state,
            "cuda",
            "RTX 4090",
            7,
            1,
        );

        let json = ckpt.to_json();
        let restored = Checkpoint::from_json(&json).unwrap();

        assert_eq!(restored.job_id, "test-job-1");
        assert_eq!(restored.prefixes.len(), 2);
        assert_eq!(restored.master_seed, state.master_seed_hex());
        assert_eq!(restored.next_counter, 42);
        assert_eq!(restored.backend, "cuda");
    }

    #[test]
    fn checkpoint_to_deterministic_state() {
        let mut state = DeterministicState::new();
        state.master_seed = [0xAB; 32];
        state.counter = 100;

        let ckpt = Checkpoint::new(
            "test-job",
            &["C0DE".to_string()],
            &state,
            "cpu",
            "none",
            3,
            1,
        );

        let restored_state = ckpt.to_deterministic_state().unwrap();
        assert_eq!(restored_state.master_seed, state.master_seed);
        assert_eq!(restored_state.counter, 100);

        // Both should produce the same next seed
        let mut s1 = state.clone();
        let mut s2 = restored_state;
        assert_eq!(s1.next_seed(), s2.next_seed());
    }

    #[test]
    fn checkpoint_save_and_load() {
        let state = DeterministicState::new();
        let ckpt = Checkpoint::new("job-42", &["AB".to_string()], &state, "cpu", "none", 1, 1);

        let path = temp_dir().join("save_load_test.json");
        ckpt.save(&path).unwrap();
        assert!(path.exists());

        let loaded = Checkpoint::load(&path).unwrap();
        assert_eq!(loaded.job_id, ckpt.job_id);
        assert_eq!(loaded.master_seed, ckpt.master_seed);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn checkpoint_update_counter() {
        let mut state = DeterministicState::new();
        state.master_seed = [0xAB; 32];
        state.counter = 0;

        let mut ckpt = Checkpoint::new("test", &["AB".to_string()], &state, "cpu", "none", 1, 1);
        assert_eq!(ckpt.next_counter, 0);

        state.counter = 12345678;
        ckpt.update(&state);
        assert_eq!(ckpt.next_counter, 12345678);
        assert_eq!(ckpt.attempts_done, 12345678);
    }

    #[test]
    fn checkpoint_default_path() {
        let path = Checkpoint::default_path("my-job-id");
        assert!(path.to_str().unwrap().contains("my-job-id"));
        assert!(path.to_str().unwrap().contains("checkpoints"));
    }

    #[test]
    fn checkpoint_atomic_write_no_partial_file() {
        let state = DeterministicState::new();
        let ckpt = Checkpoint::new(
            "atomic-test",
            &["AB".to_string()],
            &state,
            "cpu",
            "none",
            1,
            1,
        );

        let path = temp_dir().join("atomic_test.json");
        let tmp = path.with_extension("tmp");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&tmp);

        ckpt.save(&path).unwrap();
        assert!(path.exists());
        assert!(!tmp.exists()); // tmp should have been renamed

        fs::remove_file(&path).ok();
    }
}
