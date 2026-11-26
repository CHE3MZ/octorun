//! `octorun stop` — stop a preconfigured runner.

use crate::cli::Cli;
use crate::error::{Error, Result};

/// Stop the named runner if it is running locally.
///
/// # Errors
///
/// Returns an error if the runner cannot be stopped.
pub fn run(cli: &Cli) -> Result<()> {
    tracing::debug!(name = %cli.name, "stop requested");
    Err(Error::Unimplemented("stop".to_owned()))
}
