//! `pdfbro-server` binary entry point.

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
use server::banner::EngineStatus;
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
        "starting pdfbro-server",
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

    // Spawn eager engine starts in parallel, then await both before printing
    // the banner so the displayed status is accurate. Lazy engines are skipped
    // here and will start on the first request instead.
    #[cfg(feature = "chromium")]
    let chromium_handle = if config.chromium_lazy_start {
        None
    } else {
        let bg = chromium.clone();
        Some(tokio::spawn(async move { bg.start().await }))
    };

    #[cfg(feature = "libreoffice")]
    let lo_handle = if config.libreoffice_lazy_start {
        None
    } else {
        let bg = libreoffice.clone();
        Some(tokio::spawn(async move { bg.start().await }))
    };

    #[cfg(feature = "chromium")]
    let chromium_status = match chromium_handle {
        None => EngineStatus::Lazy,
        Some(h) => match h.await {
            Ok(Ok(_)) => EngineStatus::Ready,
            Ok(Err(e)) => {
                warn!(error = %e, "Failed to start Chromium engine at startup");
                EngineStatus::Unavailable
            }
            Err(e) => {
                warn!(error = %e, "Chromium start task panicked");
                EngineStatus::Unavailable
            }
        },
    };
    #[cfg(not(feature = "chromium"))]
    let chromium_status = EngineStatus::Disabled;

    #[cfg(feature = "libreoffice")]
    let libreoffice_status = match lo_handle {
        None => EngineStatus::Lazy,
        Some(h) => match h.await {
            Ok(Ok(_)) => EngineStatus::Ready,
            Ok(Err(e)) => {
                warn!(error = %e, "Failed to start LibreOffice engine at startup");
                EngineStatus::Unavailable
            }
            Err(e) => {
                warn!(error = %e, "LibreOffice start task panicked");
                EngineStatus::Unavailable
            }
        },
    };
    #[cfg(not(feature = "libreoffice"))]
    let libreoffice_status = EngineStatus::Disabled;

    banner::print(&config, chromium_status, libreoffice_status);

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
    let libreoffice_for_shutdown = libreoffice.clone();
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

    #[cfg(feature = "libreoffice")]
    {
        let shutdown = tokio::time::timeout(shutdown::DEFAULT_DRAIN, libreoffice_for_shutdown.shutdown());
        if let Err(_e) = shutdown.await {
            tracing::warn!("LibreOffice shutdown exceeded drain budget");
        } else {
            tracing::info!("LibreOffice shut down cleanly");
        }
    }

    if config.otel_enabled {
        opentelemetry::global::shutdown_tracer_provider();
        tracing::info!("OpenTelemetry tracer provider shut down");
    }

    tracing::info!("pdfbro-server exited cleanly");
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
        chrome_launch_timeout: defaults.chrome_launch_timeout,
    }
}

#[cfg(feature = "libreoffice")]
fn libreoffice_config_from(config: &ServerConfig) -> LibreOfficeConfig {
    LibreOfficeConfig {
        // The user-supplied path is now a *directory* (LOK's program path);
        // pass it through verbatim. None falls through to libreofficekit's
        // own discovery (LOK_PROGRAM_PATH + known system locations).
        install_path: config.lo_program_dir.clone(),
        timeout: config.request_timeout,
        lazy_start: config.libreoffice_lazy_start,
        idle_shutdown_timeout: config.libreoffice_idle_shutdown_timeout,
    }
}
