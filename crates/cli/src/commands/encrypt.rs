//! `folio encrypt` / `folio decrypt` — PDF password protection.

use anyhow::Context;
use engine::encrypt::{EncryptionAlgorithm, Permissions, decrypt_pdf, encrypt_pdf};

use crate::args::{DecryptArgs, EncryptAlgorithm, EncryptArgs};
use crate::io_helpers::{read_input_sync, write_output};

/// `folio encrypt INPUT --output FILE [--user-password PASS] [--owner-password PASS]`
pub(crate) async fn run_encrypt(args: &EncryptArgs) -> anyhow::Result<()> {
    let pdf_bytes = read_input_sync(&args.input)?;

    let algorithm = match args.algorithm {
        EncryptAlgorithm::Aes128 => EncryptionAlgorithm::Aes128,
        EncryptAlgorithm::Aes256 => EncryptionAlgorithm::Aes256,
    };
    let permissions = Permissions::from_string(&args.permissions);

    let encrypted = encrypt_pdf(
        &pdf_bytes,
        args.user_password.as_deref(),
        args.owner_password.as_deref(),
        algorithm,
        permissions,
    )
    .await
    .context("encrypting PDF")?;

    write_output(&args.output, &encrypted)
}

/// `folio decrypt INPUT --output FILE --password PASS`
pub(crate) async fn run_decrypt(args: &DecryptArgs) -> anyhow::Result<()> {
    let pdf_bytes = read_input_sync(&args.input)?;

    let decrypted = decrypt_pdf(&pdf_bytes, &args.password)
        .await
        .context("decrypting PDF")?;

    write_output(&args.output, &decrypted)
}
