//! `octorun temp` — create a runner, use it, then remove it.

use std::fs;

use super::cleanup::{DirGuard, cancel_flag};
use super::create::{self, Context};
use super::{deregister, runner_dir};
use crate::cli::{Cli, TempArgs};
use crate::error::Result;
use crate::runner::process;

/// Create a temporary runner, wait for it, then remove it.
///
/// Without `--ephemeral` the runner keeps accepting jobs until the process is
/// stopped with Ctrl+C. With `--ephemeral` it removes itself after one job.
///
/// # Errors
///
/// Returns an error if the runner cannot be created, started, or removed.
pub fn run(cli: &Cli, args: &TempArgs) -> Result<()> {
    let cancelled = cancel_flag()?;
    let mut guard = DirGuard::new(runner_dir(cli));

    let context = create::prepare(cli, &args.create, args.ephemeral, Some(&cancelled), true)?;
    guard.disarm();

    let mut child = match process::spawn(&context.runner_dir) {
        Ok(child) => child,
        Err(error) => {
            let _ = cleanup(&context, cli.token.as_deref());
            return Err(error);
        }
    };
    tracing::info!("temporary runner `{}` is running", cli.name);

    let outcome = match process::wait(&mut child, &cancelled) {
        Ok(outcome) => outcome,
        Err(error) => {
            let _ = process::stop(&context.runner_dir);
            let _ = child.wait();
            let _ = cleanup(&context, cli.token.as_deref());
            return Err(error);
        }
    };

    match outcome {
        process::WaitOutcome::Exited(status) => {
            if status.success() {
                tracing::info!("runner exited");
            } else {
                tracing::warn!("runner exited with {status}");
            }
        }
        process::WaitOutcome::Cancelled => {
            tracing::info!("stopping runner");
            if let Err(error) = process::stop(&context.runner_dir) {
                tracing::warn!("could not stop runner: {error}");
            }
            let _ = child.wait();
        }
    }

    cleanup(&context, cli.token.as_deref())
}

fn cleanup(context: &Context, token: Option<&str>) -> Result<()> {
    let dir = &context.runner_dir;

    if dir.join(".runner").exists()
        && let Err(error) = deregister(dir, &context.host, &context.scope, token)
    {
        tracing::warn!("could not deregister runner: {error}");
    }

    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::cleanup;
    use crate::commands::create::Context;
    use crate::github::Scope;

    fn context(runner_dir: std::path::PathBuf) -> Context {
        Context {
            runner_dir,
            host: "github.com".to_owned(),
            scope: Scope::Repository {
                owner: "acme".to_owned(),
                name: "app".to_owned(),
            },
        }
    }

    #[test]
    fn cleanup_removes_runner_directory() {
        let dir = tempfile::tempdir().unwrap();
        let runner = dir.path().join("runner");
        fs::create_dir_all(&runner).unwrap();
        fs::write(runner.join("config.sh"), b"stub").unwrap();

        cleanup(&context(runner.clone()), None).unwrap();

        assert!(!runner.exists());
    }

    #[test]
    fn cleanup_tolerates_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let runner = dir.path().join("missing");

        cleanup(&context(runner), None).unwrap();
    }
}
