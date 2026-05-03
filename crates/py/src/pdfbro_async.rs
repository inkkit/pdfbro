//! `class AsyncPdfBro` — async facade returning Python awaitables.
//! Engine futures are bridged to the caller's running event loop via
//! `pyo3_async_runtimes::tokio::future_into_py`.

use std::sync::Arc;

use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use pyo3_async_runtimes::tokio::future_into_py;

#[cfg(feature = "chromium")]
use engine::ChromiumEngine;
#[cfg(feature = "libreoffice")]
use engine::LibreOfficeEngine;

use crate::errors::{engine_to_py, EngineDisabledError};
use crate::types::from_py;

#[pyclass(name = "AsyncPdfBro", module = "pdfbro")]
pub struct AsyncPdfBro {
    #[cfg(feature = "chromium")]
    chromium: Option<Arc<ChromiumEngine>>,
    #[cfg(feature = "libreoffice")]
    libreoffice: Option<Arc<LibreOfficeEngine>>,
}

#[pymethods]
impl AsyncPdfBro {
    #[staticmethod]
    #[pyo3(signature = (engines = None, chrome_path = None, auto_download_chrome = true, chrome_cache_dir = None))]
    fn create<'py>(
        py: Python<'py>,
        engines: Option<Vec<String>>,
        chrome_path: Option<String>,
        auto_download_chrome: bool,
        chrome_cache_dir: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let want = engines.unwrap_or_else(|| vec!["chromium".into(), "office".into()]);
        let want_chromium = want.iter().any(|s| s == "chromium");
        let want_office = want.iter().any(|s| s == "office" || s == "libreoffice");
        future_into_py(py, async move {
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
            Python::with_gil(|py| {
                Ok::<PyObject, PyErr>(
                    Py::new(
                        py,
                        AsyncPdfBro {
                            #[cfg(feature = "chromium")]
                            chromium,
                            #[cfg(feature = "libreoffice")]
                            libreoffice,
                        },
                    )?
                    .into_any()
                    .into(),
                )
            })
        })
    }

    fn html_to_pdf<'py>(
        &self,
        py: Python<'py>,
        html: String,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let opts: engine::PdfOptions = from_py(options)?;
        #[cfg(feature = "chromium")]
        {
            let engine = self
                .chromium
                .clone()
                .ok_or_else(|| EngineDisabledError::new_err("chromium engine not enabled"))?;
            future_into_py(py, async move {
                let ctx = engine::RequestContext::default();
                let bytes = engine
                    .html_to_pdf(&html, None, &opts, &ctx)
                    .await
                    .map_err(engine_to_py)?;
                Python::with_gil(|py| Ok::<PyObject, PyErr>(PyBytes::new_bound(py, &bytes).into()))
            })
        }
        #[cfg(not(feature = "chromium"))]
        {
            let _ = (html, opts);
            Err(EngineDisabledError::new_err("chromium feature not compiled in"))
        }
    }

    fn url_to_pdf<'py>(
        &self,
        py: Python<'py>,
        url: String,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let opts: engine::PdfOptions = from_py(options)?;
        #[cfg(feature = "chromium")]
        {
            let engine = self
                .chromium
                .clone()
                .ok_or_else(|| EngineDisabledError::new_err("chromium engine not enabled"))?;
            future_into_py(py, async move {
                let ctx = engine::RequestContext::default();
                let bytes = engine
                    .url_to_pdf(&url, &opts, &ctx)
                    .await
                    .map_err(engine_to_py)?;
                Python::with_gil(|py| Ok::<PyObject, PyErr>(PyBytes::new_bound(py, &bytes).into()))
            })
        }
        #[cfg(not(feature = "chromium"))]
        {
            let _ = (url, opts);
            Err(EngineDisabledError::new_err("chromium feature not compiled in"))
        }
    }

    fn markdown_to_pdf<'py>(
        &self,
        py: Python<'py>,
        md: String,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let opts: engine::PdfOptions = from_py(options)?;
        #[cfg(feature = "chromium")]
        {
            let engine = self
                .chromium
                .clone()
                .ok_or_else(|| EngineDisabledError::new_err("chromium engine not enabled"))?;
            future_into_py(py, async move {
                let ctx = engine::RequestContext::default();
                let bytes = engine
                    .markdown_to_pdf(&md, &opts, &ctx)
                    .await
                    .map_err(engine_to_py)?;
                Python::with_gil(|py| Ok::<PyObject, PyErr>(PyBytes::new_bound(py, &bytes).into()))
            })
        }
        #[cfg(not(feature = "chromium"))]
        {
            let _ = (md, opts);
            Err(EngineDisabledError::new_err("chromium feature not compiled in"))
        }
    }

    fn office_to_pdf<'py>(
        &self,
        py: Python<'py>,
        path: String,
        options: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let opts: engine::OfficeOptions = from_py(options)?;
        #[cfg(feature = "libreoffice")]
        {
            let engine = self
                .libreoffice
                .clone()
                .ok_or_else(|| EngineDisabledError::new_err("libreoffice engine not enabled"))?;
            future_into_py(py, async move {
                let p = std::path::PathBuf::from(path);
                let bytes = engine.convert(&p, &opts).await.map_err(engine_to_py)?;
                Python::with_gil(|py| Ok::<PyObject, PyErr>(PyBytes::new_bound(py, &bytes).into()))
            })
        }
        #[cfg(not(feature = "libreoffice"))]
        {
            let _ = (path, opts);
            Err(EngineDisabledError::new_err("libreoffice feature not compiled in"))
        }
    }

    fn close<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        future_into_py(py, async move {
            Python::with_gil(|py| Ok::<PyObject, PyErr>(py.None()))
        })
    }

    fn __aenter__<'py>(slf: Py<Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        // Return an awaitable that resolves to self.
        future_into_py(py, async move {
            Python::with_gil(|py| Ok::<PyObject, PyErr>(slf.into_any().into_py(py)))
        })
    }

    fn __aexit__<'py>(
        &self,
        py: Python<'py>,
        _t: PyObject,
        _v: PyObject,
        _tb: PyObject,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.close(py)
    }
}
