//! Subcommand handlers and dispatch.

pub mod create;
pub mod remove;
pub mod start;
pub mod stop;

use crate::cli::{Cli, Command};
use crate::error::Result;

/// Dispatch a parsed [`Cli`] to the matching subcommand handler.
///
/// # Errors
///
/// Returns an error if the selected subcommand fails.
pub fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Create(args) => create::run(cli, args),
        Command::Remove(args) => remove::run(cli, args),
        Command::Start => start::run(cli),
        Command::Stop => stop::run(cli),
    }
}
