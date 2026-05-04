use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// State for deterministic vanity key search.
///
/// candidate_seed = SHA-256(master_seed || worker_id || counter)
///
/// This ensures the same master_seed + counter always produces the same key,
/// enabling true pause/resume with no duplicate work.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeterministicState {
    /// 32-byte master seed, randomly generated once at job creation.
    pub master_seed: [u8; 32],
    /// Worker ID for multi-worker setups (avoids overlap).
    pub worker_id: u64,
    /// Monotonically increasing counter.
    pub counter: u64,
}

impl DeterministicState {
    /// Create a new deterministic state with a random master seed.
    pub fn new() -> Self {
        let mut master_seed = [0u8; 32];
        OsRng.fill_bytes(&mut master_seed);
        DeterministicState {
            master_seed,
            worker_id: 0,
            counter: 0,
        }
    }

    /// Create a deterministic state from a hex-encoded master seed.
    pub fn from_hex_seed(hex: &str) -> Result<Self, String> {
        let bytes = hex::decode(hex).map_err(|e| format!("invalid hex seed: {}", e))?;
        if bytes.len() != 32 {
            return Err(format!(
                "master seed must be 32 bytes (64 hex chars), got {} bytes",
                bytes.len()
            ));
        }
        let mut master_seed = [0u8; 32];
        master_seed.copy_from_slice(&bytes);
        Ok(DeterministicState {
            master_seed,
            worker_id: 0,
            counter: 0,
        })
    }

    /// Get the hex-encoded master seed.
    pub fn master_seed_hex(&self) -> String {
        hex::encode_upper(self.master_seed)
    }

    /// Derive the next candidate seed and increment the counter.
    ///
    /// candidate_seed = SHA-256(master_seed || worker_id || counter)
    pub fn next_seed(&mut self) -> [u8; 32] {
        let seed = derive_seed(&self.master_seed, self.worker_id, self.counter);
        self.counter = self.counter.wrapping_add(1);
        seed
    }

    /// Peek at what the next seed would be without incrementing the counter.
    pub fn peek_seed(&self) -> [u8; 32] {
        derive_seed(&self.master_seed, self.worker_id, self.counter)
    }

    /// Set the counter to resume from a specific point.
    pub fn set_counter(&mut self, counter: u64) {
        self.counter = counter;
    }

    /// Generate a batch of seeds. More efficient than calling next_seed() N times.
    pub fn next_seeds<const N: usize>(&mut self) -> [[u8; 32]; N] {
        let mut seeds = [[0u8; 32]; N];
        for (i, seed) in seeds.iter_mut().enumerate() {
            *seed = derive_seed(&self.master_seed, self.worker_id, self.counter);
            self.counter = self.counter.wrapping_add(1);
        }
        seeds
    }
}

/// Derive a candidate seed from master_seed, worker_id, and counter.
///
/// candidate_seed = SHA-256(master_seed || worker_id || counter)
fn derive_seed(master_seed: &[u8; 32], worker_id: u64, counter: u64) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(master_seed);
    hasher.update(&worker_id.to_le_bytes());
    hasher.update(&counter.to_le_bytes());
    let result = hasher.finalize();
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&result);
    seed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_master_seed_counter_gives_same_candidate_seed() {
        let mut state = DeterministicState::new();
        // Set a fixed master for determinism in tests
        state.master_seed = [0xAB; 32];
        state.counter = 0;

        let seed1 = state.next_seed();
        state.counter = 0;
        let seed2 = state.next_seed();
        assert_eq!(seed1, seed2);
    }

    #[test]
    fn different_counter_gives_different_candidate_seed() {
        let mut state = DeterministicState::new();
        state.master_seed = [0xAB; 32];

        state.counter = 0;
        let seed0 = state.next_seed();
        state.counter = 1;
        let seed1 = state.next_seed();
        assert_ne!(seed0, seed1);
    }

    #[test]
    fn different_master_seed_gives_different_seeds() {
        let mut state_a = DeterministicState::new();
        let mut state_b = DeterministicState::new();
        state_a.master_seed = [0xAB; 32];
        state_b.master_seed = [0xCD; 32];
        state_a.counter = 0;
        state_b.counter = 0;

        assert_ne!(state_a.next_seed(), state_b.next_seed());
    }

    #[test]
    fn different_worker_id_gives_different_seeds() {
        let mut state_a = DeterministicState::new();
        let mut state_b = DeterministicState::new();
        state_a.master_seed = [0xAB; 32];
        state_b.master_seed = [0xAB; 32];
        state_a.worker_id = 0;
        state_b.worker_id = 1;
        state_a.counter = 0;
        state_b.counter = 0;

        assert_ne!(state_a.next_seed(), state_b.next_seed());
    }

    #[test]
    fn resume_uses_same_master_seed() {
        let mut state = DeterministicState::new();
        state.master_seed = [0xAB; 32];
        state.counter = 42;

        let seed_at_42 = state.next_seed();

        // Simulate pause and resume
        let master_hex = state.master_seed_hex();
        let mut resumed = DeterministicState::from_hex_seed(&master_hex).unwrap();
        resumed.set_counter(42);

        assert_eq!(resumed.next_seed(), seed_at_42);
    }

    #[test]
    fn restart_generates_new_master_seed() {
        let state1 = DeterministicState::new();
        let state2 = DeterministicState::new();
        // Extremely unlikely to get the same random 32-byte seed
        assert_ne!(state1.master_seed, state2.master_seed);
    }

    #[test]
    fn duplicate_generates_new_master_seed() {
        let state1 = DeterministicState::new();
        let state2 = DeterministicState::new();
        assert_ne!(state1.master_seed, state2.master_seed);
    }

    #[test]
    fn counter_sequence_is_deterministic() {
        let mut state = DeterministicState::new();
        state.master_seed = [0x42; 32];
        state.counter = 0;

        let seq1: Vec<[u8; 32]> = (0..10).map(|_| state.next_seed()).collect();

        state.counter = 0;
        let seq2: Vec<[u8; 32]> = (0..10).map(|_| state.next_seed()).collect();

        assert_eq!(seq1, seq2);
    }

    #[test]
    fn checkpoint_serialization_roundtrip() {
        let state = DeterministicState {
            master_seed: [0xAB; 32],
            worker_id: 7,
            counter: 12345678901234567890,
        };

        let json = serde_json::to_string(&state).unwrap();
        let restored: DeterministicState = serde_json::from_str(&json).unwrap();

        assert_eq!(state.master_seed, restored.master_seed);
        assert_eq!(state.worker_id, restored.worker_id);
        assert_eq!(state.counter, restored.counter);

        // Seeds should match
        let mut state_clone = state.clone();
        let mut restored_clone = restored.clone();
        assert_eq!(state_clone.next_seed(), restored_clone.next_seed());
    }

    #[test]
    fn master_seed_hex_roundtrip() {
        let state = DeterministicState::new();
        let hex = state.master_seed_hex();
        assert_eq!(hex.len(), 64);
        let restored = DeterministicState::from_hex_seed(&hex).unwrap();
        assert_eq!(state.master_seed, restored.master_seed);
    }

    #[test]
    fn from_hex_seed_rejects_invalid_length() {
        assert!(DeterministicState::from_hex_seed("DEAD").is_err());
        assert!(DeterministicState::from_hex_seed("").is_err());
    }

    #[test]
    fn from_hex_seed_rejects_non_hex() {
        assert!(DeterministicState::from_hex_seed(&"GH".repeat(32)).is_err());
    }
}
