use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use clap::Args;
use chrono::Utc;

use crate::{driver, preflight, quality, report, rss, stats, workload};

#[derive(Args)]
pub struct PerfArgs {
    #[arg(long, default_value = "http://localhost:3001")]
    pub pdfbro_url: String,
    #[arg(long, default_value = "http://localhost:3002")]
    pub gotenberg_url: String,
    #[arg(long, default_value = "bench-pdfbro")]
    pub pdfbro_container: String,
    #[arg(long, default_value = "bench-gotenberg")]
    pub gotenberg_container: String,
    #[arg(long, default_value_t = 4)]
    pub concurrency: usize,
    #[arg(long, default_value_t = 60)]
    pub warmup_secs: u64,
    #[arg(long, default_value_t = 120)]
    pub duration_secs: u64,
    #[arg(long, default_value_t = 3)]
    pub repetitions: usize,
    #[arg(long)]
    pub skip_preflight: bool,
    #[arg(long)]
    pub output_dir: Option<PathBuf>,
    /// Comma-separated workload names to skip (e.g. --skip url-local).
    #[arg(long, value_delimiter = ',')]
    pub skip: Vec<String>,
    /// Restart both containers before each workload so each measurement starts
    /// from a clean-slate container with no accumulated engine state.
    #[arg(long)]
    pub isolated: bool,
}

pub async fn run_perf(args: PerfArgs) -> anyhow::Result<()> {
    if !args.skip_preflight {
        preflight::check(&args.pdfbro_container, &args.gotenberg_container)?;
    }

    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let output_dir = args.output_dir.unwrap_or_else(|| {
        PathBuf::from("bench/results").join(&timestamp)
    });

    let bench_mode = if args.isolated {
        "isolated (container restarted before each workload)"
    } else {
        "accumulated (containers warm throughout)"
    };
    println!("Bench mode: {bench_mode}");

    let workloads = workload::all_workloads();
    let mut all_results = Vec::new();

    for w in &workloads {
        if args.skip.iter().any(|s| s == w.name) {
            println!("\n=== {} — skipped ===", w.name);
            continue;
        }
        println!("\n=== {} — {} ===", w.name, w.description);

        if args.isolated {
            println!("  restarting containers...");
            restart_and_wait(
                &args.pdfbro_container, &args.pdfbro_url,
                &args.gotenberg_container, &args.gotenberg_url,
            ).await?;
        }

        let pdfbro_result = run_workload(
            w, &args.pdfbro_url, w.pdfbro_route, &args.pdfbro_container,
            args.concurrency, args.warmup_secs, args.duration_secs, args.repetitions,
        ).await?;

        let gotenberg_result = run_workload(
            w, &args.gotenberg_url, w.gotenberg_route, &args.gotenberg_container,
            args.concurrency, args.warmup_secs, args.duration_secs, args.repetitions,
        ).await?;

        all_results.push(report::WorkloadResult {
            workload: w.name.to_string(),
            pdfbro: pdfbro_result,
            gotenberg: gotenberg_result,
        });
    }

    let path = report::write(&all_results, &output_dir, bench_mode)?;
    println!("\nReport written to: {}", path.display());
    Ok(())
}

/// Restart both containers and wait until their /health endpoints respond.
async fn restart_and_wait(
    pdfbro_container: &str,
    pdfbro_url: &str,
    gotenberg_container: &str,
    gotenberg_url: &str,
) -> anyhow::Result<()> {
    docker_restart(pdfbro_container)?;
    docker_restart(gotenberg_container)?;
    wait_healthy(pdfbro_url, 120).await?;
    wait_healthy(gotenberg_url, 120).await?;
    Ok(())
}

fn docker_restart(container: &str) -> anyhow::Result<()> {
    let status = std::process::Command::new("docker")
        .args(["restart", container])
        .status()?;
    if !status.success() {
        anyhow::bail!("docker restart {container} failed");
    }
    Ok(())
}

async fn wait_healthy(base_url: &str, timeout_secs: u64) -> anyhow::Result<()> {
    let url = format!("{base_url}/health");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for {url} to become healthy");
        }
        match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => return Ok(()),
            _ => tokio::time::sleep(Duration::from_secs(2)).await,
        }
    }
}

async fn run_workload(
    w: &workload::WorkloadDef,
    base_url: &str,
    route: &str,
    container_name: &str,
    concurrency: usize,
    warmup_secs: u64,
    duration_secs: u64,
    repetitions: usize,
) -> anyhow::Result<report::RunResult> {
    let url = format!("{}{}", base_url, route);

    // Warm-up
    println!("  warm-up {}s...", warmup_secs);
    drive_once(w, &url, concurrency, Duration::from_secs(warmup_secs)).await?;

    // Quality check before timed run
    println!("  quality check...");
    quality_check(w, &url).await?;

    // RSS sampler in background
    let container = container_name.to_string();
    let rss_handle = tokio::task::spawn_blocking(move || {
        let mut peak = 0u64;
        for _ in 0..60 {
            if let Some(v) = rss::sample_rss_mib(&container) {
                peak = peak.max(v);
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        peak
    });

    let mut rep_stats = Vec::new();
    let mut all_durations: Vec<u64> = Vec::new();
    let mut total_errors = 0usize;
    let mut total_elapsed = 0f64;

    for rep in 1..=repetitions {
        println!("  rep {}/{}...", rep, repetitions);
        let result = drive_once(w, &url, concurrency, Duration::from_secs(duration_secs)).await?;
        let s = stats::compute(&result.durations_ms, result.error_count, result.elapsed_secs)?;
        all_durations.extend(&result.durations_ms);
        total_errors += result.error_count;
        total_elapsed += result.elapsed_secs;
        rep_stats.push(s);
    }

    let peak_rss = rss_handle.await.ok().filter(|&p| p > 0);

    let combined = stats::compute(&all_durations, total_errors, total_elapsed)?;

    Ok(report::RunResult {
        stats: combined,
        peak_rss_mib: peak_rss,
        repetitions: rep_stats,
    })
}

async fn quality_check(w: &workload::WorkloadDef, url: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let mut form = reqwest::multipart::Form::new();
    for path in &w.fixtures {
        let bytes = tokio::fs::read(path).await
            .map_err(|e| anyhow::anyhow!("failed to read fixture {:?}: {e}", path))?;
        let filename = w.fixture_filename
            .map(|s| s.to_string())
            .unwrap_or_else(|| path.file_name().unwrap().to_string_lossy().to_string());
        let part = reqwest::multipart::Part::bytes(bytes)
            .file_name(filename.clone())
            .mime_str("application/octet-stream")?;
        form = form.part(w.fixture_field, part);
    }
    for (k, v) in &w.extra_fields {
        form = form.text(k.to_string(), v.to_string());
    }
    let resp = client.post(url).multipart(form).send().await?;
    let status = resp.status();
    let body = resp.bytes().await?;
    if !status.is_success() {
        anyhow::bail!("quality check failed with status {}: {:?}", status, &body[..body.len().min(200)]);
    }
    quality::validate_pdf(&body, w.expected_pages)?;
    Ok(())
}

async fn drive_once(
    w: &workload::WorkloadDef,
    url: &str,
    concurrency: usize,
    duration: Duration,
) -> anyhow::Result<driver::DriveResult> {
    let url = url.to_string();
    let fixtures: Vec<_> = w.fixtures.clone();
    let extra_fields: Vec<_> = w.extra_fields.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let fixture_field = w.fixture_field;
    let fixture_filename = w.fixture_filename;
    let body_fn = Arc::new(move || {
        let url = url.clone();
        let fixtures = fixtures.clone();
        let extra_fields = extra_fields.clone();
        Box::pin(async move {
            let mut form = reqwest::multipart::Form::new();
            for path in &fixtures {
                let bytes = tokio::fs::read(path).await?;
                let filename = fixture_filename
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| path.file_name().unwrap().to_string_lossy().to_string());
                let part = reqwest::multipart::Part::bytes(bytes)
                    .file_name(filename.clone())
                    .mime_str("application/octet-stream")?;
                form = form.part(fixture_field, part);
            }
            for (k, v) in &extra_fields {
                form = form.text(k.clone(), v.clone());
            }
            Ok((url, form))
        }) as std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<(String, reqwest::multipart::Form)>> + Send>>
    });

    driver::drive(concurrency, duration, body_fn).await
}
