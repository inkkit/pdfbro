//! `folio-server` binary entry point.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
use engine::{BrowserConfig, ChromiumEngine, LibreOfficeConfig, LibreOfficeEngine};
use server::config::{Cli, Command, LogFormat};
use server::webhook::{WebhookClient, WebhookQueue, start_workers};
use server::{AppState, ChromiumBackend, ServerArgs, ServerConfig, banner, build_router, shutdown};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve(args) => serve(args).await,
    }
}

async fn serve(args: ServerArgs) -> anyhow::Result<()> {
    let config = ServerConfig::from_args(&args).context("config resolution")?;
    init_tracing(&config);

    tracing::info!(
        host = %config.host,
        port = config.port,
        concurrency = config.concurrency,
        max_body_bytes = config.max_body_bytes,
        request_timeout = ?config.request_timeout,
        "starting folio-server",
    );

    let browser_cfg = browser_config_from(&config);
    let lo_cfg = libreoffice_config_from(&config);

    let (chromium, libreoffice) = tokio::join!(
        ChromiumEngine::launch_with(browser_cfg),
        LibreOfficeEngine::launch(lo_cfg),
    );
    let chromium = chromium.context("Chromium failed to launch")?;
    let libreoffice = libreoffice.context("LibreOffice failed to launch")?;

    let chromium_ready = chromium.healthy().await;
    let libreoffice_ready = libreoffice.healthy().await;
    banner::print(&config, chromium_ready, libreoffice_ready);

    let chromium_handle = chromium.clone();
    let backend = ChromiumBackend::new(chromium);

    // Start webhook workers for async processing.
    let (webhook_queue, webhook_rx) = WebhookQueue::new(100);
    start_workers(webhook_rx, 2, WebhookClient::default());

    let state = AppState::new(
        Arc::new(backend),
        Some(Arc::new(libreoffice)),
        config.clone(),
    )
    .with_webhook_queue(webhook_queue);

    let router = build_router(state);
    let addr: SocketAddr = SocketAddr::new(config.host, config.port);
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding {addr}"))?;
    tracing::info!(%addr, "listening");

    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(shutdown::shutdown_signal())
        .await
        .context("axum serve")?;

    tracing::info!("server stopped accepting connections; closing engines");

    // Best-effort engine shutdown with a bounded budget.
    let shutdown = tokio::time::timeout(shutdown::DEFAULT_DRAIN, chromium_handle.shutdown());
    if let Err(_e) = shutdown.await {
        tracing::warn!("Chromium shutdown exceeded drain budget");
    }

    tracing::info!("folio-server exited cleanly");
    Ok(())
}

/// Writer that silently drops `BrokenPipe` errors so `tracing` doesn't
/// complain when its stdout pipe is closed (e.g. during test-harness
/// shutdown).
#[derive(Clone)]
struct PipeSafeWriter;

impl std::io::Write for PipeSafeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match std::io::stdout().write(buf) {
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(buf.len()),
            other => other,
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match std::io::stdout().flush() {
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
            other => other,
        }
    }
}

fn init_tracing(config: &ServerConfig) {
    let filter = EnvFilter::try_new(&config.log_level).unwrap_or_else(|_| EnvFilter::new("info"));
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(|| PipeSafeWriter);
    match config.log_format {
        LogFormat::Json => {
            let _ = builder.json().try_init();
        }
        LogFormat::Text => {
            let _ = builder.try_init();
        }
    }
}

fn browser_config_from(config: &ServerConfig) -> BrowserConfig {
    let defaults = BrowserConfig::default();
    BrowserConfig {
        executable: config.chrome_path.clone(),
        no_sandbox: config.no_sandbox.unwrap_or(defaults.no_sandbox),
        timeout: config.request_timeout,
        ..defaults
    }
}

fn libreoffice_config_from(config: &ServerConfig) -> LibreOfficeConfig {
    LibreOfficeConfig {
        executable: config.soffice_path.clone(),
        timeout: config.request_timeout,
        ..LibreOfficeConfig::default()
    }
}
