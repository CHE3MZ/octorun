//! `octorun` creates disposable GitHub Actions runners.
//!
//! The crate is usable both as a library and through the `octorun` binary.
//! Use [`Runner`] and [`CreateOptions`] to create and manage runners, or the
//! lower-level [`github`] and [`runner`] modules.

mod api;
mod cli;
mod commands;
pub mod error;
pub mod github;
pub mod logging;
pub mod runner;

mod ui;

pub use api::{CreateOptions, Runner};
pub use error::{Error, Result};

/// Run the octorun command-line interface.
pub fn run() -> std::process::ExitCode {
    cli::entry()
}
