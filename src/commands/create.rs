//! `octorun create` — create a new runner.

use std::fs;

use super::{runner_dir, runner_url, scope_for, validate_name};
use crate::cli::{Cli, CreateArgs};
use crate::error::{Error, Result};
use crate::github::{Client, Repository, TokenKind, resolve_token};
use crate::runner::config::{self, Options};
use crate::runner::install;

/// Create a runner from the given arguments.
///
/// # Errors
///
/// Returns an error if the repository, target, or token cannot be resolved, the
/// runner files cannot be installed, or the configuration script fails.
pub fn run(cli: &Cli, args: &CreateArgs) -> Result<()> {
    validate_name(&cli.name)?;

    let runner_dir = runner_dir(cli);
    if let Some(parent) = runner_dir.parent() {
        fs::create_dir_all(parent)?;
    }

    if runner_dir.exists() {
        if !args.overwrite {
            return Err(Error::Config(format!(
                "runner directory `{}` already exists; use --overwrite to replace it",
                runner_dir.display()
            )));
        }
        fs::remove_dir_all(&runner_dir)?;
        tracing::info!("removed existing runner directory {}", runner_dir.display());
    }

    let repository = match &cli.repo {
        Some(reference) => Repository::parse(reference)?,
        None => Repository::current()?,
    };
    let host = repository.host();

    let scope = scope_for(&repository, args.org.as_deref(), args.enterprise.as_deref())?;
    let url = runner_url(host, &scope);

    let token = resolve_token(host)?;
    let api = Client::new(host, token)?;

    install::install(&api, args.location.as_deref(), &runner_dir)?;

    let registration_token = api.runner_token(&scope, TokenKind::Registration)?;
    let options = Options {
        url,
        token: registration_token,
        name: cli.name.clone(),
        replace: args.overwrite,
        labels: args.labels.clone(),
        group: args.group.clone(),
    };
    config::configure(&runner_dir, &options)?;

    tracing::info!("runner `{}` is ready at {}", cli.name, runner_dir.display());
    Ok(())
}
