# Spec 19 — PDF Encryption

> Password protection and permission control for PDF documents.
> Uses qpdf for reliable encryption without lopdf complexity.

## Goal

Provide PDF password protection with user/owner passwords and
granular permission controls. Uses shell-out to qpdf for
production-ready encryption.

## Scope

**In:**

- User password (required to open document).
- Owner password (required to change permissions).
- Permission flags (print, modify, copy, annotate).
- 128-bit and 256-bit AES encryption.
- Remove encryption (with owner password).

**Out:**

- Certificate-based encryption (PKI).
- Digital signatures.
- Custom security handlers.

## Public API

Module path: `engine::encrypt`. Stateless free functions.

```rust
use crate::types::{EngineError, EngineResult};

/// Encryption algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    /// 128-bit AES (RC4 deprecated).
    Aes128,
    /// 256-bit AES (recommended).
    Aes256,
}

/// Permission flags for encrypted PDF.
#[derive(Debug, Clone, Copy, Default)]
pub struct Permissions {
    /// Allow printing (low-res).
    pub print: bool,
    /// Allow high-quality printing.
    pub print_high_quality: bool,
    /// Allow content modification.
    pub modify_content: bool,
    /// Allow annotation and form filling.
    pub annotate: bool,
    /// Allow form filling (if false, only existing fields).
    pub fill_forms: bool,
    /// Allow content extraction (copy/paste).
    pub extract_content: bool,
    /// Allow document assembly (merge, insert pages).
    pub assemble: bool,
}

impl Permissions {
    /// Default permissions: all allowed.
    pub fn allow_all() -> Self {
        Self {
            print: true,
            print_high_quality: true,
            modify_content: true,
            annotate: true,
            fill_forms: true,
            extract_content: true,
            assemble: true,
        }
    }

    /// Restrictive permissions: view only.
    pub fn view_only() -> Self {
        Self {
            print: false,
            print_high_quality: false,
            modify_content: false,
            annotate: false,
            fill_forms: false,
            extract_content: false,
            assemble: false,
        }
    }
}

/// Encrypt PDF with password protection.
///
/// At least one of `user_password` or `owner_password` must be provided.
/// If `owner_password` is None, it's set same as user_password.
pub async fn encrypt_pdf(
    pdf: &[u8],
    user_password: Option<&str>,
    owner_password: Option<&str>,
    algorithm: EncryptionAlgorithm,
    permissions: Permissions,
) -> EngineResult<Vec<u8>>;

/// Remove encryption from PDF.
///
/// Requires owner password (or user password if no owner set).
pub async fn decrypt_pdf(
    pdf: &[u8],
    password: &str,
) -> EngineResult<Vec<u8>>;

/// Check if PDF is encrypted.
pub fn is_encrypted(pdf: &[u8]) -> EngineResult<bool>;
```

## Implementation Strategy

### Using `qpdf`

qpdf has excellent encryption support:

```bash
# Encrypt with user password
qpdf --encrypt userpass ownerpass 256 -- input.pdf output.pdf

# Encrypt with permissions
qpdf --encrypt userpass ownerpass 256 \
     --print=none --modify=none --extract=n \
     input.pdf output.pdf

# Decrypt
qpdf --password=ownerpass --decrypt input.pdf output.pdf
```

### Permission Mapping

| Permission | qpdf flag | PDF spec |
|------------|-----------|----------|
| Print low-res | `--print=low` | bit 3 |
| Print high-res | `--print=full` | bit 3 + 12 |
| Modify content | `--modify=annotate` | bit 4 |
| Annotate | `--modify=annotate` | bit 6 |
| Fill forms | `--modify=form` | bit 9 |
| Extract | `--extract=y` | bit 5 |
| Assemble | `--assemble=y` | bit 11 |

## Server API

### Encrypt Endpoint

```
POST /forms/pdfengines/encrypt
```

Form fields:
- `files` - Single PDF file
- `userPassword` - Password required to open (optional)
- `ownerPassword` - Password to change permissions (optional)
- `algorithm` - "aes128" or "aes256" (default: aes256)
- `permissions` - Comma-separated list:
  - `print`, `print-hq`, `modify`, `annotate`, `fill-forms`, `extract`, `assemble`
  - Or `all` (default), `none`, `view-only`

Response:
- Encrypted PDF
- `Content-Disposition: attachment; filename="result.pdf"`

### Decrypt Endpoint

```
POST /forms/pdfengines/decrypt
```

Form fields:
- `files` - Encrypted PDF
- `password` - User or owner password

Response:
- Decrypted PDF

## Error Handling

| Error | Condition |
|-------|-----------|
| `EngineError::InvalidInput` | No password provided |
| `EngineError::EncryptionFailed` | qpdf error |
| `EngineError::DecryptionFailed` | Wrong password |
| `EngineError::NotEncrypted` | Decrypt called on unencrypted PDF |

## Testing

Unit tests:
- Encrypt with user password, decrypt succeeds
- Encrypt with owner password only
- Permission verification (attempt restricted action)
- Wrong password rejection

Integration tests:
- Gotenberg feature parity
- PDF/A compliance after encryption (should be preserved)

## Dependencies

Runtime: `qpdf` binary (already in Docker image)

```toml
[dependencies]
# Shell execution
tokio = { workspace = true }
tempfile = { workspace = true }
```

## Security Notes

1. **Passwords transmitted in form data** - Use HTTPS in production
2. **qpdf binary must be available** - Check at startup
3. **Temporary files** - Cleaned up after operation
4. **Memory safety** - Passwords not logged

## References

- qpdf encryption docs: https://qpdf.readthedocs.io/en/stable/encryption.html
- PDF 2.0 spec ISO 32000-2: Section 7.6 (Encryption)
- Gotenberg docs: https://gotenberg.dev/docs/routes#pdf-engines
