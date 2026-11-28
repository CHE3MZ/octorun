//! `octorun create` — create a new runner.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use super::cleanup::{DirGuard, cancel_flag};
use super::{runner_dir, runner_url, scope_for, validate_name};
use crate::cli::{Cli, CreateArgs};
use crate::error::{Error, Result};
use crate::github::{Client, Repository, Scope, TokenKind, resolve_tokens_with};
use crate::runner::RunnerPlatform;
use crate::runner::config::{self, Options};
use crate::runner::install;

/// Details about a runner that was just created.
pub(crate) struct Context {
    pub runner_dir: PathBuf,
    pub host: String,
    pub scope: Scope,
}

/// Create a runner from the given arguments.
///
/// With `--no-configuration`, download and extract the runner and write the
/// `.gitignore`, then skip registration and configuration.
///
/// # Errors
///
/// Returns an error if the repository, target, or token cannot be resolved, the
/// runner files cannot be installed, or the configuration script fails.
pub fn run(cli: &Cli, args: &CreateArgs) -> Result<()> {
    let cancelled = cancel_flag()?;
    let mut guard = DirGuard::new(runner_dir(cli));

    if args.no_configuration {
        let runner_dir = prepare_dir(cli, args, Some(&cancelled))?;
        let platform = resolve_platform(args)?;
        let api = Client::anonymous("github.com")?;
        install_runner(&api, args, &runner_dir, platform, Some(&cancelled))?;

        guard.disarm();
        tracing::info!(
            "downloaded runner `{}` to {} without configuring it",
            cli.name,
            runner_dir.display()
        );
        return Ok(());
    }

    let context = prepare(cli, args, false, Some(&cancelled), false)?;
    guard.disarm();

    tracing::info!(
        "runner `{}` is ready at {}",
        cli.name,
        context.runner_dir.display()
    );
    Ok(())
}

/// Install and configure a runner, returning the details needed to manage it.
///
/// `cancel` lets a caller stop long downloads, and `cleanup_on_error` removes
/// the runner directory and registration when setup fails after registration.
///
/// # Errors
///
/// Returns [`Error::Cancelled`] when cancellation is requested, or an error if
/// the repository, target, or token cannot be resolved, the runner files cannot
/// be installed, or the configuration script fails.
pub(crate) fn prepare(
    cli: &Cli,
    args: &CreateArgs,
    ephemeral: bool,
    cancel: Option<&AtomicBool>,
    cleanup_on_error: bool,
) -> Result<Context> {
    let runner_dir = prepare_dir(cli, args, cancel)?;

    let repository = match &cli.repo {
        Some(reference) => Repository::parse(reference)?,
        None => Repository::current()?,
    };
    let host = repository.host();

    let scope = scope_for(&repository, args.org.as_deref(), args.enterprise.as_deref())?;
    let url = runner_url(host, &scope);
    let platform = resolve_platform(args)?;

    check_cancel(cancel)?;
    let tokens = resolve_tokens_with(cli.token.as_deref(), host)?;
    let api = Client::with_tokens(host, tokens)?;

    install_runner(&api, args, &runner_dir, platform, cancel)?;

    check_cancel(cancel)?;
    let registration_token = api.runner_token(&scope, TokenKind::Registration)?;
    check_cancel(cancel)?;

    let options = Options {
        url,
        token: registration_token,
        name: cli.name.clone(),
        replace: args.overwrite,
        labels: args.labels.clone(),
        ephemeral,
    };

    if let Err(error) = config::configure(&runner_dir, &options) {
        if cleanup_on_error || cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
            cleanup_partial(&runner_dir, host, &scope, cli.token.as_deref());
        }
        return Err(error);
    }

    if let Err(error) = check_cancel(cancel) {
        cleanup_partial(&runner_dir, host, &scope, cli.token.as_deref());
        return Err(error);
    }

    Ok(Context {
        runner_dir,
        host: host.to_owned(),
        scope,
    })
}

fn prepare_dir(cli: &Cli, args: &CreateArgs, cancel: Option<&AtomicBool>) -> Result<PathBuf> {
    validate_name(&cli.name)?;
    check_cancel(cancel)?;

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

    Ok(runner_dir)
}

fn resolve_platform(args: &CreateArgs) -> Result<RunnerPlatform> {
    match args.platform {
        Some(platform) => Ok(platform),
        None => RunnerPlatform::detect(),
    }
}

fn install_runner(
    api: &Client,
    args: &CreateArgs,
    runner_dir: &Path,
    platform: RunnerPlatform,
    cancel: Option<&AtomicBool>,
) -> Result<()> {
    fs::create_dir_all(runner_dir)?;
    write_gitignore(runner_dir)?;
    install::install(api, args.location.as_deref(), runner_dir, platform, cancel)
}

fn check_cancel(cancel: Option<&AtomicBool>) -> Result<()> {
    if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

fn write_gitignore(dir: &Path) -> Result<()> {
    fs::write(dir.join(".gitignore"), "*\n")?;
    Ok(())
}

fn cleanup_partial(dir: &Path, host: &str, scope: &Scope, token: Option<&str>) {
    if dir.join(".runner").exists()
        && let Err(error) = super::deregister(dir, host, scope, token)
    {
        tracing::warn!("could not deregister runner: {error}");
    }

    if let Err(error) = fs::remove_dir_all(dir)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(
            "could not remove runner directory {}: {error}",
            dir.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::write_gitignore;

    #[test]
    fn ignores_the_runner_directory_contents() {
        let dir = tempfile::tempdir().unwrap();
        write_gitignore(dir.path()).unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join(".gitignore")).unwrap(),
            "*\n"
        );
    }
}
