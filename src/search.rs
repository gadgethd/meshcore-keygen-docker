use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use rand::rngs::OsRng;
use rand::RngCore;

use crate::keygen::generate_keypair;
use crate::types::{MeshCoreKeypair, SearchResult, SearchStats};

const BATCH_SIZE: u64 = 1024;

/// Parsed prefix for fast nibble-level matching.
/// Avoids hex-encoding every public key in the hot loop.
pub struct PrefixMatcher {
    /// Full bytes to match (pairs of hex chars).
    full_bytes: Vec<u8>,
    /// If prefix has odd length, the high nibble of the trailing hex char.
    trailing_nibble: Option<u8>,
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

/// Run the vanity key search with the given number of threads.
///
/// Returns a `SearchResult` when a matching key is found.
/// Calls `on_progress` periodically with live stats.
pub fn search(
    prefix: &str,
    num_threads: usize,
    on_progress: impl Fn(&SearchStats) + Send + Sync + 'static,
    progress_interval_secs: f64,
) -> SearchResult {
    let matcher = Arc::new(PrefixMatcher::new(prefix));
    let found = Arc::new(AtomicBool::new(false));
    let total_attempts = Arc::new(AtomicU64::new(0));
    let start = Instant::now();

    // Storage for the result keypair — shared via mutex
    let result: Arc<std::sync::Mutex<Option<MeshCoreKeypair>>> =
        Arc::new(std::sync::Mutex::new(None));

    // Spawn progress reporter thread
    let progress_found = Arc::clone(&found);
    let progress_attempts = Arc::clone(&total_attempts);
    let progress_handle = thread::spawn(move || {
        let interval = std::time::Duration::from_secs_f64(progress_interval_secs);
        while !progress_found.load(Ordering::Relaxed) {
            thread::sleep(interval);
            if progress_found.load(Ordering::Relaxed) {
                break;
            }
            let attempts = progress_attempts.load(Ordering::Relaxed);
            let elapsed = start.elapsed().as_secs_f64();
            on_progress(&SearchStats {
                attempts,
                elapsed_secs: elapsed,
                keys_per_sec: if elapsed > 0.0 {
                    attempts as f64 / elapsed
                } else {
                    0.0
                },
            });
        }
    });

    // Spawn worker threads
    let mut handles = Vec::with_capacity(num_threads);
    for _ in 0..num_threads {
        let matcher = Arc::clone(&matcher);
        let found = Arc::clone(&found);
        let total_attempts = Arc::clone(&total_attempts);
        let result = Arc::clone(&result);

        handles.push(thread::spawn(move || {
            let mut seed = [0u8; 32];
            let mut local_count: u64 = 0;

            while !found.load(Ordering::Relaxed) {
                OsRng.fill_bytes(&mut seed);
                let kp = generate_keypair(&seed);
                local_count += 1;

                // Batch atomic counter updates
                if local_count % BATCH_SIZE == 0 {
                    total_attempts.fetch_add(BATCH_SIZE, Ordering::Relaxed);
                }

                if should_skip(&kp.public_key) {
                    continue;
                }

                if matcher.matches(&kp.public_key) {
                    // Flush remaining count
                    total_attempts.fetch_add(local_count % BATCH_SIZE, Ordering::Relaxed);
                    found.store(true, Ordering::Relaxed);
                    *result.lock().unwrap() = Some(kp);
                    return;
                }
            }

            // Flush remaining count on exit
            total_attempts.fetch_add(local_count % BATCH_SIZE, Ordering::Relaxed);
        }));
    }

    // Wait for all workers
    for h in handles {
        h.join().unwrap();
    }

    // Signal progress thread to stop and wait
    found.store(true, Ordering::Relaxed);
    let _ = progress_handle.join();

    let elapsed = start.elapsed().as_secs_f64();
    let attempts = total_attempts.load(Ordering::Relaxed);
    let kp = result.lock().unwrap().take().expect("search found a key");

    SearchResult {
        public_key: hex::encode_upper(kp.public_key),
        private_key: hex::encode_upper(kp.private_key),
        attempts,
        elapsed_secs: elapsed,
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
    fn search_finds_single_char_prefix() {
        let result = search("A", 2, |_| {}, 60.0);
        assert!(
            result.public_key.starts_with('A'),
            "expected public key starting with A, got {}",
            result.public_key
        );
    }
}
