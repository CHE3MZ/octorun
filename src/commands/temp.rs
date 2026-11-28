//! `octorun temp` — create a runner, use it, then remove it.

use std::fs;
use std::sync::atomic::AtomicBool;

use super::cleanup::{DirGuard, cancel_flag};
use super::create::{self, Context};
use super::{deregister, runner_dir};
use crate::cli::{Cli, TempArgs};
use crate::error::{Error, Result};
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
    if args.create.no_configuration {
        return Err(Error::Config(
            "temp runners must be registered, so --no-configuration cannot be used".to_owned(),
        ));
    }

    let cancelled = cancel_flag()?;
    let mut guard = DirGuard::new(runner_dir(cli));

    let context = create::prepare(cli, &args.create, args.ephemeral, Some(&cancelled), true)?;
    guard.disarm();

    run_runner(&context, cli, &cancelled)
}

fn run_runner(context: &Context, cli: &Cli, cancelled: &AtomicBool) -> Result<()> {
    let mut child = match process::spawn(&context.runner_dir) {
        Ok(child) => child,
        Err(error) => {
            let _ = cleanup(context, cli.token.as_deref());
            return Err(error);
        }
    };
    tracing::info!("temporary runner `{}` is running", cli.name);

    let outcome = match process::wait(&mut child, cancelled) {
        Ok(outcome) => outcome,
        Err(error) => {
            let _ = process::stop(&context.runner_dir);
            let _ = child.wait();
            let _ = cleanup(context, cli.token.as_deref());
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

    cleanup(context, cli.token.as_deref())
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
    use std::path::{Path, PathBuf};
    use std::sync::atomic::AtomicBool;

    use clap::Parser;

    use super::{cleanup, run_runner};
    use crate::cli::Cli;
    use crate::commands::create::Context;
    use crate::github::Scope;

    fn context(runner_dir: PathBuf) -> Context {
        Context {
            runner_dir,
            host: "github.com".to_owned(),
            scope: Scope::Repository {
                owner: "acme".to_owned(),
                name: "app".to_owned(),
            },
        }
    }

    fn script_name() -> &'static str {
        if cfg!(windows) { "run.cmd" } else { "run.sh" }
    }

    fn write_run_script(dir: &Path, body: &str) {
        let path = dir.join(script_name());
        fs::write(&path, body).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).unwrap();
        }
    }

    #[cfg(unix)]
    fn instant_script() -> &'static str {
        "#!/bin/sh\nexit 0\n"
    }

    #[cfg(windows)]
    fn instant_script() -> &'static str {
        "@exit /b 0\r\n"
    }

    #[cfg(unix)]
    fn failing_script() -> &'static str {
        "#!/bin/sh\nexit 3\n"
    }

    #[cfg(windows)]
    fn failing_script() -> &'static str {
        "@exit /b 3\r\n"
    }

    #[cfg(unix)]
    fn sleeping_script() -> &'static str {
        "#!/bin/sh\nsleep 30\n"
    }

    #[cfg(windows)]
    fn sleeping_script() -> &'static str {
        "@ping -n 30 127.0.0.1 >NUL\r\n"
    }

    fn cli_for(dir: &Path) -> Cli {
        Cli::try_parse_from([
            "octorun",
            "-d",
            dir.to_str().unwrap(),
            "-n",
            "runner",
            "temp",
        ])
        .unwrap()
    }

    fn create_runner_dir(parent: &Path) -> PathBuf {
        let runner = parent.join("runner");
        fs::create_dir_all(&runner).unwrap();
        runner
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

    #[test]
    fn runs_and_removes_a_successful_runner() {
        let parent = tempfile::tempdir().unwrap();
        let runner = create_runner_dir(parent.path());
        write_run_script(&runner, instant_script());
        let cli = cli_for(parent.path());

        run_runner(&context(runner.clone()), &cli, &AtomicBool::new(false)).unwrap();

        assert!(!runner.exists());
    }

    #[test]
    fn runs_and_removes_a_failing_runner() {
        let parent = tempfile::tempdir().unwrap();
        let runner = create_runner_dir(parent.path());
        write_run_script(&runner, failing_script());
        let cli = cli_for(parent.path());

        run_runner(&context(runner.clone()), &cli, &AtomicBool::new(false)).unwrap();

        assert!(!runner.exists());
    }

    #[test]
    fn stops_and_removes_a_cancelled_runner() {
        let parent = tempfile::tempdir().unwrap();
        let runner = create_runner_dir(parent.path());
        write_run_script(&runner, sleeping_script());
        let cli = cli_for(parent.path());

        run_runner(&context(runner.clone()), &cli, &AtomicBool::new(true)).unwrap();

        assert!(!runner.exists());
    }

    #[test]
    fn removes_the_directory_when_the_runner_cannot_start() {
        let parent = tempfile::tempdir().unwrap();
        let runner = create_runner_dir(parent.path());
        let cli = cli_for(parent.path());

        assert!(run_runner(&context(runner.clone()), &cli, &AtomicBool::new(false)).is_err());
        assert!(!runner.exists());
    }
}
