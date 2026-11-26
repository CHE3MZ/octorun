//! `octorun start` — start a preconfigured runner.

use super::{runner_dir, validate_name};
use crate::cli::Cli;
use crate::error::Result;
use crate::runner::process;

/// Start the named runner as a local process.
///
/// # Errors
///
/// Returns an error if the runner is not configured or cannot be started.
pub fn run(cli: &Cli) -> Result<()> {
    validate_name(&cli.name)?;
    let dir = runner_dir(cli);
    let pid = process::start(&dir)?;

    tracing::info!(
        "started runner `{}` with pid {pid}; logs at {}",
        cli.name,
        process::log_path(&dir).display()
    );
    Ok(())
}
