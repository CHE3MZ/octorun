//! `octorun start` — start a preconfigured runner.

use super::{runner_dir, validate_name};
use crate::cli::Cli;
use crate::error::Result;
use crate::runner::process;

/// Start the named runner as a local process.
///
/// # Errors
///
/// Returns an error if the runner is not configured or cannot be started.
pub fn run(cli: &Cli) -> Result<()> {
    validate_name(&cli.name)?;
    let dir = runner_dir(cli);
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
    use std::path::Path;

    use clap::Parser;

    use super::run;
    use crate::cli::Cli;
    use crate::commands::stop;

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

    #[test]
    fn starts_and_stops_a_runner() {
        let parent = tempfile::tempdir().unwrap();
        let runner = parent.path().join("runner");
        fs::create_dir_all(&runner).unwrap();
        let script = runner.join(script_name());
        fs::write(&script, sleeping_script()).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&script).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script, permissions).unwrap();
        }

        let cli = cli_for(parent.path());
        run(&cli).unwrap();
        assert!(runner.join(".octorun.pid").exists());

        stop::run(&cli).unwrap();
        assert!(!runner.join(".octorun.pid").exists());
    }
}
