//! `octorun start` — start a preconfigured runner.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use super::cleanup::cancel_flag;
use super::{runner_dir, validate_name};
use crate::cli::{Cli, StartArgs};
use crate::error::Result;
use crate::runner::process;

/// Start the named runner as a local process.
///
/// # Errors
///
/// Returns an error if the runner is not configured or cannot be started.
pub fn run(cli: &Cli, args: &StartArgs) -> Result<()> {
    validate_name(&cli.name)?;
    let dir = runner_dir(cli);

    if args.interactive {
        let cancelled = cancel_flag()?;
        return run_interactive(&dir, &cli.name, &cancelled);
    }

    let pid = process::start(&dir)?;
    tracing::info!(
        "started runner `{}` with pid {pid}; logs at {}",
        cli.name,
        process::log_path(&dir).display()
    );
    Ok(())
}

pub(crate) fn run_interactive(dir: &Path, name: &str, cancelled: &AtomicBool) -> Result<()> {
    let mut child = process::spawn_interactive(dir)?;
    tracing::info!("runner `{name}` is running; press Ctrl+C to stop");

    match process::wait(&mut child, cancelled)? {
        process::WaitOutcome::Exited(status) => {
            if status.success() {
                tracing::info!("runner `{name}` exited");
            } else {
                tracing::warn!("runner `{name}` exited with {status}");
            }
        }
        process::WaitOutcome::Cancelled => {
            tracing::info!("stopping runner `{name}`");
            let _ = process::stop_if_running(dir);
            let _ = child.wait();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::AtomicBool;

    use clap::Parser;

    use super::{run, run_interactive};
    use crate::cli::{Cli, StartArgs};
    use crate::commands::stop;

    fn script_name() -> &'static str {
        if cfg!(windows) { "run.cmd" } else { "run.sh" }
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
    fn sleeping_script() -> &'static str {
        "#!/bin/sh\nsleep 30\n"
    }

    #[cfg(windows)]
    fn sleeping_script() -> &'static str {
        "@ping -n 30 127.0.0.1 >NUL\r\n"
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

    fn cli_for(dir: &Path) -> Cli {
        Cli::try_parse_from([
            "octorun",
            "-d",
            dir.to_str().unwrap(),
            "-n",
            "runner",
            "start",
        ])
        .unwrap()
    }

    fn args(interactive: bool) -> StartArgs {
        StartArgs { interactive }
    }

    #[test]
    fn starts_and_stops_a_runner() {
        let parent = tempfile::tempdir().unwrap();
        let runner = parent.path().join("runner");
        fs::create_dir_all(&runner).unwrap();
        write_run_script(&runner, sleeping_script());

        let cli = cli_for(parent.path());
        run(&cli, &args(false)).unwrap();
        assert!(runner.join(".octorun.pid").exists());

        stop::run(&cli).unwrap();
        assert!(!runner.join(".octorun.pid").exists());
    }

    #[test]
    fn interactive_waits_for_the_runner() {
        let parent = tempfile::tempdir().unwrap();
        let runner = parent.path().join("runner");
        fs::create_dir_all(&runner).unwrap();
        write_run_script(&runner, instant_script());

        run_interactive(&runner, "runner", &AtomicBool::new(false)).unwrap();
    }

    #[test]
    fn interactive_stops_the_runner_on_cancel() {
        let parent = tempfile::tempdir().unwrap();
        let runner = parent.path().join("runner");
        fs::create_dir_all(&runner).unwrap();
        write_run_script(&runner, sleeping_script());

        run_interactive(&runner, "runner", &AtomicBool::new(true)).unwrap();

        assert!(!runner.join(".octorun.pid").exists());
    }
}
