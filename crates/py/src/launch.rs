//! Centralised engine-launch wiring for both Folio and AsyncFolio.

#[cfg(feature = "chromium")]
use engine::BrowserConfig;

#[cfg(feature = "chromium")]
pub async fn launch_chromium(
    chrome_path: Option<&str>,
    auto_download: bool,
    cache_dir: Option<&str>,
) -> Result<engine::ChromiumEngine, pyo3::PyErr> {
    use crate::errors::engine_to_py;
    #[cfg(feature = "chrome-fetch")]
    use crate::errors::fetch_to_py;

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
                Some(
                    engine::chrome_fetch::ensure_chrome(&opts)
                        .await
                        .map_err(fetch_to_py)?,
                )
            }
            #[cfg(not(feature = "chrome-fetch"))]
            {
                let _ = (auto_download, cache_dir);
                None
            }
        }
    };

    let mut cfg = BrowserConfig::default();
    cfg.executable = executable;
    engine::ChromiumEngine::launch_with(cfg)
        .await
        .map_err(engine_to_py)
}

#[cfg(feature = "libreoffice")]
pub async fn launch_libreoffice() -> Result<engine::LibreOfficeEngine, pyo3::PyErr> {
    use crate::errors::engine_to_py;
    engine::LibreOfficeEngine::discover()
        .await
        .map_err(engine_to_py)
}
