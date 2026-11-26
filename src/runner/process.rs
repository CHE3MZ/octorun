//! Starting and stopping a configured runner process.

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::{Error, Result};

const PID_FILE: &str = ".octorun.pid";
const LOG_FILE: &str = ".octorun.log";

/// Path of the log file used by a started runner.
#[must_use]
pub fn log_path(dir: &Path) -> PathBuf {
    dir.join(LOG_FILE)
}

/// Start the runner in `dir` and return its process id.
///
/// The process id is recorded in a `.octorun.pid` file so that [`stop`] can
/// find it later.
///
/// # Errors
///
/// Returns an error if the runner is not configured, is already running, or
/// cannot be spawned.
pub fn start(dir: &Path) -> Result<u32> {
    let script = run_script_name();
    let script_path = dir.join(script);
    if !script_path.exists() {
        return Err(Error::Config(format!(
            "runner is not configured: missing `{}`",
            script_path.display()
        )));
    }

    if let Some(pid) = read_pid(dir)?
        && is_alive(pid)
    {
        return Err(Error::Config(format!(
            "runner is already running with pid {pid}"
        )));
    }

    let mut command = script_command(script);
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path(dir))?;
    let log_stderr = log.try_clone()?;

    command
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_stderr));

    let child = command.spawn()?;
    let pid = child.id();

    if let Err(error) = write_pid(dir, pid) {
        let _ = terminate(pid);
        return Err(error);
    }

    Ok(pid)
}

/// Stop the runner recorded in `dir` and return its process id.
///
/// # Errors
///
/// Returns an error if no process is recorded or the process cannot be
/// terminated.
pub fn stop(dir: &Path) -> Result<u32> {
    let pid = read_pid(dir)?.ok_or_else(|| {
        Error::Config(format!(
            "no runner process is recorded in `{}`",
            dir.display()
        ))
    })?;

    if is_alive(pid) {
        terminate(pid)?;
    } else {
        tracing::warn!("runner process {pid} is not running");
    }

    remove_pid(dir)?;
    Ok(pid)
}

fn pid_path(dir: &Path) -> PathBuf {
    dir.join(PID_FILE)
}

fn read_pid(dir: &Path) -> Result<Option<u32>> {
    match fs::read_to_string(pid_path(dir)) {
        Ok(contents) => Ok(contents.trim().parse().ok()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn write_pid(dir: &Path, pid: u32) -> Result<()> {
    fs::write(pid_path(dir), pid.to_string())?;
    Ok(())
}

fn remove_pid(dir: &Path) -> Result<()> {
    match fs::remove_file(pid_path(dir)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn run_script_name() -> &'static str {
    if cfg!(windows) { "run.cmd" } else { "run.sh" }
}

fn script_command(script: &str) -> Command {
    if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(script);
        command
    } else {
        Command::new(format!("./{script}"))
    }
}

#[cfg(windows)]
fn is_alive(pid: u32) -> bool {
    let filter = format!("PID eq {pid}");
    Command::new("tasklist")
        .args(["/FI", filter.as_str(), "/NH"])
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
        })
}

#[cfg(unix)]
fn is_alive(pid: u32) -> bool {
    let pid = pid.to_string();
    Command::new("kill")
        .args(["-0", pid.as_str()])
        .output()
        .is_ok_and(|output| output.status.success())
}

#[cfg(windows)]
fn terminate(pid: u32) -> Result<()> {
    let pid = pid.to_string();
    let output = Command::new("taskkill")
        .args(["/PID", pid.as_str(), "/T", "/F"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        return Err(Error::Command(format!(
            "could not stop process {pid}: {stderr}"
        )));
    }

    Ok(())
}

#[cfg(unix)]
fn terminate(pid: u32) -> Result<()> {
    let pid = pid.to_string();
    let output = Command::new("kill").arg(&pid).output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        return Err(Error::Command(format!(
            "could not stop process {pid}: {stderr}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PID_FILE, read_pid, remove_pid, run_script_name, start, stop, write_pid};

    #[test]
    fn records_and_clears_pid_file() {
        let dir = tempfile::tempdir().unwrap();

        assert_eq!(read_pid(dir.path()).unwrap(), None);
        write_pid(dir.path(), 4242).unwrap();
        assert_eq!(read_pid(dir.path()).unwrap(), Some(4242));
        assert!(dir.path().join(PID_FILE).exists());

        remove_pid(dir.path()).unwrap();
        assert_eq!(read_pid(dir.path()).unwrap(), None);
    }

    #[test]
    fn uses_platform_run_script() {
        let expected = if cfg!(windows) { "run.cmd" } else { "run.sh" };
        assert_eq!(run_script_name(), expected);
    }

    #[test]
    fn start_requires_configured_runner() {
        let dir = tempfile::tempdir().unwrap();
        assert!(start(dir.path()).is_err());
    }

    #[test]
    fn stop_requires_recorded_process() {
        let dir = tempfile::tempdir().unwrap();
        assert!(stop(dir.path()).is_err());
    }
}
