//! `octorun temp` — create a runner, use it, then remove it.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
    let cancelled = cancel_flag()?;
    let mut guard = DirGuard::new(runner_dir(cli));

    let context = create::prepare(cli, &args.create, args.ephemeral, Some(&cancelled), true)?;
    guard.disarm();

    let mut child = match process::spawn(&context.runner_dir) {
        Ok(child) => child,
        Err(error) => {
            let _ = cleanup(&context);
            return Err(error);
        }
    };
    tracing::info!("temporary runner `{}` is running", cli.name);

    let outcome = match process::wait(&mut child, &cancelled) {
        Ok(outcome) => outcome,
        Err(error) => {
            let _ = process::stop(&context.runner_dir);
            let _ = child.wait();
            let _ = cleanup(&context);
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

    cleanup(&context)
}

fn cancel_flag() -> Result<Arc<AtomicBool>> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let handler = Arc::clone(&cancelled);

    ctrlc::set_handler(move || {
        handler.store(true, Ordering::SeqCst);
    })
    .map_err(|error| Error::Command(format!("could not install Ctrl+C handler: {error}")))?;

    Ok(cancelled)
}

fn cleanup(context: &Context) -> Result<()> {
    let dir = &context.runner_dir;

    if dir.join(".runner").exists()
        && let Err(error) = deregister(dir, &context.host, &context.scope)
    {
        tracing::warn!("could not deregister runner: {error}");
    }

    match fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Removes a runner directory if setup does not finish.
struct DirGuard {
    dir: PathBuf,
    armed: bool,
}

impl DirGuard {
    fn new(dir: PathBuf) -> Self {
        let armed = !dir.exists();
        Self { dir, armed }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{DirGuard, cleanup};
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

        cleanup(&context(runner.clone())).unwrap();

        assert!(!runner.exists());
    }

    #[test]
    fn cleanup_tolerates_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let runner = dir.path().join("missing");

        cleanup(&context(runner)).unwrap();
    }

    #[test]
    fn guard_removes_only_new_directories() {
        let dir = tempfile::tempdir().unwrap();

        let fresh = dir.path().join("fresh");
        {
            let _guard = DirGuard::new(fresh.clone());
            fs::create_dir_all(&fresh).unwrap();
        }
        assert!(!fresh.exists());

        let existing = dir.path().join("existing");
        fs::create_dir_all(&existing).unwrap();
        {
            let _guard = DirGuard::new(existing.clone());
        }
        assert!(existing.exists());
    }
}
