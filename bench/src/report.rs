use std::fs;
use std::path::PathBuf;
use crate::stats::LatencyStats;

pub struct WorkloadResult {
    pub workload: String,
    pub pdfbro: RunResult,
    pub gotenberg: RunResult,
}

pub struct RunResult {
    pub stats: LatencyStats,
    pub peak_rss_mib: Option<u64>,
    pub repetitions: Vec<LatencyStats>,
}

pub fn write(results: &[WorkloadResult], output_dir: &PathBuf) -> anyhow::Result<PathBuf> {
    fs::create_dir_all(output_dir)?;
    let path = output_dir.join("perf.md");

    let mut md = String::new();
    md.push_str("# pdfbro vs Gotenberg — Performance Report\n\n");
    md.push_str(&format!(
        "Generated: {}\n\n",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ")
    ));

    md.push_str("## Latency (ms)\n\n");
    md.push_str("| Workload | Server | p50 | p95 | p99 | RPS | Errors |\n");
    md.push_str("|----------|--------|-----|-----|-----|-----|--------|\n");
    for r in results {
        md.push_str(&format!(
            "| {} | pdfbro | {} | {} | {} | {:.1} | {:.1}% |\n",
            r.workload, r.pdfbro.stats.p50_ms, r.pdfbro.stats.p95_ms,
            r.pdfbro.stats.p99_ms, r.pdfbro.stats.rps, r.pdfbro.stats.error_rate * 100.0,
        ));
        md.push_str(&format!(
            "| {} | gotenberg | {} | {} | {} | {:.1} | {:.1}% |\n",
            r.workload, r.gotenberg.stats.p50_ms, r.gotenberg.stats.p95_ms,
            r.gotenberg.stats.p99_ms, r.gotenberg.stats.rps, r.gotenberg.stats.error_rate * 100.0,
        ));
    }

    md.push_str("\n## Peak RSS (MiB)\n\n");
    md.push_str("| Workload | pdfbro | Gotenberg |\n");
    md.push_str("|----------|-------|-----------|\n");
    for r in results {
        md.push_str(&format!(
            "| {} | {} | {} |\n",
            r.workload,
            r.pdfbro.peak_rss_mib.map_or("N/A".to_string(), |v| v.to_string()),
            r.gotenberg.peak_rss_mib.map_or("N/A".to_string(), |v| v.to_string()),
        ));
    }

    let mut cv_warnings = Vec::new();
    for r in results {
        if r.pdfbro.stats.cv > 15.0 {
            cv_warnings.push(format!("- pdfbro/{}: CV={:.1}% (unstable)", r.workload, r.pdfbro.stats.cv));
        }
        if r.gotenberg.stats.cv > 15.0 {
            cv_warnings.push(format!("- gotenberg/{}: CV={:.1}% (unstable)", r.workload, r.gotenberg.stats.cv));
        }
    }
    if !cv_warnings.is_empty() {
        md.push_str("\n## Stability Warnings (CV > 15%)\n\n");
        md.push_str(&cv_warnings.join("\n"));
        md.push('\n');
    }

    md.push_str("\n## Caveats\n\n");
    md.push_str("- Results are hardware-specific and not portable across machines.\n");
    md.push_str("- Both servers ran under `cpus: \"2\"` / `mem_limit: 2g` Docker cgroups.\n");
    md.push_str("- Chrome PDF rendering is non-deterministic; latency varies across runs.\n");
    md.push_str("- 60-second warm-up discarded before measurements.\n");

    fs::write(&path, &md)?;
    Ok(path)
}
