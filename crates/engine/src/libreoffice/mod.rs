//! `LibreOfficeEngine` — convert office documents to PDF via the `soffice`
//! subprocess.
//!
//! Implementation of `docs/specs/12-engine-libreoffice.md`. Each call spawns a
//! short-lived `soffice --headless` child with its own isolated
//! `UserInstallation` profile, making concurrent invocations safe.

pub mod filter;

mod convert;
mod discover;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::types::{EngineError, EngineResult, PageRanges};

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Wrapper around the `soffice` binary. Cheap to clone (`Arc` inside).
///
/// # Example
///
/// ```ignore
/// use std::path::Path;
/// use engine::{LibreOfficeEngine, OfficeOptions};
///
/// # async fn doc() -> engine::EngineResult<()> {
/// let lo = LibreOfficeEngine::discover().await?;
/// let pdf = lo
///     .convert(Path::new("doc.docx"), &OfficeOptions::default())
///     .await?;
/// # let _ = pdf;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct LibreOfficeEngine {
    inner: Arc<Inner>,
}

struct Inner {
    exe: PathBuf,
    timeout: Duration,
    semaphore: Semaphore,
}

/// Engine-wide configuration. Pass to [`LibreOfficeEngine::launch`].
#[derive(Debug, Clone)]
pub struct LibreOfficeConfig {
    /// Path to `soffice` (or `libreoffice`). `None` = autodiscover via
    /// `$LIBREOFFICE_PATH`, `$PATH`, and platform defaults.
    pub executable: Option<PathBuf>,
    /// Per-conversion timeout. Default 120s.
    pub timeout: Duration,
    /// Maximum concurrent subprocess invocations. Default
    /// [`std::thread::available_parallelism`].
    pub max_concurrency: usize,
}

impl Default for LibreOfficeConfig {
    fn default() -> Self {
        Self {
            executable: None,
            timeout: Duration::from_secs(120),
            max_concurrency: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        }
    }
}

impl LibreOfficeEngine {
    /// Discover `soffice` on `$PATH` and platform defaults using
    /// [`LibreOfficeConfig::default`].
    pub async fn discover() -> EngineResult<Self> {
        Self::launch(LibreOfficeConfig::default()).await
    }

    /// Construct an engine with explicit configuration.
    ///
    /// If `config.executable` is `Some`, the path is required to exist;
    /// otherwise auto-discovery is performed. The chosen executable is then
    /// probed (`--headless --version`) before the engine is returned.
    pub async fn launch(config: LibreOfficeConfig) -> EngineResult<Self> {
        let exe = match config.executable {
            Some(p) => {
                if !p.exists() {
                    return Err(EngineError::Internal(format!(
                        "LibreOffice not found: {}",
                        p.display()
                    )));
                }
                p
            }
            None => discover::find_soffice()?,
        };

        discover::probe(&exe, config.timeout).await?;

        let max = config.max_concurrency.max(1);
        Ok(Self {
            inner: Arc::new(Inner {
                exe,
                timeout: config.timeout,
                semaphore: Semaphore::new(max),
            }),
        })
    }

    /// Convert one input file to PDF bytes.
    ///
    /// The input may be any LibreOffice-supported format; see
    /// [`filter::for_extension`] for the dispatch table. Concurrent calls
    /// are gated by `max_concurrency` and each gets a fresh
    /// `UserInstallation` directory.
    pub async fn convert(
        &self,
        input: &Path,
        opts: &OfficeOptions,
    ) -> EngineResult<Vec<u8>> {
        opts.validate()?;
        if !input.exists() {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("input not found: {}", input.display()),
            )));
        }
        let _permit = self
            .inner
            .semaphore
            .acquire()
            .await
            .map_err(|e| EngineError::Internal(format!("semaphore closed: {e}")))?;
        convert::run_convert(&self.inner.exe, self.inner.timeout, input, opts).await
    }

    /// Convert many inputs in parallel (bounded by `max_concurrency`),
    /// returning one `Vec<u8>` per input in the same order.
    ///
    /// Merging into a single PDF is **not** part of this API — call
    /// `engine::pdfops::merge` (spec 13) on the result if needed.
    pub async fn convert_many(
        &self,
        inputs: &[PathBuf],
        opts: &OfficeOptions,
    ) -> EngineResult<Vec<Vec<u8>>> {
        opts.validate()?;
        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        let mut set = tokio::task::JoinSet::new();
        for (i, p) in inputs.iter().enumerate() {
            let engine = self.clone();
            let path = p.clone();
            let opts = opts.clone();
            set.spawn(async move {
                let res = engine.convert(&path, &opts).await;
                (i, res)
            });
        }

        let mut slots: Vec<Option<EngineResult<Vec<u8>>>> =
            (0..inputs.len()).map(|_| None).collect();
        while let Some(joined) = set.join_next().await {
            let (i, res) =
                joined.map_err(|e| EngineError::Internal(format!("join error: {e}")))?;
            slots[i] = Some(res);
        }

        let mut out = Vec::with_capacity(inputs.len());
        for slot in slots {
            match slot {
                Some(Ok(v)) => out.push(v),
                Some(Err(e)) => return Err(e),
                None => {
                    return Err(EngineError::Internal(
                        "convert_many: missing result slot".into(),
                    ));
                }
            }
        }
        Ok(out)
    }

    /// Returns `true` iff `soffice --version` succeeds within a 5-second
    /// timeout (regardless of the engine's `config.timeout`).
    pub async fn healthy(&self) -> bool {
        discover::probe(&self.inner.exe, Duration::from_secs(5))
            .await
            .is_ok()
    }
}

impl std::fmt::Debug for LibreOfficeEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibreOfficeEngine")
            .field("exe", &self.inner.exe)
            .field("timeout", &self.inner.timeout)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Per-call conversion options. All fields are optional; defaults match
/// LibreOffice's own export defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct OfficeOptions {
    /// Render in landscape orientation.
    pub landscape: bool,
    /// Subset of pages to include in the output.
    pub page_ranges: Option<PageRanges>,
    /// PDF/A profile, if any.
    pub pdf_a: Option<PdfAProfile>,
    /// PDF/UA accessibility tagging.
    pub pdf_ua: bool,
    /// JPEG quality knob for embedded raster images. `1..=100`. `None` =
    /// LibreOffice default.
    pub quality: Option<u8>,
    /// Reduce image resolution (DPI). `None` = LibreOffice default.
    pub max_image_resolution: Option<u32>,
}

impl OfficeOptions {
    /// Validate the option set. Called at the top of [`LibreOfficeEngine::convert`]
    /// and [`LibreOfficeEngine::convert_many`].
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::InvalidOption`] if `quality` is outside
    /// `1..=100`, if `max_image_resolution` is `Some(0)`, or if
    /// `page_ranges` is somehow empty.
    pub fn validate(&self) -> EngineResult<()> {
        if let Some(q) = self.quality
            && !(1..=100).contains(&q)
        {
            return Err(EngineError::InvalidOption(format!(
                "quality must be 1..=100 (got {q})"
            )));
        }
        if let Some(r) = self.max_image_resolution
            && r == 0
        {
            return Err(EngineError::InvalidOption(
                "maxImageResolution must be > 0".into(),
            ));
        }
        if let Some(pr) = &self.page_ranges
            && pr.as_slice().is_empty()
        {
            return Err(EngineError::InvalidOption("pageRanges is empty".into()));
        }
        Ok(())
    }

    /// Build the LibreOffice filter-options blob (the `:{...}` suffix on
    /// `--convert-to`). Returns `None` if no fields are set, in which case
    /// the bare exporter (e.g. `pdf:writer_pdf_Export`) is used unmodified.
    pub(crate) fn filter_blob(&self) -> Option<String> {
        let mut map = serde_json::Map::new();

        if let Some(pr) = &self.page_ranges {
            map.insert("PageRange".into(), entry_str(&pr.to_string()));
        }
        if let Some(prof) = self.pdf_a {
            let v: i64 = match prof {
                PdfAProfile::A1B => 1,
                PdfAProfile::A2B => 2,
                PdfAProfile::A3B => 3,
            };
            map.insert("SelectPdfVersion".into(), entry_long(v));
        }
        if self.pdf_ua {
            map.insert("PDFUACompliance".into(), entry_bool(true));
        }
        if let Some(q) = self.quality {
            map.insert("Quality".into(), entry_long(i64::from(q)));
        }
        if let Some(r) = self.max_image_resolution {
            map.insert("MaxImageResolution".into(), entry_long(i64::from(r)));
        }
        if self.landscape {
            map.insert("IsLandscape".into(), entry_bool(true));
        }

        if map.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(map).to_string())
        }
    }
}

fn entry_str(v: &str) -> serde_json::Value {
    serde_json::json!({ "type": "string", "value": v })
}

fn entry_long(v: i64) -> serde_json::Value {
    serde_json::json!({ "type": "long", "value": v })
}

fn entry_bool(v: bool) -> serde_json::Value {
    serde_json::json!({ "type": "boolean", "value": v })
}

/// PDF/A export profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PdfAProfile {
    /// PDF/A-1b — the most conservative subset (PDF 1.4-based).
    A1B,
    /// PDF/A-2b — based on PDF 1.7; supports JPEG2000 and transparency.
    A2B,
    /// PDF/A-3b — like 2b plus permits embedded arbitrary file attachments.
    A3B,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_is_send_sync_clone() {
        use static_assertions::assert_impl_all;
        assert_impl_all!(LibreOfficeEngine: Send, Sync, Clone);
        assert_impl_all!(LibreOfficeConfig: Send, Sync, Clone);
        assert_impl_all!(OfficeOptions: Send, Sync, Clone);
        assert_impl_all!(PdfAProfile: Send, Sync, Clone, Copy);
    }

    #[test]
    fn libreoffice_config_default_matches_spec() {
        let c = LibreOfficeConfig::default();
        assert!(c.executable.is_none());
        assert_eq!(c.timeout, Duration::from_secs(120));
        assert!(c.max_concurrency >= 1);
    }

    #[test]
    fn office_options_default_emits_no_filter_blob() {
        assert!(OfficeOptions::default().filter_blob().is_none());
    }

    #[test]
    fn office_options_with_page_ranges_emits_pagerange_key() {
        let opts = OfficeOptions {
            page_ranges: Some(PageRanges::parse("1-3,5").expect("parse")),
            ..Default::default()
        };
        let blob = opts.filter_blob().expect("blob");
        let v: serde_json::Value = serde_json::from_str(&blob).expect("json");
        assert_eq!(v["PageRange"]["type"], "string");
        assert_eq!(v["PageRange"]["value"], "1-3,5");
    }

    #[test]
    fn office_options_with_pdf_a_maps_select_pdf_version_long() {
        let cases = [(PdfAProfile::A1B, 1), (PdfAProfile::A2B, 2), (PdfAProfile::A3B, 3)];
        for (prof, expected) in cases {
            let opts = OfficeOptions {
                pdf_a: Some(prof),
                ..Default::default()
            };
            let blob = opts.filter_blob().expect("blob");
            let v: serde_json::Value = serde_json::from_str(&blob).expect("json");
            assert_eq!(v["SelectPdfVersion"]["type"], "long");
            assert_eq!(v["SelectPdfVersion"]["value"], expected);
        }
    }

    #[test]
    fn office_options_landscape_and_pdfua_blob_keys() {
        let opts = OfficeOptions {
            landscape: true,
            pdf_ua: true,
            ..Default::default()
        };
        let blob = opts.filter_blob().expect("blob");
        let v: serde_json::Value = serde_json::from_str(&blob).expect("json");
        assert_eq!(v["IsLandscape"]["type"], "boolean");
        assert_eq!(v["IsLandscape"]["value"], true);
        assert_eq!(v["PDFUACompliance"]["type"], "boolean");
        assert_eq!(v["PDFUACompliance"]["value"], true);
    }

    #[test]
    fn office_options_quality_and_resolution_blob_long() {
        let opts = OfficeOptions {
            quality: Some(75),
            max_image_resolution: Some(150),
            ..Default::default()
        };
        let blob = opts.filter_blob().expect("blob");
        let v: serde_json::Value = serde_json::from_str(&blob).expect("json");
        assert_eq!(v["Quality"]["type"], "long");
        assert_eq!(v["Quality"]["value"], 75);
        assert_eq!(v["MaxImageResolution"]["type"], "long");
        assert_eq!(v["MaxImageResolution"]["value"], 150);
    }

    #[test]
    fn office_options_quality_zero_rejected() {
        let opts = OfficeOptions {
            quality: Some(0),
            ..Default::default()
        };
        assert!(matches!(
            opts.validate(),
            Err(EngineError::InvalidOption(_))
        ));
    }

    #[test]
    fn office_options_quality_above_100_rejected() {
        let opts = OfficeOptions {
            quality: Some(101),
            ..Default::default()
        };
        assert!(matches!(
            opts.validate(),
            Err(EngineError::InvalidOption(_))
        ));
    }

    #[test]
    fn office_options_max_image_resolution_zero_rejected() {
        let opts = OfficeOptions {
            max_image_resolution: Some(0),
            ..Default::default()
        };
        assert!(matches!(
            opts.validate(),
            Err(EngineError::InvalidOption(_))
        ));
    }

    #[test]
    fn office_options_default_validates_ok() {
        assert!(OfficeOptions::default().validate().is_ok());
    }

    #[test]
    fn office_options_serde_camel_case_roundtrip() {
        let opts = OfficeOptions {
            landscape: true,
            page_ranges: Some(PageRanges::parse("1-3").expect("parse")),
            pdf_a: Some(PdfAProfile::A2B),
            pdf_ua: true,
            quality: Some(80),
            max_image_resolution: Some(200),
        };
        let json = serde_json::to_value(&opts).expect("ser");
        assert_eq!(json["pageRanges"], "1-3");
        assert_eq!(json["pdfA"], "a2-b");
        assert_eq!(json["pdfUa"], true);
        assert_eq!(json["maxImageResolution"], 200);
        let back: OfficeOptions = serde_json::from_value(json).expect("de");
        assert_eq!(back, opts);
    }

    #[test]
    fn office_options_deserialise_with_missing_fields() {
        let v: OfficeOptions = serde_json::from_str("{}").expect("de");
        assert_eq!(v, OfficeOptions::default());
    }

    #[tokio::test]
    async fn launch_with_missing_executable_path_errors() {
        let cfg = LibreOfficeConfig {
            executable: Some(PathBuf::from("/nonexistent/__folio_no_soffice")),
            ..LibreOfficeConfig::default()
        };
        let err = LibreOfficeEngine::launch(cfg)
            .await
            .expect_err("should fail");
        assert!(matches!(err, EngineError::Internal(_)));
    }
}
