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

pub fn write(results: &[WorkloadResult], output_dir: &PathBuf, bench_mode: &str) -> anyhow::Result<PathBuf> {
    fs::create_dir_all(output_dir)?;
    let path = output_dir.join("perf.md");

    let mut md = String::new();
    md.push_str("# pdfbro vs Gotenberg — Performance Report\n\n");
    md.push_str(&format!(
        "Generated: {}  \nMode: `{}`\n\n",
        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
        bench_mode,
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
    md.push_str("> **How to read this table:**  \n");
    md.push_str("> RSS is sampled via `docker stats` — it measures the **entire container** (server process +\n");
    md.push_str("> Chrome + LibreOffice + all children), not just the operation under test.\n");
    md.push_str(">\n");
    match bench_mode {
        m if m.starts_with("isolated") => {
            md.push_str("> Mode **isolated**: containers were restarted before each workload, so each row\n");
            md.push_str("> reflects that engine's steady-state memory for that workload only — no carryover\n");
            md.push_str("> from previous operations.\n");
        }
        m if m.starts_with("accumulated") => {
            md.push_str("> Mode **accumulated**: containers ran warm throughout. Later workloads show higher\n");
            md.push_str("> numbers because Chrome and LibreOffice from earlier workloads remain alive in the\n");
            md.push_str("> container. The merge row includes both Chrome and LibreOffice baseline RSS even\n");
            md.push_str("> though neither is used for a pure PDF merge.\n");
        }
        _ => {}
    }
    md.push('\n');

    md.push_str("| Workload | pdfbro | Gotenberg | Winner |\n");
    md.push_str("|----------|--------|-----------|--------|\n");
    for r in results {
        let pdfbro_rss = r.pdfbro.peak_rss_mib;
        let gotenberg_rss = r.gotenberg.peak_rss_mib;
        let winner = match (pdfbro_rss, gotenberg_rss) {
            (Some(p), Some(g)) if p < g => {
                let pct = ((g as f64 - p as f64) / g as f64 * 100.0) as i64;
                format!("pdfbro (−{pct}%)")
            }
            (Some(p), Some(g)) if g < p => {
                let pct = ((p as f64 - g as f64) / p as f64 * 100.0) as i64;
                format!("Gotenberg (−{pct}%)")
            }
            (Some(_), Some(_)) => "tie".to_string(),
            _ => "N/A".to_string(),
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            r.workload,
            pdfbro_rss.map_or("N/A".to_string(), |v| v.to_string()),
            gotenberg_rss.map_or("N/A".to_string(), |v| v.to_string()),
            winner,
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
    md.push_str("- Both servers ran under `cpus: \"2\"` / `memory: 2g` Docker resource limits.\n");
    md.push_str("- Chrome PDF rendering is non-deterministic; latency varies across runs.\n");
    md.push_str("- 60-second warm-up discarded before measurements.\n");
    md.push_str("- RSS = `docker stats` memory — includes all child processes in the container.\n");
    md.push_str("  pdfbro keeps Chrome and LibreOffice alive (warm engines); Gotenberg may recycle\n");
    md.push_str("  them (default: restart Chrome every 100 requests, LibreOffice every 10 requests).\n");
    md.push_str("  Use `--isolated` mode and/or the `steady-state` compose override for fair\n");
    md.push_str("  apples-to-apples memory comparisons.\n");

    fs::write(&path, &md)?;
    Ok(path)
}
