//! HTTP route handlers, organised one file per logical sub-API.

pub mod batch;
pub mod batch_state;
pub mod batch_types;
#[cfg(feature = "chromium")]
pub mod chromium;
pub mod debug;
pub mod health;
#[cfg(feature = "libreoffice")]
pub mod libreoffice;
pub mod pdfengines;
pub mod util;
