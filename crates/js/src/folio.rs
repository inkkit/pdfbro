//! `class Folio` — async Node.js facade over the engine.

use std::sync::Arc;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value as Json;

#[cfg(feature = "chromium")]
use engine::ChromiumEngine;
#[cfg(feature = "libreoffice")]
use engine::LibreOfficeEngine;

use crate::errors::engine_to_napi;
#[cfg(feature = "chrome-fetch")]
use crate::errors::fetch_to_napi;

/// Options passed to [`Folio::create`].
#[napi(object)]
pub struct CreateOptions {
    /// Which engines to enable. Defaults to `["chromium", "office"]`.
    pub engines: Option<Vec<String>>,
    /// Explicit path to a Chrome/Chromium executable.
    pub chrome_path: Option<String>,
    /// Automatically download Chrome if no system Chrome is found.
    pub auto_download_chrome: Option<bool>,
    /// Directory used to cache downloaded Chrome binaries.
    pub chrome_cache_dir: Option<String>,
}

/// Async Folio client that wraps the PDF/document engines.
#[napi]
pub struct Folio {
    #[cfg(feature = "chromium")]
    chromium: Option<Arc<ChromiumEngine>>,
    #[cfg(feature = "libreoffice")]
    libreoffice: Option<Arc<LibreOfficeEngine>>,
}

#[napi]
impl Folio {
    /// Create a new Folio instance, launching the requested engines.
    #[napi(factory)]
    pub async fn create(opts: Option<CreateOptions>) -> Result<Folio> {
        let opts = opts.unwrap_or(CreateOptions {
            engines: None,
            chrome_path: None,
            auto_download_chrome: None,
            chrome_cache_dir: None,
        });
        let want = opts.engines.unwrap_or_else(|| vec!["chromium".into(), "office".into()]);
        let want_chromium = want.iter().any(|s| s == "chromium");
        let want_office = want.iter().any(|s| s == "office" || s == "libreoffice");

        #[cfg(feature = "chromium")]
        let chromium = if want_chromium {
            Some(Arc::new(
                launch_chromium(
                    opts.chrome_path.as_deref(),
                    opts.auto_download_chrome.unwrap_or(true),
                    opts.chrome_cache_dir.as_deref(),
                )
                .await?,
            ))
        } else {
            None
        };

        #[cfg(feature = "libreoffice")]
        let libreoffice = if want_office {
            Some(Arc::new(
                LibreOfficeEngine::discover().await.map_err(engine_to_napi)?,
            ))
        } else {
            None
        };

        #[cfg(not(feature = "chromium"))]
        let _ = (
            want_chromium,
            opts.chrome_path,
            opts.auto_download_chrome,
            opts.chrome_cache_dir,
        );
        #[cfg(not(feature = "libreoffice"))]
        let _ = want_office;

        Ok(Folio {
            #[cfg(feature = "chromium")]
            chromium,
            #[cfg(feature = "libreoffice")]
            libreoffice,
        })
    }

    /// Convert an HTML string to a PDF buffer.
    #[napi]
    pub async fn html_to_pdf(&self, html: String, options: Option<Json>) -> Result<Buffer> {
        let opts: engine::PdfOptions = parse_opts(options)?;
        #[cfg(feature = "chromium")]
        {
            let engine = self.chromium.clone().ok_or_else(|| {
                Error::new(
                    Status::GenericFailure,
                    "[EngineDisabled] chromium engine not enabled",
                )
            })?;
            let ctx = engine::RequestContext::default();
            let bytes = engine
                .html_to_pdf(&html, None, &opts, &ctx)
                .await
                .map_err(engine_to_napi)?;
            Ok(bytes.into())
        }
        #[cfg(not(feature = "chromium"))]
        {
            let _ = (html, opts);
            Err(Error::new(
                Status::GenericFailure,
                "[EngineDisabled] chromium feature not compiled in",
            ))
        }
    }

    /// Convert a URL to a PDF buffer.
    #[napi]
    pub async fn url_to_pdf(&self, url: String, options: Option<Json>) -> Result<Buffer> {
        let opts: engine::PdfOptions = parse_opts(options)?;
        #[cfg(feature = "chromium")]
        {
            let engine = self.chromium.clone().ok_or_else(|| {
                Error::new(
                    Status::GenericFailure,
                    "[EngineDisabled] chromium engine not enabled",
                )
            })?;
            let ctx = engine::RequestContext::default();
            let bytes = engine
                .url_to_pdf(&url, &opts, &ctx)
                .await
                .map_err(engine_to_napi)?;
            Ok(bytes.into())
        }
        #[cfg(not(feature = "chromium"))]
        {
            let _ = (url, opts);
            Err(Error::new(
                Status::GenericFailure,
                "[EngineDisabled] chromium feature not compiled in",
            ))
        }
    }

    /// Convert a Markdown string to a PDF buffer.
    #[napi]
    pub async fn markdown_to_pdf(&self, md: String, options: Option<Json>) -> Result<Buffer> {
        let opts: engine::PdfOptions = parse_opts(options)?;
        #[cfg(feature = "chromium")]
        {
            let engine = self.chromium.clone().ok_or_else(|| {
                Error::new(
                    Status::GenericFailure,
                    "[EngineDisabled] chromium engine not enabled",
                )
            })?;
            let ctx = engine::RequestContext::default();
            let bytes = engine
                .markdown_to_pdf(&md, &opts, &ctx)
                .await
                .map_err(engine_to_napi)?;
            Ok(bytes.into())
        }
        #[cfg(not(feature = "chromium"))]
        {
            let _ = (md, opts);
            Err(Error::new(
                Status::GenericFailure,
                "[EngineDisabled] chromium feature not compiled in",
            ))
        }
    }

    /// Convert an office document at `path` to a PDF buffer.
    #[napi]
    pub async fn office_to_pdf(&self, path: String, options: Option<Json>) -> Result<Buffer> {
        let opts: engine::OfficeOptions = parse_opts(options)?;
        #[cfg(feature = "libreoffice")]
        {
            let engine = self.libreoffice.clone().ok_or_else(|| {
                Error::new(
                    Status::GenericFailure,
                    "[EngineDisabled] libreoffice engine not enabled",
                )
            })?;
            let p = std::path::PathBuf::from(path);
            let bytes = engine.convert(&p, &opts).await.map_err(engine_to_napi)?;
            Ok(bytes.into())
        }
        #[cfg(not(feature = "libreoffice"))]
        {
            let _ = (path, opts);
            Err(Error::new(
                Status::GenericFailure,
                "[EngineDisabled] libreoffice feature not compiled in",
            ))
        }
    }

    /// Shut down the Folio instance and release resources.
    #[napi]
    pub async fn close(&self) -> Result<()> {
        Ok(())
    }
}

fn parse_opts<T: serde::de::DeserializeOwned + Default>(v: Option<Json>) -> Result<T> {
    match v {
        None => Ok(T::default()),
        Some(j) => serde_json::from_value(j).map_err(|e| {
            Error::new(
                Status::InvalidArg,
                format!("[Validation] invalid options: {e}"),
            )
        }),
    }
}

#[cfg(feature = "chromium")]
async fn launch_chromium(
    chrome_path: Option<&str>,
    auto_download: bool,
    cache_dir: Option<&str>,
) -> Result<engine::ChromiumEngine> {
    let executable: Option<std::path::PathBuf> = match chrome_path {
        Some(p) => Some(p.into()),
        None => {
            #[cfg(feature = "chrome-fetch")]
            {
                let opts = engine::chrome_fetch::EnsureOptions {
                    explicit: None,
                    cache_dir: cache_dir.map(Into::into),
                    auto_download,
                };
                Some(engine::chrome_fetch::ensure_chrome(&opts).await.map_err(fetch_to_napi)?)
            }
            #[cfg(not(feature = "chrome-fetch"))]
            {
                let _ = (auto_download, cache_dir);
                None
            }
        }
    };
    let mut cfg = engine::BrowserConfig::default();
    cfg.executable = executable;
    engine::ChromiumEngine::launch_with(cfg).await.map_err(engine_to_napi)
}
