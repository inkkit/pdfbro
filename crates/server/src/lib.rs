#![deny(rust_2018_idioms)]
#![warn(missing_docs)]
//! Folio HTTP server — Gotenberg-compatible facade over the [`engine`] crate.
//!
//! See `docs/specs/30-server.md` for the wire contract. Public entry points
//! live in [`app`] (router construction), [`state`] (`AppState`), and
//! [`config`] (CLI / env resolution).

pub mod app;
pub mod backend;
pub mod banner;
pub mod config;
pub mod error;
pub mod logging;
pub mod metrics;
pub mod multipart;
pub mod routes;
pub mod shutdown;
pub mod state;
pub mod webhook;

pub use app::build_router;
pub use backend::{ChromiumBackend, PdfBackend};
pub use config::{LogFormat, ServerArgs, ServerConfig};
pub use error::{ApiError, ApiResult};
pub use state::AppState;
