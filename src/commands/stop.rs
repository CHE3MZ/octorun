//! `octorun stop` — stop a preconfigured runner.

use super::{runner_dir, validate_name};
use crate::cli::Cli;
use crate::error::Result;
use crate::runner::process;

/// Stop the named runner if it was started by octorun.
///
/// # Errors
///
/// Returns an error if no process is recorded or it cannot be terminated.
pub fn run(cli: &Cli) -> Result<()> {
    validate_name(&cli.name)?;
    let dir = runner_dir(cli);
    let pid = process::stop(&dir)?;

    tracing::info!("stopped runner `{}` (pid {pid})", cli.name);
    Ok(())
}
