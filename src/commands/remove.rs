//! `octorun remove` — delete a runner.

use crate::cli::{Cli, RemoveArgs};
use crate::error::{Error, Result};

/// Remove a runner from the given arguments.
///
/// # Errors
///
/// Returns an error if the runner cannot be removed.
pub fn run(cli: &Cli, args: &RemoveArgs) -> Result<()> {
    tracing::debug!(name = %cli.name, ?args, "remove requested");
    Err(Error::Unimplemented("remove".to_owned()))
}
