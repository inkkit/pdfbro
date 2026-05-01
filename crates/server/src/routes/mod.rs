//! HTTP route handlers, organised one file per logical sub-API.

pub mod batch;
pub mod batch_state;
pub mod batch_types;
#[cfg(feature = "chromium")]
pub mod chromium;
pub mod console;
pub mod debug;
pub mod estimate;
pub mod health;
#[cfg(feature = "libreoffice")]
pub mod libreoffice;
pub mod openapi;
pub mod pdfengines;
pub mod preview;
pub mod util;
