//! `folio-server` binary entry point.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use clap::Parser;
#[cfg(not(feature = "chromium"))]
use server::backend::PdfBackend;
use server::config::{Cli, Command};
use server::logging::init_logging;
use server::routes::batch_state::{BatchStateManager, spawn_cleanup_task};
use server::webhook::{WebhookClient, WebhookEngineContext, WebhookQueue, start_workers};
use server::{AppState, ServerArgs, ServerConfig, banner, build_router, shutdown};
use server::supervised_engine::{SupervisedChromiumEngine, SupervisedLibreOfficeEngine};
use tracing::warn;

#[cfg(feature = "chromium")]
use engine::BrowserConfig;
#[cfg(feature = "chromium")]
use server::ChromiumBackend;
#[cfg(feature = "libreoffice")]
use engine::LibreOfficeConfig;
use tokio::net::TcpListener;

use axum_server::tls_rustls::RustlsConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Serve(args) => serve(args).await,
    }
}

async fn serve(args: ServerArgs) -> anyhow::Result<()> {
    let config = ServerConfig::from_args(&args).context("config resolution")?;
    init_logging(
        config.log_format.as_str(),
        &config.log_level,
        config.otel_enabled,
        &config.otel_endpoint,
    )
    .context("logging initialization")?;

    tracing::info!(
        host = %config.host,
        port = config.port,
        concurrency = config.concurrency,
        max_body_bytes = config.max_body_bytes,
        request_timeout = ?config.request_timeout,
        otel_enabled = config.otel_enabled,
        otel_endpoint = %config.otel_endpoint,
        "starting folio-server",
    );

    #[cfg(feature = "chromium")]
    let browser_cfg = browser_config_from(&config);
    #[cfg(feature = "libreoffice")]
    let lo_cfg = libreoffice_config_from(&config);

    // Create supervised engines with lazy/eager start and idle shutdown support
    #[cfg(feature = "chromium")]
    let chromium = SupervisedChromiumEngine::new(browser_cfg);
    #[cfg(feature = "chromium")]
    chromium.start_idle_monitor();
    #[cfg(not(feature = "chromium"))]
    let _chromium: Option<()> = None;

    #[cfg(feature = "libreoffice")]
    let libreoffice = SupervisedLibreOfficeEngine::new(lo_cfg);
    #[cfg(feature = "libreoffice")]
    libreoffice.start_idle_monitor();
    #[cfg(not(feature = "libreoffice"))]
    let _libreoffice: Option<()> = None;

    // Eagerly start engines at server startup (unless lazy_start is enabled)
    #[cfg(feature = "chromium")]
    if !config.chromium_lazy_start {
        if let Err(e) = chromium.start().await {
            warn!(error = %e, "Failed to start Chromium engine at startup");
        }
    }
    #[cfg(feature = "libreoffice")]
    if !config.libreoffice_lazy_start {
        if let Err(e) = libreoffice.start().await {
            warn!(error = %e, "Failed to start LibreOffice engine at startup");
        }
    }

    // Check health
    #[cfg(feature = "chromium")]
    let chromium_ready = chromium.healthy().await;
    #[cfg(not(feature = "chromium"))]
    let chromium_ready = false;

    #[cfg(feature = "libreoffice")]
    let libreoffice_ready = libreoffice.healthy().await;
    #[cfg(not(feature = "libreoffice"))]
    let libreoffice_ready = false;
    
    banner::print(&config, chromium_ready, libreoffice_ready);

    #[cfg(feature = "chromium")]
    let backend = ChromiumBackend::new(chromium.clone());

    // Start webhook workers for async processing.
    let (webhook_queue, webhook_rx) = WebhookQueue::new(100);
    let webhook_ctx = WebhookEngineContext {
        #[cfg(feature = "chromium")]
        chromium: Some(Arc::new(backend.clone())),
        #[cfg(not(feature = "chromium"))]
        chromium: None,
        #[cfg(feature = "libreoffice")]
        libreoffice: Some(Arc::new(libreoffice.clone())),
        #[cfg(not(feature = "libreoffice"))]
        libreoffice: None,
    };
    start_workers(webhook_rx, 2, WebhookClient::default(), webhook_ctx);

    // Initialize batch state manager
    let batch_manager = BatchStateManager::new(
        config.batch_storage_path.clone(),
        config.batch_retention_minutes,
    )
    .await
    .context("batch manager initialization")?;

    // Spawn batch cleanup task (runs every hour)
    spawn_cleanup_task(batch_manager.clone(), 60);

    let state = AppState::new(
        #[cfg(feature = "chromium")]
        Some(Arc::new(backend)),
        #[cfg(not(feature = "chromium"))]
        None::<Arc<dyn PdfBackend>>,
        config.clone(),
    )
    .with_webhook_queue(webhook_queue)
    .with_batch_manager(batch_manager);
    #[cfg(feature = "libreoffice")]
    let state = state.with_libreoffice(Some(Arc::new(libreoffice)));

    let router = build_router(state, &config);
    let addr: SocketAddr = SocketAddr::new(config.host, config.port);

    // Check if TLS is configured
    let tls_enabled = config.api_tls_cert_file.is_some() && config.api_tls_key_file.is_some();

    if tls_enabled {
        let cert_file = config.api_tls_cert_file.as_ref().unwrap();
        let key_file = config.api_tls_key_file.as_ref().unwrap();

        tracing::info!(%addr, cert = %cert_file.display(), key = %key_file.display(), "starting HTTPS server (listening)");

        let tls_config = RustlsConfig::from_pem_file(cert_file, key_file)
            .await
            .context("loading TLS certificates")?;

        let handle = axum_server::Handle::new();
        let shutdown_handle = handle.clone();

        // Spawn graceful shutdown signal handler
        tokio::spawn(async move {
            shutdown::shutdown_signal().await;
            shutdown_handle.shutdown();
        });

        axum_server::bind_rustls(addr, tls_config)
            .handle(handle)
            .serve(router.into_make_service())
            .await
            .context("axum TLS serve")?;
    } else {
        tracing::info!(%addr, "starting HTTP server (listening)");

        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("binding {addr}"))?;

        axum::serve(listener, router.into_make_service())
            .with_graceful_shutdown(shutdown::shutdown_signal())
            .await
            .context("axum serve")?;
    }

    tracing::info!("server stopped accepting connections; closing engines");

    // Best-effort engine shutdown with a bounded budget.
    #[cfg(feature = "chromium")]
    {
        let shutdown = tokio::time::timeout(shutdown::DEFAULT_DRAIN, chromium.shutdown());
        if let Err(_e) = shutdown.await {
            tracing::warn!("Chromium shutdown exceeded drain budget");
        }
    }

    if config.otel_enabled {
        opentelemetry::global::shutdown_tracer_provider();
        tracing::info!("OpenTelemetry tracer provider shut down");
    }

    tracing::info!("folio-server exited cleanly");
    Ok(())
}

#[cfg(feature = "chromium")]
fn browser_config_from(config: &ServerConfig) -> BrowserConfig {
    let defaults = BrowserConfig::default();
    BrowserConfig {
        executable: config.chrome_path.clone(),
        headless: defaults.headless,
        extra_args: defaults.extra_args.clone(),
        no_sandbox: config.no_sandbox.unwrap_or(defaults.no_sandbox),
        timeout: config.request_timeout,
        lazy_start: config.chromium_lazy_start,
        idle_shutdown_timeout: config.chromium_idle_shutdown_timeout,
        network_idle_timeout: None,
    }
}

#[cfg(feature = "libreoffice")]
fn libreoffice_config_from(config: &ServerConfig) -> LibreOfficeConfig {
    let defaults = LibreOfficeConfig::default();
    LibreOfficeConfig {
        executable: config.soffice_path.clone(),
        timeout: config.request_timeout,
        max_concurrency: defaults.max_concurrency,
        lazy_start: config.libreoffice_lazy_start,
        idle_shutdown_timeout: config.libreoffice_idle_shutdown_timeout,
    }
}
