//! `octorun start` — start a preconfigured runner.

use crate::cli::Cli;
use crate::error::{Error, Result};

/// Start the named runner as a local process.
///
/// # Errors
///
/// Returns an error if the runner cannot be started.
pub fn run(cli: &Cli) -> Result<()> {
    tracing::debug!(name = %cli.name, "start requested");
    Err(Error::Unimplemented("start".to_owned()))
}
