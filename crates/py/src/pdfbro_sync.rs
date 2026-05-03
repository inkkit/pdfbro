//! `class PdfBro` — sync facade over the engine using a shared tokio runtime.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

#[cfg(feature = "chromium")]
use engine::ChromiumEngine;
#[cfg(feature = "libreoffice")]
use engine::LibreOfficeEngine;

use crate::errors::{engine_to_py, EngineDisabledError};
use crate::runtime::runtime;
use crate::types::from_py;

pub(crate) struct State {
    #[cfg(feature = "chromium")]
    pub chromium: Option<Arc<ChromiumEngine>>,
    #[cfg(feature = "libreoffice")]
    pub libreoffice: Option<Arc<LibreOfficeEngine>>,
    pub closed: bool,
}

#[pyclass(name = "PdfBro", module = "pdfbro")]
pub struct PdfBro {
    pub(crate) inner: parking_lot::Mutex<State>,
}

#[pymethods]
impl PdfBro {
    #[new]
    #[pyo3(signature = (engines = None, chrome_path = None, auto_download_chrome = true, chrome_cache_dir = None))]
    fn new(
        py: Python<'_>,
        engines: Option<Vec<String>>,
        chrome_path: Option<String>,
        auto_download_chrome: bool,
        chrome_cache_dir: Option<String>,
    ) -> PyResult<Self> {
        let want = engines.unwrap_or_else(|| vec!["chromium".into(), "office".into()]);
        let want_chromium = want.iter().any(|s| s == "chromium");
        let want_office = want.iter().any(|s| s == "office" || s == "libreoffice");

        py.allow_threads(|| -> PyResult<Self> {
            runtime().block_on(async move {
                #[cfg(feature = "chromium")]
                let chromium = if want_chromium {
                    Some(Arc::new(
                        crate::launch::launch_chromium(
                            chrome_path.as_deref(),
                            auto_download_chrome,
                            chrome_cache_dir.as_deref(),
                        )
                        .await?,
                    ))
                } else {
                    None
                };

                #[cfg(feature = "libreoffice")]
                let libreoffice = if want_office {
                    Some(Arc::new(crate::launch::launch_libreoffice().await?))
                } else {
                    None
                };

                #[cfg(not(feature = "chromium"))]
                let _ = (want_chromium, chrome_path, auto_download_chrome, chrome_cache_dir);
                #[cfg(not(feature = "libreoffice"))]
                let _ = want_office;

                Ok(PdfBro {
                    inner: parking_lot::Mutex::new(State {
                        #[cfg(feature = "chromium")]
                        chromium,
                        #[cfg(feature = "libreoffice")]
                        libreoffice,
                        closed: false,
                    }),
                })
            })
        })
    }

    fn html_to_pdf<'py>(
        &self,
        py: Python<'py>,
        html: &str,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let opts: engine::PdfOptions = from_py(options)?;
        let engine = self.chromium_or_err()?;
        let html = html.to_string();
        let ctx = engine::RequestContext::default();
        let bytes = py
            .allow_threads(|| {
                runtime().block_on(async move {
                    engine.html_to_pdf(&html, None, &opts, &ctx).await
                })
            })
            .map_err(engine_to_py)?;
        Ok(PyBytes::new_bound(py, &bytes))
    }

    fn url_to_pdf<'py>(
        &self,
        py: Python<'py>,
        url: &str,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let opts: engine::PdfOptions = from_py(options)?;
        let engine = self.chromium_or_err()?;
        let url = url.to_string();
        let ctx = engine::RequestContext::default();
        let bytes = py
            .allow_threads(|| {
                runtime().block_on(async move { engine.url_to_pdf(&url, &opts, &ctx).await })
            })
            .map_err(engine_to_py)?;
        Ok(PyBytes::new_bound(py, &bytes))
    }

    fn markdown_to_pdf<'py>(
        &self,
        py: Python<'py>,
        md: &str,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let opts: engine::PdfOptions = from_py(options)?;
        let engine = self.chromium_or_err()?;
        let md = md.to_string();
        let ctx = engine::RequestContext::default();
        let bytes = py
            .allow_threads(|| {
                runtime()
                    .block_on(async move { engine.markdown_to_pdf(&md, &opts, &ctx).await })
            })
            .map_err(engine_to_py)?;
        Ok(PyBytes::new_bound(py, &bytes))
    }

    fn office_to_pdf<'py>(
        &self,
        py: Python<'py>,
        path: &str,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let opts: engine::OfficeOptions = from_py(options)?;
        let engine = self.office_or_err()?;
        let p = std::path::PathBuf::from(path);
        let bytes = py
            .allow_threads(|| {
                runtime().block_on(async move { engine.convert(&p, &opts).await })
            })
            .map_err(engine_to_py)?;
        Ok(PyBytes::new_bound(py, &bytes))
    }

    fn close(&self, py: Python<'_>) -> PyResult<()> {
        py.allow_threads(|| {
            let mut state = self.inner.lock();
            if state.closed {
                return Ok(());
            }
            state.closed = true;
            Ok::<(), PyErr>(())
        })
    }

    fn __enter__(slf: Py<Self>) -> Py<Self> {
        slf
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        _t: PyObject,
        _v: PyObject,
        _tb: PyObject,
    ) -> PyResult<()> {
        self.close(py)
    }
}

impl PdfBro {
    #[cfg(feature = "chromium")]
    fn chromium_or_err(&self) -> PyResult<Arc<ChromiumEngine>> {
        self.inner.lock().chromium.clone().ok_or_else(|| {
            EngineDisabledError::new_err("chromium engine not enabled for this PdfBro instance")
        })
    }

    #[cfg(not(feature = "chromium"))]
    fn chromium_or_err(&self) -> PyResult<()> {
        Err(EngineDisabledError::new_err(
            "chromium feature not compiled in",
        ))
    }

    #[cfg(feature = "libreoffice")]
    fn office_or_err(&self) -> PyResult<Arc<LibreOfficeEngine>> {
        self.inner.lock().libreoffice.clone().ok_or_else(|| {
            EngineDisabledError::new_err("libreoffice engine not enabled for this PdfBro instance")
        })
    }

    #[cfg(not(feature = "libreoffice"))]
    fn office_or_err(&self) -> PyResult<()> {
        Err(EngineDisabledError::new_err(
            "libreoffice feature not compiled in",
        ))
    }
}
