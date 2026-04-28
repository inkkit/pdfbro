//! In-memory job queue for webhook processing.

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{Mutex, mpsc};
use tracing::{error, info, warn};
use uuid::Uuid;


use super::{WebhookClient, WebhookConfig, WebhookError, WebhookJob, WebhookOperation, process_webhook_job};

/// Job queue sender handle.
#[derive(Clone)]
pub struct WebhookQueue {
    sender: mpsc::Sender<WebhookJob>,
}

impl WebhookQueue {
    /// Create a new queue with the given buffer size.
    pub fn new(buffer_size: usize) -> (Self, mpsc::Receiver<WebhookJob>) {
        let (sender, receiver) = mpsc::channel(buffer_size);
        (Self { sender }, receiver)
    }

    /// Send a job to the queue.
    pub async fn send(&self, job: WebhookJob) -> Result<(), WebhookError> {
        self.sender
            .send(job)
            .await
            .map_err(|_| WebhookError::Delivery("Queue closed".into()))
    }
}

/// Spawn a webhook job and return the job ID.
pub async fn spawn_job(
    queue: &WebhookQueue,
    operation: WebhookOperation,
    config: WebhookConfig,
    data: super::JobData,
) -> Result<String, WebhookError> {
    let job_id = Uuid::new_v4().to_string();

    let operation_str = operation.as_str();
    let job = WebhookJob {
        id: job_id.clone(),
        operation,
        config,
        data,
    };

    queue.send(job).await?;
    info!(job_id = %job_id, operation = %operation_str, "Webhook job spawned");

    Ok(job_id)
}

/// Start worker tasks to process webhook jobs.
pub fn start_workers(
    receiver: mpsc::Receiver<WebhookJob>,
    num_workers: usize,
    client: WebhookClient,
) {
    let client = Arc::new(client);
    let receiver = Arc::new(Mutex::new(receiver));

    for worker_id in 0..num_workers {
        let client = Arc::clone(&client);
        let rx = Arc::clone(&receiver);

        tokio::spawn(async move {
            info!(worker_id, "Webhook worker started");

            loop {
                let job = {
                    let mut rx_guard = rx.lock().await;
                    rx_guard.recv().await
                };

                let job = match job {
                    Some(j) => j,
                    None => {
                        warn!(worker_id, "Webhook worker shutting down (channel closed)");
                        break;
                    }
                };

                let start_time = Instant::now();
                let job_id = job.id.clone();

                info!(worker_id, job_id = %job_id, "Processing webhook job");

                match process_webhook_job(job, &client, start_time).await {
                    Ok(()) => {
                        info!(worker_id, job_id = %job_id, "Webhook job completed");
                    }
                    Err(e) => {
                        error!(worker_id, job_id = %job_id, error = %e, "Webhook job failed");
                    }
                }
            }

            warn!(worker_id, "Webhook worker shutting down");
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn queue_send_and_receive() {
        let (queue, mut receiver) = WebhookQueue::new(10);

        let job = WebhookJob {
            id: "test-1".into(),
            operation: WebhookOperation::PdfMerge,
            config: WebhookConfig {
                webhook_url: "https://example.com/webhook".into(),
                error_url: None,
                extra_headers: Default::default(),
                sync_mode: false,
            },
            data: super::super::JobData::PdfMerge { files: vec![] },
        };

        queue.send(job.clone()).await.unwrap();

        let received = receiver.recv().await.unwrap();
        assert_eq!(received.id, "test-1");
    }
}
