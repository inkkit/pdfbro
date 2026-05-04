#![deny(rust_2018_idioms)]
#![warn(missing_docs)]
#![recursion_limit = "256"]
//! pdfbro HTTP server — Gotenberg-compatible facade over the [`engine`] crate.
//!
//! See `docs/specs/30-server.md` for the wire contract. Public entry points
//! live in [`app`] (router construction), [`state`] (`AppState`), and
//! [`config`] (CLI / env resolution).

pub mod app;
pub mod auth;
pub mod backend;
pub mod banner;
pub mod batch_worker;
pub mod console_store;
pub mod config;
pub mod cgroup;
pub mod download;
pub mod error;
pub mod logging;
pub mod metrics;
pub mod multipart;
pub mod routes;
pub mod security;
pub mod shutdown;
pub mod state;
pub mod supervised_engine;
pub mod ulid_utils;
pub mod webhook;

pub use app::build_router;
pub use config::{LogFormat, ServerArgs, ServerConfig};
pub use error::{ApiError, ApiResult};
pub use state::AppState;

#[cfg(feature = "chromium")]
pub use backend::{ChromiumBackend, PdfBackend};
