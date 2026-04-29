//! Integration tests for `engine::encrypt`.
//!
//! Skipped automatically when `qpdf` is not installed.

use engine::encrypt::{
    EncryptionAlgorithm, Permissions, decrypt_pdf, encrypt_pdf, qpdf_available,
};
use lopdf::{Document, Object, dictionary};

fn qpdf_present() -> bool {
    qpdf_available()
}

/// Build a minimal unencrypted 1-page PDF in memory.
fn make_test_pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let resources_id = doc.add_object(dictionary! {});
    let content_id = doc.add_object(lopdf::Stream::new(dictionary! {}, Vec::new()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => resources_id,
        "Contents" => content_id,
    });

    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(page_id)],
            "Count" => 1_i64,
        }),
    );

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).expect("save test pdf");
    bytes
}

#[tokio::test]
async fn encrypt_user_password_then_decrypt_roundtrip() {
    if !qpdf_present() {
        return;
    }
    let pdf = make_test_pdf();

    let encrypted = encrypt_pdf(
        &pdf,
        Some("user123"),
        None,
        EncryptionAlgorithm::Aes256,
        Permissions::allow_all(),
    )
    .await
    .expect("encrypt should succeed");

    // Decrypt proves encryption happened
    let decrypted = decrypt_pdf(&encrypted, "user123")
        .await
        .expect("decrypt with user password should succeed");

    // Verify page count preserved
    let doc = Document::load_mem(&decrypted).unwrap();
    assert_eq!(doc.get_pages().len(), 1);
}

#[tokio::test]
async fn encrypt_owner_password_only_then_decrypt_roundtrip() {
    if !qpdf_present() {
        return;
    }
    let pdf = make_test_pdf();

    let encrypted = encrypt_pdf(
        &pdf,
        None,
        Some("owner456"),
        EncryptionAlgorithm::Aes256,
        Permissions::view_only(),
    )
    .await
    .expect("encrypt with owner password only should succeed");

    let decrypted = decrypt_pdf(&encrypted, "owner456")
        .await
        .expect("decrypt with owner password should succeed");

    let doc = Document::load_mem(&decrypted).unwrap();
    assert_eq!(doc.get_pages().len(), 1);
}

#[tokio::test]
async fn decrypt_with_wrong_password_fails() {
    if !qpdf_present() {
        return;
    }
    let pdf = make_test_pdf();

    let encrypted = encrypt_pdf(
        &pdf,
        Some("correct"),
        None,
        EncryptionAlgorithm::Aes256,
        Permissions::allow_all(),
    )
    .await
    .expect("encrypt should succeed");

    let result = decrypt_pdf(&encrypted, "wrong").await;
    assert!(result.is_err(), "decrypt with wrong password should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("password") || err.contains("Password"),
        "error should mention password: {err}"
    );
}

#[tokio::test]
async fn encrypt_requires_at_least_one_password() {
    let pdf = make_test_pdf();

    let result = encrypt_pdf(
        &pdf,
        None,
        None,
        EncryptionAlgorithm::Aes256,
        Permissions::allow_all(),
    )
    .await;

    assert!(result.is_err(), "encrypt without passwords should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("password") || err.contains("Password"),
        "error should mention password: {err}"
    );
}

#[tokio::test]
async fn permissions_parsing_via_engine() {
    let p = Permissions::from_string("print,annotate");
    assert!(p.print);
    assert!(!p.print_high_quality);
    assert!(p.annotate);
    assert!(!p.modify_content);
    assert!(p.fill_forms); // annotate implies fill_forms
}

#[tokio::test]
async fn permissions_view_only_alias() {
    let p = Permissions::from_string("view-only");
    assert!(!p.print);
    assert!(!p.annotate);
    assert!(!p.extract_content);
}
