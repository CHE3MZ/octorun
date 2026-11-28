//! `octorun remove` — delete a runner.

use std::fs;
use std::path::{Path, PathBuf};

use super::{runner_dir, scope_for, validate_name};
use crate::cli::{Cli, RemoveArgs};
use crate::error::{Error, Result};
use crate::github::{Repository, Scope};

/// Remove a runner and its directory.
///
/// The runner is deregistered when its configuration script is present. Without
/// the script the directory is deleted but the GitHub registration may stay.
///
/// # Errors
///
/// Returns an error if the runner directory is missing, the token cannot be
/// resolved, deregistration fails, or the directory cannot be deleted.
pub fn run(cli: &Cli, args: &RemoveArgs) -> Result<()> {
    remove_with(cli, args, super::deregister)
}

fn remove_with(
    cli: &Cli,
    args: &RemoveArgs,
    deregister: impl FnOnce(&Path, &str, &Scope, Option<&str>) -> Result<()>,
) -> Result<()> {
    validate_name(&cli.name)?;
    let runner_dir = runner_dir(cli);

    if !runner_dir.exists() {
        return Err(Error::Config(format!(
            "runner directory `{}` does not exist",
            runner_dir.display()
        )));
    }

    if config_script(&runner_dir).exists() {
        let repository = match &cli.repo {
            Some(reference) => Repository::parse(reference)?,
            None => Repository::current()?,
        };
        let host = repository.host();
        let scope = scope_for(&repository, args.org.as_deref(), args.enterprise.as_deref())?;

        deregister(&runner_dir, host, &scope, cli.token.as_deref())?;
    } else {
        tracing::warn!(
            "no configuration script found; deleting the directory without deregistering"
        );
    }

    fs::remove_dir_all(&runner_dir)?;
    tracing::info!("removed runner `{}` at {}", cli.name, runner_dir.display());
    Ok(())
}

fn config_script(dir: &Path) -> PathBuf {
    dir.join(if cfg!(windows) {
        "config.cmd"
    } else {
        "config.sh"
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::Path;

    use clap::Parser;

    use super::{config_script, remove_with};
    use crate::cli::{Cli, Command, RemoveArgs};
    use crate::error::{Error, Result};
    use crate::github::Scope;

    fn cli_for(dir: &Path) -> (Cli, RemoveArgs) {
        let cli = Cli::try_parse_from([
            "octorun",
            "-r",
            "acme/app",
            "-d",
            dir.to_str().unwrap(),
            "-n",
            "runner",
            "remove",
        ])
        .unwrap();
        let args = match &cli.command {
            Command::Remove(args) => args.clone(),
            _ => panic!("expected remove"),
        };
        (cli, args)
    }

    fn script_name() -> &'static str {
        if cfg!(windows) {
            "config.cmd"
        } else {
            "config.sh"
        }
    }

    #[test]
    fn uses_platform_config_script() {
        let expected = if cfg!(windows) {
            "config.cmd"
        } else {
            "config.sh"
        };
        assert_eq!(
            config_script(Path::new("/tmp/runner")),
            Path::new("/tmp/runner").join(expected)
        );
    }

    #[test]
    fn rejects_a_missing_runner_directory() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, args) = cli_for(dir.path());

        assert!(remove_with(&cli, &args, |_, _, _, _| Ok(())).is_err());
    }

    #[test]
    fn removes_a_runner_without_a_config_script() {
        let dir = tempfile::tempdir().unwrap();
        let runner = dir.path().join("runner");
        fs::create_dir_all(&runner).unwrap();
        let (cli, args) = cli_for(dir.path());

        remove_with(&cli, &args, |_, _, _, _| Ok(())).unwrap();

        assert!(!runner.exists());
    }

    #[test]
    fn deregisters_a_configured_runner() {
        let dir = tempfile::tempdir().unwrap();
        let runner = dir.path().join("runner");
        fs::create_dir_all(&runner).unwrap();
        fs::write(runner.join(script_name()), b"stub").unwrap();
        let (cli, args) = cli_for(dir.path());
        let called = Cell::new(false);

        remove_with(
            &cli,
            &args,
            |path: &Path, host: &str, scope: &Scope, token: Option<&str>| -> Result<()> {
                called.set(true);
                assert_eq!(path, runner.as_path());
                assert_eq!(host, "github.com");
                assert!(matches!(scope, Scope::Repository { .. }));
                assert!(token.is_none());
                Ok(())
            },
        )
        .unwrap();

        assert!(called.get());
        assert!(!runner.exists());
    }

    #[test]
    fn keeps_the_directory_when_deregistration_fails() {
        let dir = tempfile::tempdir().unwrap();
        let runner = dir.path().join("runner");
        fs::create_dir_all(&runner).unwrap();
        fs::write(runner.join(script_name()), b"stub").unwrap();
        let (cli, args) = cli_for(dir.path());

        let error = remove_with(&cli, &args, |_, _, _, _| -> Result<()> {
            Err(Error::Config("nope".to_owned()))
        })
        .unwrap_err();

        assert!(matches!(error, Error::Config(_)));
        assert!(runner.exists());
    }
}
