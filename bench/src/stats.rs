use hdrhistogram::Histogram;

pub struct LatencyStats {
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub rps: f64,
    pub error_rate: f64,
    pub cv: f64,
}

pub fn compute(
    durations_ms: &[u64],
    error_count: usize,
    elapsed_secs: f64,
) -> anyhow::Result<LatencyStats> {
    let total = durations_ms.len() + error_count;
    if durations_ms.is_empty() {
        anyhow::bail!("no successful requests to compute stats");
    }

    let mut hist = Histogram::<u64>::new_with_max(60_000, 3)?;
    for &d in durations_ms {
        hist.record(d)?;
    }

    let mean = durations_ms.iter().sum::<u64>() as f64 / durations_ms.len() as f64;
    let variance = durations_ms
        .iter()
        .map(|&d| {
            let diff = d as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / durations_ms.len() as f64;
    let stddev = variance.sqrt();
    let cv = if mean > 0.0 { stddev / mean * 100.0 } else { 0.0 };

    Ok(LatencyStats {
        p50_ms: hist.value_at_quantile(0.50),
        p95_ms: hist.value_at_quantile(0.95),
        p99_ms: hist.value_at_quantile(0.99),
        rps: total as f64 / elapsed_secs,
        error_rate: error_count as f64 / total as f64,
        cv,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_basic_stats() {
        let durations = vec![100u64, 200, 300, 400, 500];
        let stats = compute(&durations, 0, 5.0).unwrap();
        assert_eq!(stats.p50_ms, 300);
        assert!(stats.rps > 0.0);
        assert_eq!(stats.error_rate, 0.0);
    }

    #[test]
    fn compute_error_rate() {
        let durations = vec![100u64, 200];
        let stats = compute(&durations, 1, 3.0).unwrap();
        assert!((stats.error_rate - 1.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn compute_cv_flag() {
        let durations: Vec<u64> = (1..=100).map(|i| i * 10).collect();
        let stats = compute(&durations, 0, 100.0).unwrap();
        assert!(stats.cv > 15.0, "expected high CV for linearly spaced data");
    }
}
