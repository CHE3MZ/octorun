//! Starting and stopping a configured runner process.

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};

const PID_FILE: &str = ".octorun.pid";
const LOG_FILE: &str = ".octorun.log";

/// Path of the log file used by a started runner.
#[must_use]
pub fn log_path(dir: &Path) -> PathBuf {
    dir.join(LOG_FILE)
}

/// How a watched runner process finished.
#[derive(Debug)]
#[non_exhaustive]
pub enum WaitOutcome {
    /// The process exited on its own.
    Exited(ExitStatus),
    /// A cancellation was requested.
    Cancelled,
}

/// Start the runner in `dir` and return its process id.
///
/// # Errors
///
/// Returns an error if the runner is not configured, is already running, or
/// cannot be spawned.
pub fn start(dir: &Path) -> Result<u32> {
    let child = spawn(dir)?;
    Ok(child.id())
}

/// Start the runner in `dir` and return the child process.
///
/// The process id is recorded in a `.octorun.pid` file so that [`stop`] can
/// find it later.
///
/// # Errors
///
/// Returns an error if the runner is not configured, is already running, or
/// cannot be spawned.
pub fn spawn(dir: &Path) -> Result<Child> {
    let mut command = prepare_command(dir)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path(dir))?;
    let log_stderr = log.try_clone()?;

    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_stderr));

    spawn_and_record(dir, command)
}

/// Start the runner in `dir` with the terminal's standard streams.
///
/// The caller is expected to wait for the child so the runner keeps the
/// foreground process group and receives Ctrl+C.
///
/// # Errors
///
/// Returns an error if the runner is not configured, is already running, or
/// cannot be spawned.
pub(crate) fn spawn_interactive(dir: &Path) -> Result<Child> {
    let mut command = prepare_command(dir)?;
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    spawn_and_record(dir, command)
}

fn prepare_command(dir: &Path) -> Result<Command> {
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
    command.current_dir(dir);
    Ok(command)
}

fn spawn_and_record(dir: &Path, mut command: Command) -> Result<Child> {
    let child = command.spawn()?;
    let pid = child.id();

    if let Err(error) = write_pid(dir, pid) {
        let _ = terminate(pid);
        return Err(error);
    }

    Ok(child)
}

/// Wait for `child` to exit or for `cancelled` to be set.
///
/// # Errors
///
/// Returns an error if the child status cannot be read.
pub fn wait(child: &mut Child, cancelled: &AtomicBool) -> Result<WaitOutcome> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(WaitOutcome::Exited(status));
        }
        if cancelled.load(Ordering::SeqCst) {
            return Ok(WaitOutcome::Cancelled);
        }
        thread::sleep(Duration::from_millis(250));
    }
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

/// Stop the runner recorded in `dir` if it is still alive.
///
/// Returns the pid when a live process was terminated, or `None` when no
/// process was running.
///
/// # Errors
///
/// Returns an error if the pid cannot be read or the process cannot be
/// terminated.
pub(crate) fn stop_if_running(dir: &Path) -> Result<Option<u32>> {
    let Some(pid) = read_pid(dir)? else {
        return Ok(None);
    };

    if !is_alive(pid) {
        tracing::warn!("runner process {pid} is not running");
        remove_pid(dir)?;
        return Ok(None);
    }

    terminate(pid)?;
    remove_pid(dir)?;
    Ok(Some(pid))
}

/// Wait for `pid` to exit, polling until `timeout` elapses.
///
/// # Errors
///
/// Returns an error if the process is still alive after `timeout`.
pub(crate) fn wait_for_exit(pid: u32, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        if !is_alive(pid) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }

    if is_alive(pid) {
        return Err(Error::Command(format!("runner process {pid} did not exit")));
    }

    Ok(())
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
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    use super::{
        PID_FILE, WaitOutcome, log_path, pid_path, read_pid, remove_pid, run_script_name, spawn,
        start, stop, stop_if_running, wait, wait_for_exit, write_pid,
    };
    use crate::error::Error;

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
        let path = dir.join(run_script_name());
        fs::write(&path, body).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).unwrap();
        }
    }

    const DEAD_PID: u32 = 4_294_967_294;

    #[test]
    fn records_the_log_path() {
        assert!(log_path(Path::new("runner")).ends_with(".octorun.log"));
    }

    #[test]
    fn spawns_and_waits_for_the_runner() {
        let dir = tempfile::tempdir().unwrap();
        write_run_script(dir.path(), instant_script());

        let mut child = spawn(dir.path()).unwrap();
        assert!(dir.path().join(PID_FILE).exists());

        let outcome = wait(&mut child, &AtomicBool::new(false)).unwrap();
        assert!(matches!(outcome, WaitOutcome::Exited(status) if status.success()));
    }

    #[test]
    fn spawn_rejects_a_running_runner() {
        let dir = tempfile::tempdir().unwrap();
        write_run_script(dir.path(), instant_script());
        write_pid(dir.path(), std::process::id()).unwrap();

        assert!(spawn(dir.path()).is_err());
    }

    #[test]
    fn spawn_replaces_a_stale_pid() {
        let dir = tempfile::tempdir().unwrap();
        write_run_script(dir.path(), instant_script());
        write_pid(dir.path(), DEAD_PID).unwrap();

        let mut child = spawn(dir.path()).unwrap();
        let outcome = wait(&mut child, &AtomicBool::new(false)).unwrap();

        assert!(matches!(outcome, WaitOutcome::Exited(_)));
    }

    #[test]
    fn wait_reports_cancellation() {
        let dir = tempfile::tempdir().unwrap();
        write_run_script(dir.path(), sleeping_script());

        let mut child = spawn(dir.path()).unwrap();
        let outcome = wait(&mut child, &AtomicBool::new(true)).unwrap();
        assert!(matches!(outcome, WaitOutcome::Cancelled));

        stop(dir.path()).unwrap();
        let _ = child.wait();
    }

    #[test]
    fn stop_terminates_the_recorded_process() {
        let dir = tempfile::tempdir().unwrap();
        write_run_script(dir.path(), sleeping_script());

        let mut child = spawn(dir.path()).unwrap();
        let pid = stop(dir.path()).unwrap();

        assert_eq!(pid, child.id());
        assert!(!dir.path().join(PID_FILE).exists());
        let _ = child.wait();
    }

    #[test]
    fn stop_tolerates_a_dead_process() {
        let dir = tempfile::tempdir().unwrap();
        write_pid(dir.path(), DEAD_PID).unwrap();

        assert_eq!(stop(dir.path()).unwrap(), DEAD_PID);
        assert!(!dir.path().join(PID_FILE).exists());
    }

    #[test]
    fn reads_invalid_pid_contents_as_none() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(pid_path(dir.path()), "not-a-pid").unwrap();

        assert_eq!(read_pid(dir.path()).unwrap(), None);
    }

    #[test]
    fn stop_if_running_ignores_a_missing_pid() {
        let dir = tempfile::tempdir().unwrap();

        assert_eq!(stop_if_running(dir.path()).unwrap(), None);
    }

    #[test]
    fn stop_if_running_clears_a_stale_pid() {
        let dir = tempfile::tempdir().unwrap();
        write_pid(dir.path(), DEAD_PID).unwrap();

        assert_eq!(stop_if_running(dir.path()).unwrap(), None);
        assert!(!dir.path().join(PID_FILE).exists());
    }

    #[test]
    fn stop_if_running_terminates_a_live_process() {
        let dir = tempfile::tempdir().unwrap();
        write_run_script(dir.path(), sleeping_script());

        let mut child = spawn(dir.path()).unwrap();
        let pid = child.id();
        let waiter = std::thread::spawn(move || {
            let _ = child.wait();
        });

        assert_eq!(stop_if_running(dir.path()).unwrap(), Some(pid));
        waiter.join().unwrap();
        assert!(!dir.path().join(PID_FILE).exists());
    }

    #[test]
    fn waits_for_a_dead_process() {
        wait_for_exit(DEAD_PID, Duration::from_secs(1)).unwrap();
    }

    #[test]
    fn times_out_on_a_live_process() {
        let error = wait_for_exit(std::process::id(), Duration::from_millis(50)).unwrap_err();

        assert!(matches!(error, Error::Command(_)));
    }
}
