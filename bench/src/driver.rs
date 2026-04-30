use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

pub struct DriveResult {
    pub durations_ms: Vec<u64>,
    pub error_count: usize,
    pub elapsed_secs: f64,
}

type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send>>;

pub async fn drive(
    concurrency: usize,
    duration: Duration,
    body_fn: Arc<dyn Fn() -> BoxFuture<anyhow::Result<(String, reqwest::multipart::Form)>> + Send + Sync>,
) -> anyhow::Result<DriveResult> {
    let client = reqwest::Client::new();
    let sem = Arc::new(Semaphore::new(concurrency));
    let errors = Arc::new(AtomicUsize::new(0));
    let start = Instant::now();
    let deadline = start + duration;

    let mut handles = Vec::new();

    while Instant::now() < deadline {
        let permit = sem.clone().acquire_owned().await?;
        let body_fn = body_fn.clone();
        let errors = errors.clone();
        let client = client.clone();

        let handle = tokio::spawn(async move {
            let _permit = permit;
            let req_start = Instant::now();
            let result = body_fn().await;
            match result {
                Ok((url, form)) => {
                    match client.post(&url).multipart(form).send().await {
                        Ok(resp) if resp.status().is_success() => {
                            Some(req_start.elapsed().as_millis() as u64)
                        }
                        Ok(_) | Err(_) => {
                            errors.fetch_add(1, Ordering::Relaxed);
                            None
                        }
                    }
                }
                Err(_) => {
                    errors.fetch_add(1, Ordering::Relaxed);
                    None
                }
            }
        });
        handles.push(handle);
    }

    let mut durations = Vec::new();
    for h in handles {
        if let Ok(Some(ms)) = h.await {
            durations.push(ms);
        }
    }

    Ok(DriveResult {
        durations_ms: durations,
        error_count: errors.load(Ordering::Relaxed),
        elapsed_secs: start.elapsed().as_secs_f64(),
    })
}
