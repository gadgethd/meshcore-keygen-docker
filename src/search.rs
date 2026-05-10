use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rand::rngs::OsRng;
use rand::RngCore;

use crate::checkpoint::Checkpoint;
use crate::deterministic::DeterministicState;
use crate::keygen::generate_keypair;
use crate::types::{MeshCoreKeypair, SearchError, SearchResult, SearchStats};

const BATCH_SIZE: u64 = 1024;
const CHUNK_SIZE: u64 = 1_000_000_000;

/// Result from a single GPU batch dispatch.
pub struct GpuBatchResult {
    pub keys_checked: u64,
    pub keypair: Option<MeshCoreKeypair>,
}

/// Trait abstracting GPU vanity key search backends (Metal, CUDA, etc.).
/// Each implementor owns a single GPU device and is used from one thread.
pub trait GpuSearcher: Send {
    fn search_batch(
        &mut self,
        base_nonce: u64,
    ) -> Result<GpuBatchResult, Box<dyn std::error::Error + Send + Sync>>;

    fn device_name(&self) -> &str;
}

/// Parsed prefix for fast nibble-level matching.
/// Avoids hex-encoding every public key in the hot loop.
pub struct PrefixMatcher {
    /// Full bytes to match (pairs of hex chars).
    pub(crate) full_bytes: Vec<u8>,
    /// If prefix has odd length, the high nibble of the trailing hex char.
    pub(crate) trailing_nibble: Option<u8>,
}

impl PrefixMatcher {
    /// Parse a hex prefix string into a matcher.
    /// Assumes input is already validated as uppercase hex.
    pub fn new(prefix: &str) -> Self {
        let prefix = prefix.to_ascii_uppercase();
        let mut full_bytes = Vec::new();
        let chars: Vec<u8> = prefix.bytes().collect();

        let mut i = 0;
        while i + 1 < chars.len() {
            let hi = nibble_from_ascii(chars[i]);
            let lo = nibble_from_ascii(chars[i + 1]);
            full_bytes.push((hi << 4) | lo);
            i += 2;
        }

        let trailing_nibble = if i < chars.len() {
            Some(nibble_from_ascii(chars[i]))
        } else {
            None
        };

        PrefixMatcher {
            full_bytes,
            trailing_nibble,
        }
    }

    /// Check if a 32-byte public key matches this prefix.
    #[inline(always)]
    pub fn matches(&self, public_key: &[u8; 32]) -> bool {
        // Check full bytes
        for (i, &expected) in self.full_bytes.iter().enumerate() {
            if public_key[i] != expected {
                return false;
            }
        }

        // Check trailing nibble (high nibble of next byte)
        if let Some(nibble) = self.trailing_nibble {
            let idx = self.full_bytes.len();
            if (public_key[idx] >> 4) != nibble {
                return false;
            }
        }

        true
    }
}

fn nibble_from_ascii(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'A'..=b'F' => c - b'A' + 10,
        b'a'..=b'f' => c - b'a' + 10,
        _ => unreachable!("validated hex input"),
    }
}

/// Check if a public key should be skipped (starts with 00 or FF).
#[inline(always)]
fn should_skip(public_key: &[u8; 32]) -> bool {
    public_key[0] == 0x00 || public_key[0] == 0xFF
}

/// Internal result containing keypair + which prefix matched.
struct MatchResult {
    keypair: MeshCoreKeypair,
    matched_prefix: String,
    seed: Option<String>,
    master_seed: Option<String>,
    counter: Option<u64>,
}
/// Handle for a running vanity key search.
/// Exposes atomics so a TUI render loop can poll progress directly.
pub struct SearchHandle {
    found: Arc<AtomicBool>,
    attempts: Arc<AtomicU64>,
    result: Arc<Mutex<Option<MatchResult>>>,
    start: Instant,
    workers: Vec<JoinHandle<()>>,
    checkpoint_path: Option<std::path::PathBuf>,
    checkpoint_interval: Duration,
    deterministic_state: Arc<Mutex<Option<DeterministicState>>>,
}

impl SearchHandle {
    /// Start a vanity key search in background threads.
    pub fn start(prefixes: &[String], num_threads: usize) -> Self {
        let matchers: Arc<Vec<(String, PrefixMatcher)>> = Arc::new(
            prefixes
                .iter()
                .map(|p| (p.clone(), PrefixMatcher::new(p)))
                .collect(),
        );
        let found = Arc::new(AtomicBool::new(false));
        let attempts = Arc::new(AtomicU64::new(0));
        let result: Arc<Mutex<Option<MatchResult>>> = Arc::new(Mutex::new(None));

        let mut workers = Vec::with_capacity(num_threads);
        for _ in 0..num_threads {
            let matchers = Arc::clone(&matchers);
            let found = Arc::clone(&found);
            let total_attempts = Arc::clone(&attempts);
            let result = Arc::clone(&result);

            workers.push(thread::spawn(move || {
                let mut seed = [0u8; 32];
                let mut local_count: u64 = 0;

                while !found.load(Ordering::Acquire) {
                    OsRng.fill_bytes(&mut seed);
                    let kp = generate_keypair(&seed);
                    local_count += 1;

                    if local_count.is_multiple_of(BATCH_SIZE) {
                        total_attempts.fetch_add(BATCH_SIZE, Ordering::Relaxed);
                    }

                    if should_skip(&kp.public_key) {
                        continue;
                    }

                    if let Some(matched) = matchers.iter().find(|(_, m)| m.matches(&kp.public_key))
                    {
                        total_attempts.fetch_add(local_count % BATCH_SIZE, Ordering::Relaxed);
                        found.store(true, Ordering::Release);
                        *result.lock().unwrap() = Some(MatchResult {
                            keypair: kp,
                            matched_prefix: matched.0.clone(),
                            seed: Some(hex::encode(seed).to_uppercase()),
                            master_seed: None,
                            counter: None,
                        });
                        return;
                    }
                }

                total_attempts.fetch_add(local_count % BATCH_SIZE, Ordering::Relaxed);
            }));
        }

        SearchHandle {
            found,
            attempts,
            result,
            start: Instant::now(),
            workers,
            checkpoint_path: None,
            checkpoint_interval: Duration::from_secs(10),
            deterministic_state: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if a match has been found.
    pub fn is_done(&self) -> bool {
        self.found.load(Ordering::Acquire)
    }

    /// Get current search statistics.
    pub fn stats(&self, expected: u64) -> SearchStats {
        let attempts = self.attempts.load(Ordering::Relaxed);
        let elapsed = self.start.elapsed().as_secs_f64();
        SearchStats {
            attempts,
            expected_attempts: expected,
            elapsed_secs: elapsed,
            keys_per_sec: if elapsed > 0.0 {
                attempts as f64 / elapsed
            } else {
                0.0
            },
        }
    }

    /// Join all worker threads and return the result.
    /// Returns `Err` if all workers exited without finding a match (e.g., GPU errors).
    pub fn finish(self) -> Result<SearchResult, SearchError> {
        for h in self.workers {
            h.join().unwrap();
        }

        let elapsed = self.start.elapsed().as_secs_f64();
        let attempts = self.attempts.load(Ordering::Relaxed);

        match self.result.lock().unwrap().take() {
            Some(m) => Ok(SearchResult {
                public_key: hex::encode_upper(m.keypair.public_key),
                private_key: hex::encode_upper(m.keypair.private_key),
                matched_prefix: m.matched_prefix,
                attempts,
                elapsed_secs: elapsed,
                seed: m.seed,
                master_seed: m.master_seed,
                counter: m.counter,
                backend: None,
                device: None,
                job_id: None,
            }),
            None => Err(SearchError {
                attempts,
                elapsed_secs: elapsed,
            }),
        }
    }

    /// Start a deterministic vanity key search using master_seed + counter.
    /// Supports checkpointing and JSON progress output.
    pub fn start_deterministic(
        prefixes: &[String],
        mut state: DeterministicState,
        num_threads: usize,
        checkpoint_path: Option<std::path::PathBuf>,
        checkpoint_interval_secs: u64,
        _json_progress: bool,
    ) -> Self {
        let matchers: Arc<Vec<(String, PrefixMatcher)>> = Arc::new(
            prefixes
                .iter()
                .map(|p| (p.clone(), PrefixMatcher::new(p)))
                .collect(),
        );
        let found = Arc::new(AtomicBool::new(false));
        let attempts = Arc::new(AtomicU64::new(0));
        let result: Arc<Mutex<Option<MatchResult>>> = Arc::new(Mutex::new(None));
        let state_arc = Arc::new(Mutex::new(Some(state.clone())));
        let master_seed_hex = state.master_seed_hex();

        let mut workers = Vec::with_capacity(num_threads);
        for worker_id in 0..num_threads {
            let matchers = Arc::clone(&matchers);
            let found = Arc::clone(&found);
            let total_attempts = Arc::clone(&attempts);
            let result = Arc::clone(&result);
            let state_arc = Arc::clone(&state_arc);

            // Each worker gets a chunk of counters to avoid overlap
            let worker_state = {
                let mut s = state.clone();
                s.worker_id = worker_id as u64;
                s.counter = worker_id as u64 * CHUNK_SIZE;
                s
            };

            workers.push(thread::spawn({
                let master_hex = master_seed_hex.clone();
                move || {
                    let mut local_state = worker_state;
                    let mut local_count: u64 = 0;

                    while !found.load(Ordering::Acquire) {
                        let seed = local_state.next_seed();
                        let kp = generate_keypair(&seed);
                        local_count += 1;

                        if local_count.is_multiple_of(BATCH_SIZE) {
                            total_attempts.fetch_add(BATCH_SIZE, Ordering::Relaxed);
                        }

                        if should_skip(&kp.public_key) {
                            continue;
                        }

                        if let Some(matched) =
                            matchers.iter().find(|(_, m)| m.matches(&kp.public_key))
                        {
                            total_attempts.fetch_add(local_count % BATCH_SIZE, Ordering::Relaxed);
                            found.store(true, Ordering::Release);
                            *result.lock().unwrap() = Some(MatchResult {
                                keypair: kp,
                                matched_prefix: matched.0.clone(),
                                seed: Some(hex::encode_upper(seed)),
                                master_seed: Some(master_hex),
                                counter: Some(local_state.counter - 1),
                            });
                            if let Ok(mut s) = state_arc.lock() {
                                if let Some(ref mut shared) = *s {
                                    shared.counter = local_state.counter;
                                }
                            }
                            return;
                        }
                    }

                    total_attempts.fetch_add(local_count % BATCH_SIZE, Ordering::Relaxed);
                    if let Ok(mut s) = state_arc.lock() {
                        if let Some(ref mut shared) = *s {
                            if local_state.counter > shared.counter {
                                shared.counter = local_state.counter;
                            }
                        }
                    }
                }
            }));
        }

        SearchHandle {
            found,
            attempts,
            result,
            start: Instant::now(),
            workers,
            checkpoint_path,
            checkpoint_interval: Duration::from_secs(checkpoint_interval_secs),
            deterministic_state: state_arc,
        }
    }

    /// Get current deterministic state (for checkpoint saves).
    pub fn get_deterministic_state(&self) -> Option<DeterministicState> {
        self.deterministic_state.lock().ok()?.clone()
    }

    /// Save a checkpoint of the current search state.
    pub fn save_checkpoint(
        &self,
        job_id: &str,
        prefixes: &[String],
        backend: &str,
        device: &str,
        cpu_worker_threads: usize,
        cpu_reserved_cores: usize,
    ) -> Result<(), String> {
        let path = self
            .checkpoint_path
            .clone()
            .unwrap_or_else(|| Checkpoint::default_path(job_id));
        let state = self
            .get_deterministic_state()
            .ok_or("no deterministic state".to_string())?;
        let ckpt = Checkpoint::new(
            job_id,
            prefixes,
            &state,
            backend,
            device,
            cpu_worker_threads,
            cpu_reserved_cores,
        );
        ckpt.save(&path)
    }

    /// Get JSON progress line (emitted per-batch).
    pub fn json_progress(&self, backend: &str, device: &str) -> Option<String> {
        let state = self.get_deterministic_state()?;
        let stats = self.stats(0);
        let progress = serde_json::json!({
            "type": "progress",
            "attempts": stats.attempts,
            "next_counter": state.counter,
            "keys_per_second": stats.keys_per_sec,
            "elapsed_seconds": stats.elapsed_secs,
            "backend": backend,
            "device": device,
        });
        Some(progress.to_string())
    }

    /// JSON result output.
    pub fn json_result(&self) -> Option<String> {
        let m = self.result.lock().unwrap();
        let m = m.as_ref()?;
        let elapsed = self.start.elapsed().as_secs_f64();
        let attempts = self.attempts.load(Ordering::Relaxed);
        let result = serde_json::json!({
            "type": "result",
            "prefix": m.matched_prefix,
            "public_key": hex::encode_upper(m.keypair.public_key),
            "private_key": hex::encode_upper(m.keypair.private_key),
            "candidate_seed": m.seed,
            "master_seed": m.master_seed,
            "counter": m.counter,
            "attempts": attempts,
            "elapsed_seconds": elapsed,
        });
        Some(result.to_string())
    }

    /// Start a GPU-only vanity key search. Spawns one thread per GPU device.
    pub fn start_gpu(prefixes: &[String], gpu_searchers: Vec<Box<dyn GpuSearcher>>) -> Self {
        let matchers: Arc<Vec<(String, PrefixMatcher)>> = Arc::new(
            prefixes
                .iter()
                .map(|p| (p.clone(), PrefixMatcher::new(p)))
                .collect(),
        );
        let found = Arc::new(AtomicBool::new(false));
        let attempts = Arc::new(AtomicU64::new(0));
        let result: Arc<Mutex<Option<MatchResult>>> = Arc::new(Mutex::new(None));

        let mut workers = Vec::with_capacity(gpu_searchers.len());
        for searcher in gpu_searchers {
            workers.push(thread::spawn({
                let matchers = Arc::clone(&matchers);
                let found = Arc::clone(&found);
                let attempts = Arc::clone(&attempts);
                let result = Arc::clone(&result);
                move || gpu_dispatch_loop(searcher, matchers, found, attempts, result)
            }));
        }

        SearchHandle {
            found,
            attempts,
            result,
            start: Instant::now(),
            workers,
            checkpoint_path: None,
            checkpoint_interval: Duration::from_secs(10),
            deterministic_state: Arc::new(Mutex::new(None)),
        }
    }

    /// Start a hybrid vanity key search: CPU threads + GPU devices concurrently.
    pub fn start_hybrid(
        prefixes: &[String],
        cpu_threads: usize,
        gpu_searchers: Vec<Box<dyn GpuSearcher>>,
    ) -> Self {
        let matchers: Arc<Vec<(String, PrefixMatcher)>> = Arc::new(
            prefixes
                .iter()
                .map(|p| (p.clone(), PrefixMatcher::new(p)))
                .collect(),
        );
        let found = Arc::new(AtomicBool::new(false));
        let attempts = Arc::new(AtomicU64::new(0));
        let result: Arc<Mutex<Option<MatchResult>>> = Arc::new(Mutex::new(None));

        let mut workers = Vec::with_capacity(cpu_threads + gpu_searchers.len());

        // Spawn CPU workers
        for _ in 0..cpu_threads {
            let matchers = Arc::clone(&matchers);
            let found = Arc::clone(&found);
            let total_attempts = Arc::clone(&attempts);
            let result = Arc::clone(&result);

            workers.push(thread::spawn(move || {
                let mut seed = [0u8; 32];
                let mut local_count: u64 = 0;

                while !found.load(Ordering::Acquire) {
                    OsRng.fill_bytes(&mut seed);
                    let kp = generate_keypair(&seed);
                    local_count += 1;

                    if local_count.is_multiple_of(BATCH_SIZE) {
                        total_attempts.fetch_add(BATCH_SIZE, Ordering::Relaxed);
                    }

                    if should_skip(&kp.public_key) {
                        continue;
                    }

                    if let Some(matched) = matchers.iter().find(|(_, m)| m.matches(&kp.public_key))
                    {
                        total_attempts.fetch_add(local_count % BATCH_SIZE, Ordering::Relaxed);
                        found.store(true, Ordering::Release);
                        *result.lock().unwrap() = Some(MatchResult {
                            keypair: kp,
                            matched_prefix: matched.0.clone(),
                            seed: Some(hex::encode_upper(seed)),
                            master_seed: None,
                            counter: None,
                        });
                        return;
                    }
                }

                total_attempts.fetch_add(local_count % BATCH_SIZE, Ordering::Relaxed);
            }));
        }

        // Spawn GPU dispatch threads
        for searcher in gpu_searchers {
            workers.push(thread::spawn({
                let matchers = Arc::clone(&matchers);
                let found = Arc::clone(&found);
                let attempts = Arc::clone(&attempts);
                let result = Arc::clone(&result);
                move || gpu_dispatch_loop(searcher, matchers, found, attempts, result)
            }));
        }

        SearchHandle {
            found,
            attempts,
            result,
            start: Instant::now(),
            workers,
            checkpoint_path: None,
            checkpoint_interval: Duration::from_secs(10),
            deterministic_state: Arc::new(Mutex::new(None)),
        }
    }
}

/// GPU dispatch loop shared by start_gpu and start_hybrid.
/// Runs search_batch in a loop until a match is found or another thread signals done.
fn gpu_dispatch_loop(
    mut searcher: Box<dyn GpuSearcher>,
    matchers: Arc<Vec<(String, PrefixMatcher)>>,
    found: Arc<AtomicBool>,
    attempts: Arc<AtomicU64>,
    result: Arc<Mutex<Option<MatchResult>>>,
) {
    let mut nonce_bytes = [0u8; 8];
    OsRng.fill_bytes(&mut nonce_bytes);
    let mut base_nonce: u64 = u64::from_le_bytes(nonce_bytes);

    while !found.load(Ordering::Relaxed) {
        match searcher.search_batch(base_nonce) {
            Ok(batch_result) => {
                attempts.fetch_add(batch_result.keys_checked, Ordering::Relaxed);
                if let Some(kp) = batch_result.keypair {
                    found.store(true, Ordering::Relaxed);
                    let matched_prefix = matchers
                        .iter()
                        .find(|(_, m)| m.matches(&kp.public_key))
                        .map(|(p, _)| p.clone())
                        .unwrap_or_else(|| matchers[0].0.clone());
                    *result.lock().unwrap() = Some(MatchResult {
                        keypair: kp,
                        matched_prefix,
                        seed: None,
                        master_seed: None,
                        counter: None,
                    });
                    return;
                }
                base_nonce = base_nonce.wrapping_add(batch_result.keys_checked);
            }
            Err(e) => {
                eprintln!("GPU error ({}): {}", searcher.device_name(), e);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_matcher_full_bytes() {
        let m = PrefixMatcher::new("AB");
        let mut key = [0u8; 32];
        key[0] = 0xAB;
        assert!(m.matches(&key));

        key[0] = 0xAC;
        assert!(!m.matches(&key));
    }

    #[test]
    fn prefix_matcher_odd_nibble() {
        let m = PrefixMatcher::new("A");
        let mut key = [0u8; 32];
        key[0] = 0xA0;
        assert!(m.matches(&key));

        key[0] = 0xAF;
        assert!(m.matches(&key));

        key[0] = 0xB0;
        assert!(!m.matches(&key));
    }

    #[test]
    fn prefix_matcher_multi_byte() {
        let m = PrefixMatcher::new("ABCD");
        let mut key = [0u8; 32];
        key[0] = 0xAB;
        key[1] = 0xCD;
        assert!(m.matches(&key));

        key[1] = 0xCE;
        assert!(!m.matches(&key));
    }

    #[test]
    fn prefix_matcher_three_nibbles() {
        let m = PrefixMatcher::new("ABC");
        let mut key = [0u8; 32];
        key[0] = 0xAB;
        key[1] = 0xC0;
        assert!(m.matches(&key));

        key[1] = 0xCF;
        assert!(m.matches(&key));

        key[1] = 0xD0;
        assert!(!m.matches(&key));
    }

    #[test]
    fn skip_00_prefix() {
        let mut key = [0u8; 32];
        key[0] = 0x00;
        assert!(should_skip(&key));
    }

    #[test]
    fn skip_ff_prefix() {
        let mut key = [0u8; 32];
        key[0] = 0xFF;
        assert!(should_skip(&key));
    }

    #[test]
    fn no_skip_normal_prefix() {
        let mut key = [0u8; 32];
        key[0] = 0xAB;
        assert!(!should_skip(&key));
    }

    #[test]
    fn prefix_matcher_case_insensitive() {
        let m = PrefixMatcher::new("ab");
        let mut key = [0u8; 32];
        key[0] = 0xAB;
        assert!(m.matches(&key));
    }

    #[test]
    fn search_handle_finds_single_char_prefix() {
        let handle = SearchHandle::start(&["A".to_string()], 2);
        let result = handle.finish().expect("search should find a match");
        assert!(
            result.public_key.starts_with('A'),
            "expected public key starting with A, got {}",
            result.public_key
        );
        assert_eq!(result.matched_prefix, "A");
    }

    #[test]
    fn search_handle_multiple_prefixes() {
        let handle = SearchHandle::start(&["A".to_string(), "B".to_string()], 2);
        let result = handle.finish().expect("search should find a match");
        assert!(
            result.public_key.starts_with('A') || result.public_key.starts_with('B'),
            "expected public key starting with A or B, got {}",
            result.public_key
        );
        assert!(
            result.matched_prefix == "A" || result.matched_prefix == "B",
            "expected matched_prefix A or B, got {}",
            result.matched_prefix
        );
    }
}
