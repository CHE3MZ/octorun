//! Runner platform detection, download, extraction, and configuration.

pub mod config;
pub mod download;
pub mod extract;
pub mod install;
pub mod platform;

pub use download::download;
pub use extract::extract;
pub use platform::{arch, archive_extension, archive_name, os};
