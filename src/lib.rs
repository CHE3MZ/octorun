//! `octorun` creates disposable GitHub Actions runners.
//!
//! The crate is usable both as a library and through the `octorun` binary. The
//! command-line surface is defined in [`cli`], dispatched in [`commands`], and
//! backed by the [`github`] and [`runner`] modules.

pub mod cli;
pub mod commands;
pub mod error;
pub mod github;
pub mod logging;
pub mod runner;

mod ui;

pub use error::{Error, Result};
