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

    // Start engines in the background so the HTTP server comes up immediately
    // and Fly.io / health checks can connect before engines finish warming up.
    #[cfg(feature = "chromium")]
    if !config.chromium_lazy_start {
        let chromium_bg = chromium.clone();
        tokio::spawn(async move {
            if let Err(e) = chromium_bg.start().await {
                warn!(error = %e, "Failed to start Chromium engine at startup");
            }
        });
    }
    #[cfg(feature = "libreoffice")]
    if !config.libreoffice_lazy_start {
        let lo_bg = libreoffice.clone();
        tokio::spawn(async move {
            if let Err(e) = lo_bg.start().await {
                warn!(error = %e, "Failed to start LibreOffice engine at startup");
            }
        });
    }

    banner::print(&config, false, false);

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
    let webhook_client = WebhookClient::new(server::webhook::WebhookClientConfig {
        max_retries: config.webhook_max_retry,
        retry_min_wait: config.webhook_retry_min_wait,
        retry_max_wait: config.webhook_retry_max_wait,
        client_timeout: config.webhook_client_timeout,
    });
    start_workers(webhook_rx, 2, webhook_client, webhook_ctx);

    // Compile the operator-supplied allow/deny regex lists once at startup.
    // Bad patterns abort startup so the operator gets immediate feedback.
    let webhook_validator = server::webhook::WebhookUrlValidator::compile(
        &config.webhook_allow_list,
        &config.webhook_deny_list,
    )
    .map_err(anyhow::Error::msg)
    .context("compile webhook allow/deny regex lists")?;

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
    .with_webhook_validator(webhook_validator)
    .with_batch_manager(batch_manager);
    #[cfg(feature = "libreoffice")]
    let state = state.with_libreoffice(Some(Arc::new(libreoffice)));

    {
        use server::console_store::spawn_console_sampler;
        spawn_console_sampler(state.clone(), state.started_at);
    }

    // Clone state before moving into the router, so we can signal SSE shutdown.
    let state_for_shutdown = state.clone();
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
        let state_for_tls_shutdown = state_for_shutdown.clone();
        tokio::spawn(async move {
            shutdown::shutdown_signal().await;
            // Signal SSE streams to close so they don't block the drain.
            let _ = state_for_tls_shutdown.console.shutdown_tx.send(true);
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
            .with_graceful_shutdown(async move {
                shutdown::shutdown_signal().await;
                // Signal SSE streams to close so they don't block the drain.
                let _ = state_for_shutdown.console.shutdown_tx.send(true);
            })
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
        max_page_memory_mb: defaults.max_page_memory_mb,
        max_browser_memory_mb: defaults.max_browser_memory_mb,
        max_concurrent_renders: defaults.max_concurrent_renders,
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
        unoserver_port: config.libreoffice_unoserver_port,
        unoserver_ready_timeout: config.libreoffice_unoserver_ready_timeout,
    }
}
