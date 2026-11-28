//! `octorun restart` — restart a runner started by octorun.

use std::time::Duration;

use super::{runner_dir, validate_name};
use crate::cli::Cli;
use crate::error::Result;
use crate::runner::process;

const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Stop the runner if it is running, then start it again.
///
/// A runner that is not running is started. Restarting only relaunches the
/// configured directory; it does not update the runner files or re-register.
///
/// # Errors
///
/// Returns an error if the runner is not configured, the old process cannot be
/// stopped, it does not exit in time, or the new process cannot be started.
pub fn run(cli: &Cli) -> Result<()> {
    validate_name(&cli.name)?;
    let dir = runner_dir(cli);

    if let Some(pid) = process::stop_if_running(&dir)? {
        tracing::info!("stopped runner `{}` (pid {pid})", cli.name);
        process::wait_for_exit(pid, EXIT_TIMEOUT)?;
    }

    let pid = process::start(&dir)?;
    tracing::info!(
        "started runner `{}` with pid {pid}; logs at {}",
        cli.name,
        process::log_path(&dir).display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use clap::Parser;

    use super::run;
    use crate::cli::Cli;
    use crate::runner::process;

    fn script_name() -> &'static str {
        if cfg!(windows) { "run.cmd" } else { "run.sh" }
    }

    #[cfg(unix)]
    fn sleeping_script() -> &'static str {
        "#!/bin/sh\nsleep 30\n"
    }

    #[cfg(windows)]
    fn sleeping_script() -> &'static str {
        "@ping -n 30 127.0.0.1 >NUL\r\n"
    }

    fn write_run_script(dir: &Path) {
        let path = dir.join(script_name());
        fs::write(&path, sleeping_script()).unwrap();

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
            "restart",
        ])
        .unwrap()
    }

    fn create_runner_dir(parent: &Path) -> PathBuf {
        let runner = parent.join("runner");
        fs::create_dir_all(&runner).unwrap();
        write_run_script(&runner);
        runner
    }

    fn read_pid(dir: &Path) -> u32 {
        fs::read_to_string(dir.join(".octorun.pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    }

    #[test]
    fn starts_when_the_runner_is_not_running() {
        let parent = tempfile::tempdir().unwrap();
        let runner = create_runner_dir(parent.path());
        let cli = cli_for(parent.path());

        run(&cli).unwrap();

        assert!(runner.join(".octorun.pid").exists());
        process::stop(&runner).unwrap();
    }

    #[test]
    fn replaces_a_running_process() {
        let parent = tempfile::tempdir().unwrap();
        let runner = create_runner_dir(parent.path());
        let cli = cli_for(parent.path());

        let mut child = process::spawn(&runner).unwrap();
        let old_pid = child.id();
        let waiter = std::thread::spawn(move || {
            let _ = child.wait();
        });

        run(&cli).unwrap();
        waiter.join().unwrap();

        assert_ne!(read_pid(&runner), old_pid);
        process::stop(&runner).unwrap();
    }

    #[test]
    fn requires_a_configured_runner() {
        let parent = tempfile::tempdir().unwrap();
        fs::create_dir_all(parent.path().join("runner")).unwrap();
        let cli = cli_for(parent.path());

        assert!(run(&cli).is_err());
    }
}
