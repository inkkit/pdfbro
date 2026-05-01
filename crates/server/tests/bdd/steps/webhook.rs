#![allow(dead_code)]

//! In-process webhook capture server for BDD tests.
//!
//! All scenarios that test webhooks are tagged `@folio-skip` because Folio
//! uses a synchronous response API instead of push callbacks. This module
//! exists only to hold the step definitions so the feature files parse
//! correctly.

use crate::support::world::FolioWorld;

/// Step: Given I have a webhook server
/// No-op stub — webhook scenarios are all @folio-skip.
pub async fn setup_webhook_server(_world: &mut FolioWorld) {}

/// Step: When I wait for the asynchronous request to the webhook
pub async fn wait_for_webhook(_world: &mut FolioWorld) {}

/// Step: Then the webhook request header "..." should be "..."
pub async fn check_webhook_header(_world: &mut FolioWorld, _name: String, _value: String) {}

/// Step: Then there should be N PDF(s) in the webhook request
pub async fn check_webhook_pdf_count(_world: &mut FolioWorld, _count: usize) {}
