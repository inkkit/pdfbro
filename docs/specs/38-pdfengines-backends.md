# Spec 38 — PDF Engine Backends

> Support multiple PDF engine backends (QPDF, PDFCPU, pdftk)
> for different operations. Gotenberg allows selecting which backend
> to use for merge, split, flatten, etc.

## Goal

Implement support for multiple PDF engine backends, allowing
operators to choose the best tool for each operation.
Matches Gotenberg's `--pdfengines-*-engines` flags.

## Scope

**In:**

- Configurable backends per operation type:
  - Merge engines: QPDF, PDFCPU, pdftk
  - Split engines: QPDF, PDFCPU
  - Flatten engines: QPDF, PDFCPU, pdftk
  - Convert engines: QPDF (PDF/A)
  - Encrypt engines: QPDF, pdftk
  - Metadata engines: QPDF, pdftk
  - Bookmark engines: QPDF, pdftk
  - Watermark engines: PDFCPU, pdftk
  - Stamp engines: PDFCPU, pdftk
  - Rotate engines: QPDF, pdftk

**Out:**

- Auto-detection of available backends
- Fallback to lopdf when no external tool available
- Custom backends via plugin system

## Configuration Flags

| Flag | Env Variable | Gotenberg Source | Description |
|------|-------------|------------------|-------------|
| `--pdfengines-merge-engines` | `PDFENGINES_MERGE_ENGINES` | `pkg/modules/pdfengines/config.go:MergeEngines` | Comma-separated list (qpdf,pdfcpu,pdftk) |
| `--pdfengines-split-engines` | `PDFENGINES_SPLIT_ENGINES` | `pkg/modules/pdfengines/config.go:SplitEngines` | Comma-separated list |
| `--pdfengines-flatten-engines` | `PDFENGINES_FLATTEN_ENGINES` | `pkg/modules/pdfengines/config.go:FlattenEngines` | Comma-separated list |
| `--pdfengines-convert-engines` | `PDFENGINES_CONVERT_ENGINES` | `pkg/modules/pdfengines/config.go:ConvertEngines` | Usually just qpdf |
| `--pdfengines-read-metadata-engines` | `PDFENGINES_READ_METADATA_ENGINES` | `pkg/modules/pdfengines/config.go:ReadMetadataEngines` | QPDF, pdftk |
| `--pdfengines-write-metadata-engines` | `PDFENGINES_WRITE_METADATA_ENGINES` | `pkg/modules/pdfengines/config.go:WriteMetadataEngines` | QPDF, pdftk |
| `--pdfengines-encrypt-engines` | `PDFENGINES_ENCRYPT_ENGINES` | `pkg/modules/pdfengines/config.go:EncryptEngines` | QPDF, pdftk |
| `--pdfengines-decrypt-engines` | `PDFENGINES_DECRYPT_ENGINES` | `pkg/modules/pdfengines/config.go:DecryptEngines` | QPDF, pdftk |
| `--pdfengines-embed-engines` | `PDFENGINES_EMBED_ENGINES` | `pkg/modules/pdfengines/config.go:EmbedEngines` | QPDF |
| `--pdfengines-read-bookmarks-engines` | `PDFENGINES_READ_BOOKMARKS_ENGINES` | `pkg/modules/pdfengines/config.go:ReadBookmarksEngines` | QPDF, pdftk |
| `--pdfengines-write-bookmarks-engines` | `PDFENGINES_WRITE_BOOKMARKS_ENGINES` | `pkg/modules/pdfengines/config.go:WriteBookmarksEngines` | QPDF, pdftk |
| `--pdfengines-watermark-engines` | `PDFENGINES_WATERMARK_ENGINES` | `pkg/modules/pdfengines/config.go:WatermarkEngines` | PDFCPU, pdftk |
| `--pdfengines-stamp-engines` | `PDFENGINES_STAMP_ENGINES` | `pkg/modules/pdfengines/config.go:StampEngines` | PDFCPU, pdftk |
| `--pdfengines-rotate-engines` | `PDFENGINES_ROTATE_ENGINES` | `pkg/modules/pdfengines/config.go:RotateEngines` | QPDF, pdftk |

## Engine Capabilities Matrix

| Operation | QPDF | PDFCPU | pdftk | lopdf (Folio native) |
|-----------|------|--------|-------|---------------------|
| Merge | ✅ | ✅ | ✅ | ✅ |
| Split | ✅ | ✅ | ❌ | ✅ |
| Flatten | ✅ | ✅ | ✅ | ✅ |
| PDF/A Convert | ✅ | ❌ | ❌ | Partial |
| Encrypt | ✅ | ❌ | ✅ | ✅ |
| Decrypt | ✅ | ❌ | ✅ | ✅ |
| Read Metadata | ✅ | ❌ | ✅ | ✅ |
| Write Metadata | ✅ | ❌ | ✅ | ✅ |
| Read Bookmarks | ✅ | ❌ | ✅ | ✅ |
| Write Bookmarks | ✅ | ❌ | ✅ | ✅ |
| Watermark | ❌ | ✅ | ✅ | ✅ |
| Stamp | ❌ | ✅ | ✅ | ✅ |
| Rotate | ✅ | ❌ | ✅ | ✅ |
| Embed Files | ✅ | ❌ | ❌ | ✅ |

## Implementation

### 1. Enum for Engine Type

```rust
// crates/engine/src/pdfops/mod.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfEngineType {
    Qpdf,
    PdfCpu,
    PdfTk,
    LoPdf,  // Folio native
}

impl PdfEngineType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "qpdf" => Some(Self::Qpdf),
            "pdfcpu" => Some(Self::PdfCpu),
            "pdftk" => Some(Self::PdfTk),
            "lopdf" => Some(Self::LoPdf),
            _ => None,
        }
    }

    pub fn binary_name(&self) -> &'static str {
        match self {
            Self::Qpdf => "qpdf",
            Self::PdfCpu => "pdfcpu",
            Self::PdfTk => "pdftk",
            Self::LoPdf => "lopdf (built-in)",
        }
    }

    pub fn is_available(&self) -> bool {
        match self {
            Self::LoPdf => true,  // Always available
            _ => which::which(self.binary_name()).is_ok(),
        }
    }
}
```

### 2. Configuration Struct

```rust
// crates/server/src/config.rs

pub struct PdfEnginesConfig {
    pub merge_engines: Vec<PdfEngineType>,
    pub split_engines: Vec<PdfEngineType>,
    pub flatten_engines: Vec<PdfEngineType>,
    pub convert_engines: Vec<PdfEngineType>,
    pub read_metadata_engines: Vec<PdfEngineType>,
    pub write_metadata_engines: Vec<PdfEngineType>,
    pub encrypt_engines: Vec<PdfEngineType>,
    pub decrypt_engines: Vec<PdfEngineType>,
    pub embed_engines: Vec<PdfEngineType>,
    pub read_bookmarks_engines: Vec<PdfEngineType>,
    pub write_bookmarks_engines: Vec<PdfEngineType>,
    pub watermark_engines: Vec<PdfEngineType>,
    pub stamp_engines: Vec<PdfEngineType>,
    pub rotate_engines: Vec<PdfEngineType>,
}

impl Default for PdfEnginesConfig {
    fn default() -> Self {
        Self {
            merge_engines: vec![PdfEngineType::Qpdf, PdfEngineType::PdfCpu, PdfEngineType::PdfTk],
            split_engines: vec![PdfEngineType::Qpdf, PdfEngineType::PdfCpu],
            // ... etc.
        }
    }
}
```

### 3. Engine Selection Logic

```rust
// crates/engine/src/pdfops/mod.rs

pub struct PdfOps {
    config: PdfEnginesConfig,
}

impl PdfOps {
    /// Select first available engine for operation.
    fn select_engine(&self, engines: &[PdfEngineType]) -> Option<PdfEngineType> {
        engines.iter()
            .find(|e| e.is_available())
            .copied()
    }

    pub fn merge(&self, inputs: &[PathBuf]) -> Result<Vec<u8>, EngineError> {
        let engine = self.select_engine(&self.config.merge_engines)
            .ok_or_else(|| EngineError::Internal("No merge engine available".into()))?;

        match engine {
            PdfEngineType::Qpdf => self.merge_qpdf(inputs),
            PdfEngineType::PdfCpu => self.merge_pdfcpu(inputs),
            PdfEngineType::PdfTk => self.merge_pdftk(inputs),
            PdfEngineType::LoPdf => self.merge_lopdf(inputs),
        }
    }

    fn merge_qpdf(&self, inputs: &[PathBuf]) -> Result<Vec<u8>, EngineError> {
        let mut cmd = std::process::Command::new("qpdf");
        cmd.arg("--empty").arg("output.pdf");

        for input in inputs {
            cmd.arg("--pages").arg(input).arg("1-z").arg("--");
        }

        // ... execute command
        todo!()
    }

    fn merge_pdfcpu(&self, inputs: &[PathBuf]) -> Result<Vec<u8>, EngineError> {
        let mut cmd = std::process::Command::new("pdfcpu");
        cmd.arg("import");

        for input in inputs {
            cmd.arg(input);
        }

        // ... execute command
        todo!()
    }
}
```

### 4. CLI Flags Parsing

```rust
// crates/server/src/config.rs

impl ServerConfig {
    fn parse_pdfengines_args(args: &Args) -> PdfEnginesConfig {
        let parse_engines = |arg: Option<&str>| {
            arg.unwrap_or("")
                .split(',')
                .filter_map(PdfEngineType::from_str)
                .collect::<Vec<_>>()
        };

        PdfEnginesConfig {
            merge_engines: parse_engines(args.value_of("pdfengines-merge-engines")),
            // ... parse all 14 engine lists
        }
    }
}
```

## References to Gotenberg Source

| Feature | Gotenberg File | Line Numbers |
|---------|------------------|-------------|
| Engine config struct | `pkg/modules/pdfengines/config.go` | Full file (~100 lines) |
| Engine selection | `pkg/modules/pdfengines/pdfengines.go` | ~L200-300 |
| QPDF wrapper | `pkg/modules/pdfengines/qpdf.go` | Full file |
| PDFCPU wrapper | `pkg/modules/pdfengines/pdfcpu.go` | Full file |
| pdftk wrapper | `pkg/modules/pdfengines/pdftk.go` | Full file |

To read Gotenberg source:
```bash
cd /Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg
cat pkg/modules/pdfengines/config.go | grep -A3 "MergeEngines"
```

## Expected Behavior

### Engine Priority
1. Try first engine in list
2. If not available (not installed), try next
3. If none available, return error

### Default Behavior (No Flags)
- Use all available engines in order: qpdf, pdfcpu, pdftk, lopdf

### Custom Engine Selection
```bash
# Use only QPDF for merge (fast, reliable)
--pdfengines-merge-engines=qpdf

# Try PDFCPU first, fallback to pdftk
--pdfengines-split-engines=pdfcpu,pdftk
```

## Test Plan

### Unit Tests

- `engine_type_from_str_parses_correctly`
- `engine_type_is_available_qpdf_installed`
- `select_engine_returns_first_available`
- `select_engine_falls_back_to_next`

### Integration Tests

- `merge_uses_qpdf_when_available`
- `merge_falls_back_to_pdfcpu`
- `merge_uses_lopdf_as_last_resort`

## Acceptance

- [ ] `PdfEngineType` enum with all 4 types
- [ ] `PdfEnginesConfig` with 14 engine lists
- [ ] CLI flags for all engine selections
- [ ] Engine selection logic with fallback
- [ ] QPDF wrapper for merge/split/encrypt
- [ ] PDFCPU wrapper for merge/split/watermark
- [ ] pdftk wrapper for merge/encrypt/bookmarks
- [ ] Unit tests for engine selection
- [ ] Integration tests with real tools
- [ ] `cargo clippy -p engine -- -D warnings` clean

## References

- Gotenberg PDF engines: `/Users/__deesh_reddy__/projects/personal_git/rust_builds/folio/gotenberg/pkg/modules/pdfengines/`
- QPDF documentation: https://qpdf.readthedocs.io/
- PDFCPU documentation: https://pdfcpu.io/
- pdftk documentation: https://www.pdftk.com/
