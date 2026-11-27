//! `octorun remove` — delete a runner.

use std::fs;
use std::path::{Path, PathBuf};

use super::{runner_dir, scope_for, validate_name};
use crate::cli::{Cli, RemoveArgs};
use crate::error::{Error, Result};
use crate::github::Repository;

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

        super::deregister(&runner_dir, host, &scope, cli.token.as_deref())?;
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
    use std::path::Path;

    use super::config_script;

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
}
