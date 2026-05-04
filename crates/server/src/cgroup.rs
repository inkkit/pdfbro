// crates/server/src/cgroup.rs
//! Cgroup-aware resource detection for Docker containers.
//! Reads CPU and memory limits from cgroup v1 or v2 filesystems.

use std::fs;
use std::path::Path;

/// Detected cgroup version and limits.
#[derive(Debug, Clone)]
pub struct CgroupLimits {
    /// CPU core limit (None = unlimited).
    pub cpu_limit_cores: Option<f64>,
    /// Memory limit in MB (None = unlimited).
    pub memory_limit_mb: Option<f64>,
    /// Current memory usage in MB.
    pub memory_used_mb: Option<f64>,
    /// Whether running in a container (cgroup limits detected).
    pub is_container: bool,
}

impl CgroupLimits {
    /// Detect cgroup limits from the filesystem.
    pub fn detect() -> Self {
        // Try cgroup v2 first, then v1
        if Path::new("/sys/fs/cgroup/cgroup.controllers").exists() {
            Self::detect_v2()
        } else {
            Self::detect_v1()
        }
    }

    fn detect_v2() -> Self {
        let cpu_limit = Self::read_cpu_max_v2();
        let memory_limit = Self::read_memory_max_v2();
        let memory_used = Self::read_memory_current_v2();

        Self {
            cpu_limit_cores: cpu_limit,
            memory_limit_mb: memory_limit.map(|m| m as f64 / 1024.0 / 1024.0),
            memory_used_mb: memory_used.map(|m| m as f64 / 1024.0 / 1024.0),
            is_container: cpu_limit.is_some() || memory_limit.is_some(),
        }
    }

    fn detect_v1() -> Self {
        let cpu_limit = Self::read_cpu_quota_v1();
        let memory_limit = Self::read_memory_limit_v1();
        let memory_used = Self::read_memory_usage_v1();

        Self {
            cpu_limit_cores: cpu_limit,
            memory_limit_mb: memory_limit.map(|m| m as f64 / 1024.0 / 1024.0),
            memory_used_mb: memory_used.map(|m| m as f64 / 1024.0 / 1024.0),
            is_container: cpu_limit.is_some() || memory_limit.is_some(),
        }
    }

    /// Read CPU limit from cgroup v2 (format: "quota period" or "max 100000")
    fn read_cpu_max_v2() -> Option<f64> {
        let content = fs::read_to_string("/sys/fs/cgroup/cpu.max").ok()?;
        let parts: Vec<&str> = content.trim().split_whitespace().collect();
        if parts.len() >= 2 && parts[0] != "max" {
            let quota: f64 = parts[0].parse().ok()?;
            let period: f64 = parts[1].parse().ok()?;
            if period > 0.0 {
                return Some(quota / period);
            }
        }
        None
    }

    /// Read memory limit from cgroup v2 (format: "123456789" or "max")
    fn read_memory_max_v2() -> Option<u64> {
        let content = fs::read_to_string("/sys/fs/cgroup/memory.max").ok()?;
        if content.trim() == "max" {
            return None;
        }
        content.trim().parse().ok()
    }

    /// Read memory usage from cgroup v2
    fn read_memory_current_v2() -> Option<u64> {
        fs::read_to_string("/sys/fs/cgroup/memory.current")
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    /// Read CPU quota from cgroup v1
    fn read_cpu_quota_v1() -> Option<f64> {
        let quota: i64 = fs::read_to_string("/sys/fs/cgroup/cpu/cpu.cfs_quota_us")
            .ok()
            .and_then(|s| s.trim().parse().ok())?;
        
        if quota < 0 {
            return None; // unlimited
        }

        let period: i64 = fs::read_to_string("/sys/fs/cgroup/cpu/cpu.cfs_period_us")
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(100000);

        if period > 0 {
            Some(quota as f64 / period as f64)
        } else {
            None
        }
    }

    /// Read memory limit from cgroup v1
    fn read_memory_limit_v1() -> Option<u64> {
        let content = fs::read_to_string("/sys/fs/cgroup/memory/memory.limit_in_bytes").ok()?;
        // In cgroup v1, a very large number (like 9223372036854771712) means unlimited
        let val: u64 = content.trim().parse().ok()?;
        if val > 1u64 << 60 {
            return None;
        }
        Some(val)
    }

    /// Read memory usage from cgroup v1
    fn read_memory_usage_v1() -> Option<u64> {
        fs::read_to_string("/sys/fs/cgroup/memory/memory.usage_in_bytes")
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    /// Calculate CPU percentage relative to cgroup limit.
    /// Returns the percentage of the container's CPU quota being used.
    pub fn cpu_pct_relative_to_limit(&self, host_cpu_pct: f64, num_host_cpus: usize) -> f64 {
        match self.cpu_limit_cores {
            Some(limit) if limit > 0.0 => {
                // Scale host CPU% to container's perspective
                // host_cpu_pct is already a percentage of total host CPUs
                // We need to scale it by (num_host_cpus / cpu_limit)
                let scale_factor = num_host_cpus as f64 / limit;
                (host_cpu_pct * scale_factor).min(100.0 * scale_factor)
            }
            _ => host_cpu_pct, // No limit, use host percentage
        }
    }

    /// Get memory usage percentage relative to cgroup limit.
    pub fn memory_pct(&self) -> f64 {
        match (self.memory_used_mb, self.memory_limit_mb) {
            (Some(used), Some(limit)) if limit > 0.0 => {
                (used / limit * 100.0).min(100.0)
            }
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_pct_calculation() {
        let limits = CgroupLimits {
            cpu_limit_cores: Some(2.0),
            memory_limit_mb: None,
            memory_used_mb: None,
            is_container: true,
        };

        // On a 4-core host, if sysinfo reports 25% CPU (1 core fully used)
        // That should be 50% of the container's 2-core limit
        let host_pct = 25.0;
        let num_host_cpus = 4;
        assert_eq!(limits.cpu_pct_relative_to_limit(host_pct, num_host_cpus), 50.0);
    }

    #[test]
    fn test_memory_pct_calculation() {
        let limits = CgroupLimits {
            cpu_limit_cores: None,
            memory_limit_mb: Some(1024.0),
            memory_used_mb: Some(512.0),
            is_container: true,
        };
        assert_eq!(limits.memory_pct(), 50.0);
    }
}
