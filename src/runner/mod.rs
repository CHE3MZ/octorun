//! Runner platform detection, download, extraction, and configuration.

pub mod config;
pub mod download;
pub mod extract;
pub mod install;
pub mod platform;
pub mod process;

pub use download::{download, download_with_cancel, sha256_file};
pub use extract::extract;
pub use platform::{RunnerPlatform, arch, archive_extension, archive_name, os};
