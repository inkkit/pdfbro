//! PDF encryption and password protection.
//!
//! Implements spec 19 — PDF Encryption using qpdf.

use std::time::Duration;

use lopdf::Object;

use crate::types::{EngineError, EngineResult};

/// Encryption algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    /// 128-bit AES encryption.
    Aes128,
    /// 256-bit AES encryption (recommended).
    Aes256,
}

impl EncryptionAlgorithm {
    /// qpdf key length argument.
    fn key_length(&self) -> &'static str {
        match self {
            EncryptionAlgorithm::Aes128 => "128",
            EncryptionAlgorithm::Aes256 => "256",
        }
    }
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

    /// Parse from comma-separated string.
    pub fn from_string(s: &str) -> Self {
        let mut perms = Self::view_only();
        for part in s.split(',') {
            match part.trim() {
                "all" => return Self::allow_all(),
                "none" | "view-only" => return Self::view_only(),
                "print" => perms.print = true,
                "print-hq" => {
                    perms.print = true;
                    perms.print_high_quality = true;
                }
                "modify" => perms.modify_content = true,
                "annotate" => {
                    perms.annotate = true;
                    perms.fill_forms = true;
                }
                "fill-forms" => perms.fill_forms = true,
                "extract" => perms.extract_content = true,
                "assemble" => perms.assemble = true,
                _ => {}
            }
        }
        perms
    }

    /// Convert to qpdf arguments.
    fn to_qpdf_args(&self) -> Vec<String> {
        let mut args = vec![];

        // Print permissions
        if self.print_high_quality {
            args.push("--print=full".to_string());
        } else if self.print {
            args.push("--print=low".to_string());
        } else {
            args.push("--print=none".to_string());
        }

        // Modify permissions
        if self.modify_content {
            args.push("--modify=all".to_string());
        } else if self.annotate || self.fill_forms {
            args.push("--modify=annotate".to_string());
        } else {
            args.push("--modify=none".to_string());
        }

        // Extract
        args.push(if self.extract_content {
            "--extract=y"
        } else {
            "--extract=n"
        }.to_string());

        // Assemble
        args.push(if self.assemble {
            "--assemble=y"
        } else {
            "--assemble=n"
        }.to_string());

        args
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
) -> EngineResult<Vec<u8>> {
    let user_pass = user_password.unwrap_or("");
    let owner_pass = owner_password.unwrap_or(user_pass);

    if user_pass.is_empty() && owner_pass.is_empty() {
        return Err(EngineError::InvalidOption(
            "At least one password must be provided for encryption".into(),
        ));
    }

    // Create temp files
    let tmp_dir = tempfile::tempdir()?;
    let input_path = tmp_dir.path().join("input.pdf");
    let output_path = tmp_dir.path().join("output.pdf");

    // Write input
    tokio::fs::write(&input_path, pdf).await?;

    // Build qpdf command
    let mut cmd = tokio::process::Command::new("qpdf");
    cmd.arg("--encrypt")
        .arg(user_pass)
        .arg(owner_pass)
        .arg(algorithm.key_length());

    // Add permission arguments
    for arg in permissions.to_qpdf_args() {
        cmd.arg(arg);
    }

    cmd.arg("--")
        .arg(&input_path)
        .arg(&output_path)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Execute
    let timeout = Duration::from_secs(60);
    let output = match tokio::time::timeout(timeout, cmd.output()).await {
        Err(_) => return Err(EngineError::Timeout(timeout)),
        Ok(Err(e)) => {
            return Err(EngineError::Internal(format!(
                "qpdf spawn failed: {}",
                e
            )));
        }
        Ok(Ok(o)) => o,
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(EngineError::Internal(format!(
            "qpdf encryption failed: {}",
            stderr
        )));
    }

    // Read output
    let result = tokio::fs::read(&output_path).await?;
    Ok(result)
}

/// Remove encryption from PDF.
///
/// Requires owner password (or user password if no owner set).
pub async fn decrypt_pdf(pdf: &[u8], password: &str) -> EngineResult<Vec<u8>> {
    // Create temp files
    let tmp_dir = tempfile::tempdir()?;
    let input_path = tmp_dir.path().join("input.pdf");
    let output_path = tmp_dir.path().join("output.pdf");

    // Write input
    tokio::fs::write(&input_path, pdf).await?;

    // Build qpdf command
    let mut cmd = tokio::process::Command::new("qpdf");
    cmd.arg(format!("--password={}", password))
        .arg("--decrypt")
        .arg(&input_path)
        .arg(&output_path)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Execute
    let timeout = Duration::from_secs(60);
    let output = match tokio::time::timeout(timeout, cmd.output()).await {
        Err(_) => return Err(EngineError::Timeout(timeout)),
        Ok(Err(e)) => {
            return Err(EngineError::Internal(format!(
                "qpdf spawn failed: {}",
                e
            )));
        }
        Ok(Ok(o)) => o,
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Check for wrong password
        if stderr.contains("invalid password") || stderr.contains("Incorrect password") {
            return Err(EngineError::InvalidOption("Incorrect password".into()));
        }
        return Err(EngineError::Internal(format!(
            "qpdf decryption failed: {}",
            stderr
        )));
    }

    // Read output
    let result = tokio::fs::read(&output_path).await?;
    Ok(result)
}

/// Check if PDF is encrypted.
pub fn is_encrypted(pdf: &[u8]) -> EngineResult<bool> {
    use lopdf::Document;

    let doc = Document::load_mem(pdf).map_err(|e| {
        EngineError::InvalidOption(format!("Failed to load PDF: {}", e))
    })?;

    // Check trailer for encryption dictionary
    match doc.trailer.get(b"Encrypt") {
        Ok(Object::Dictionary(dict)) => Ok(!dict.is_empty()),
        Ok(_) => Ok(false), // Not a dictionary
        Err(_) => Ok(false), // No Encrypt entry
    }
}

/// Check if qpdf is available.
pub fn qpdf_available() -> bool {
    which::which("qpdf").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_parsing() {
        let p = Permissions::from_string("print,annotate");
        assert!(p.print);
        assert!(!p.print_high_quality);
        assert!(p.annotate);
        assert!(!p.modify_content);
    }

    #[test]
    fn permissions_all() {
        let p = Permissions::from_string("all");
        assert!(p.print);
        assert!(p.print_high_quality);
        assert!(p.modify_content);
        assert!(p.annotate);
    }

    #[test]
    fn permissions_view_only() {
        let p = Permissions::from_string("none");
        assert!(!p.print);
        assert!(!p.annotate);
        assert!(!p.extract_content);
    }

    #[test]
    fn encryption_algorithm_key_length() {
        assert_eq!(EncryptionAlgorithm::Aes128.key_length(), "128");
        assert_eq!(EncryptionAlgorithm::Aes256.key_length(), "256");
    }
}
