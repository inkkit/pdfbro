//! `folio-server` binary entry point.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
#[cfg(not(feature = "chromium"))]
use server::backend::PdfBackend;
use server::config::{Cli, Command};
use server::logging::init_logging;
use server::webhook::{WebhookClient, WebhookQueue, start_workers};
use server::{AppState, ServerArgs, ServerConfig, banner, build_router, shutdown};

#[cfg(feature = "chromium")]
use engine::{BrowserConfig, ChromiumEngine};
#[cfg(feature = "chromium")]
use server::ChromiumBackend;
#[cfg(feature = "libreoffice")]
use engine::{LibreOfficeConfig, LibreOfficeEngine};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve(args) => serve(args).await,
    }
}

async fn serve(args: ServerArgs) -> anyhow::Result<()> {
    let config = ServerConfig::from_args(&args).context("config resolution")?;
    init_logging(config.log_format.as_str(), &config.log_level)
        .context("logging initialization")?;

    tracing::info!(
        host = %config.host,
        port = config.port,
        concurrency = config.concurrency,
        max_body_bytes = config.max_body_bytes,
        request_timeout = ?config.request_timeout,
        "starting folio-server",
    );

    #[cfg(feature = "chromium")]
    let browser_cfg = browser_config_from(&config);
    #[cfg(feature = "libreoffice")]
    let lo_cfg = libreoffice_config_from(&config);

    #[cfg(feature = "chromium")]
    let chromium = ChromiumEngine::launch_with(browser_cfg)
        .await
        .context("Chromium failed to launch")?;
    #[cfg(not(feature = "chromium"))]
    let _chromium: Option<()> = None;

    #[cfg(feature = "libreoffice")]
    let libreoffice = match LibreOfficeEngine::launch(lo_cfg).await {
        Ok(lo) => Some(lo),
        Err(e) => {
            tracing::warn!(error = %e, "LibreOffice failed to launch; continuing without it");
            None
        }
    };
    #[cfg(not(feature = "libreoffice"))]
    let _libreoffice: Option<()> = None;

    #[cfg(feature = "chromium")]
    let chromium_ready = chromium.healthy().await;
    #[cfg(not(feature = "chromium"))]
    let chromium_ready = false;

    #[cfg(feature = "libreoffice")]
    let libreoffice_ready = match &libreoffice {
        Some(lo) => lo.healthy().await,
        None => false,
    };
    #[cfg(not(feature = "libreoffice"))]
    let libreoffice_ready = false;
    banner::print(&config, chromium_ready, libreoffice_ready);

    #[cfg(feature = "chromium")]
    let chromium_handle = chromium.clone();
    #[cfg(feature = "chromium")]
    let backend = ChromiumBackend::new(chromium);

    // Start webhook workers for async processing.
    let (webhook_queue, webhook_rx) = WebhookQueue::new(100);
    start_workers(webhook_rx, 2, WebhookClient::default());

    let state = AppState::new(
        #[cfg(feature = "chromium")]
        Some(Arc::new(backend)),
        #[cfg(not(feature = "chromium"))]
        None::<Arc<dyn PdfBackend>>,
        config.clone(),
    )
    .with_webhook_queue(webhook_queue);
    #[cfg(feature = "libreoffice")]
    let state = state.with_libreoffice(libreoffice.map(Arc::new));

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
    #[cfg(feature = "chromium")]
    {
        let shutdown = tokio::time::timeout(shutdown::DEFAULT_DRAIN, chromium_handle.shutdown());
        if let Err(_e) = shutdown.await {
            tracing::warn!("Chromium shutdown exceeded drain budget");
        }
    }

    tracing::info!("folio-server exited cleanly");
    Ok(())
}

#[cfg(feature = "chromium")]
fn browser_config_from(config: &ServerConfig) -> BrowserConfig {
    let defaults = BrowserConfig::default();
    BrowserConfig {
        executable: config.chrome_path.clone(),
        no_sandbox: config.no_sandbox.unwrap_or(defaults.no_sandbox),
        timeout: config.request_timeout,
        ..defaults
    }
}

#[cfg(feature = "libreoffice")]
fn libreoffice_config_from(config: &ServerConfig) -> LibreOfficeConfig {
    LibreOfficeConfig {
        executable: config.soffice_path.clone(),
        timeout: config.request_timeout,
        ..LibreOfficeConfig::default()
    }
}
