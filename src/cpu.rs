use serde::{Deserialize, Serialize};

/// CPU and worker configuration for resource reservation.
///
/// Ensures the web UI and container services always have at least one
/// logical CPU core available, preventing the keygen from starving
/// the API, scheduler, database, and logging.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CpuConfig {
    /// Total logical CPU cores detected on the system.
    pub total_logical_cores: usize,
    /// Cores reserved for web UI / container services (default 1).
    pub reserved_cores: usize,
    /// Optional override for max CPU worker threads.
    /// If None, uses total_logical_cores - reserved_cores.
    pub max_worker_threads: Option<usize>,
}

impl CpuConfig {
    /// Detect CPU resources and apply default reservation.
    pub fn detect() -> Self {
        let total = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let reserved = std::env::var("RESERVED_CPU_CORES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let max_workers = std::env::var("MAX_CPU_WORKERS")
            .ok()
            .and_then(|v| v.parse().ok());

        CpuConfig {
            total_logical_cores: total,
            reserved_cores: reserved.min(total.saturating_sub(1)),
            max_worker_threads: max_workers,
        }
    }

    /// Number of CPU worker threads available for keygen.
    pub fn available_workers(&self) -> usize {
        if let Some(max) = self.max_worker_threads {
            return max.min(self.total_logical_cores.saturating_sub(self.reserved_cores));
        }
        self.total_logical_cores.saturating_sub(self.reserved_cores)
    }

    /// Whether we have enough cores to run CPU workers at all.
    pub fn can_run_cpu_workers(&self) -> bool {
        self.available_workers() > 0
    }

    /// Human-readable summary for display.
    pub fn summary(&self) -> String {
        format!(
            "total={}, reserved={}, workers={}",
            self.total_logical_cores,
            self.reserved_cores,
            self.available_workers()
        )
    }
}

impl Default for CpuConfig {
    fn default() -> Self {
        Self::detect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_reserves_at_least_one_core() {
        let config = CpuConfig {
            total_logical_cores: 8,
            reserved_cores: 1,
            max_worker_threads: None,
        };
        assert_eq!(config.available_workers(), 7);
        assert!(config.can_run_cpu_workers());
    }

    #[test]
    fn single_core_machine_has_zero_workers() {
        let config = CpuConfig {
            total_logical_cores: 1,
            reserved_cores: 1,
            max_worker_threads: None,
        };
        assert_eq!(config.available_workers(), 0);
        assert!(!config.can_run_cpu_workers());
    }

    #[test]
    fn cannot_reserve_more_than_total_minus_one() {
        let config = CpuConfig {
            total_logical_cores: 4,
            reserved_cores: 4, // too high
            max_worker_threads: None,
        };
        // available_workers floors at 0
        assert_eq!(config.available_workers(), 0);
    }

    #[test]
    fn max_worker_threads_override() {
        let config = CpuConfig {
            total_logical_cores: 8,
            reserved_cores: 1,
            max_worker_threads: Some(3),
        };
        assert_eq!(config.available_workers(), 3);
    }

    #[test]
    fn max_worker_threads_capped_by_available() {
        let config = CpuConfig {
            total_logical_cores: 8,
            reserved_cores: 1,
            max_worker_threads: Some(100), // too high
        };
        assert_eq!(config.available_workers(), 7);
    }

    #[test]
    fn reserve_zero_uses_all_cores() {
        let config = CpuConfig {
            total_logical_cores: 8,
            reserved_cores: 0,
            max_worker_threads: None,
        };
        assert_eq!(config.available_workers(), 8);
    }

    #[test]
    fn detect_returns_valid_config() {
        let config = CpuConfig::detect();
        assert!(config.total_logical_cores >= 1);
        // Reserved must not consume all cores
        assert!(config.available_workers() <= config.total_logical_cores);
    }

    #[test]
    fn summary_contains_expected_fields() {
        let config = CpuConfig {
            total_logical_cores: 8,
            reserved_cores: 1,
            max_worker_threads: None,
        };
        let summary = config.summary();
        assert!(summary.contains("total=8"));
        assert!(summary.contains("reserved=1"));
        assert!(summary.contains("workers=7"));
    }

    #[test]
    fn serialization_roundtrip() {
        let config = CpuConfig {
            total_logical_cores: 8,
            reserved_cores: 2,
            max_worker_threads: Some(4),
        };
        let json = serde_json::to_string(&config).unwrap();
        let restored: CpuConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.total_logical_cores, 8);
        assert_eq!(restored.reserved_cores, 2);
        assert_eq!(restored.max_worker_threads, Some(4));
        assert_eq!(restored.available_workers(), 4);
    }
}
