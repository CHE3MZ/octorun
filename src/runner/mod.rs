//! Runner platform detection, download, and extraction.

pub mod download;
pub mod extract;
pub mod platform;

pub use download::download;
pub use extract::extract;
pub use platform::{arch, archive_extension, archive_name, os};
