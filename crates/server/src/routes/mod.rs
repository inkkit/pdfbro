//! HTTP route handlers, organised one file per logical sub-API.

#[cfg(feature = "chromium")]
pub mod chromium;
pub mod health;
#[cfg(feature = "libreoffice")]
pub mod libreoffice;
pub mod pdfengines;
pub mod util;
