//! `octorun create` — create a new runner.

use crate::cli::{Cli, CreateArgs};
use crate::error::{Error, Result};

/// Create a runner from the given arguments.
///
/// # Errors
///
/// Returns an error if the runner cannot be created.
pub fn run(cli: &Cli, args: &CreateArgs) -> Result<()> {
    tracing::debug!(name = %cli.name, ?args, "create requested");
    Err(Error::Unimplemented("create".to_owned()))
}
