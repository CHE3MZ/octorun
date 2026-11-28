//! Configuring and removing a runner through its bundled scripts.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{Error, Result};

/// Values passed to `config.sh` or `config.cmd`.
#[derive(Debug, Clone)]
pub struct Options {
    /// URL of the repository, organization, or enterprise.
    pub url: String,
    /// Runner registration token.
    pub token: String,
    /// Display name for the runner.
    pub name: String,
    /// Replace an existing runner with the same name.
    pub replace: bool,
    /// Comma separated list of extra labels.
    pub labels: Option<String>,
    /// Run a single job, then deregister the runner.
    pub ephemeral: bool,
}

/// Build the argument list for `config.sh` / `config.cmd`.
#[must_use]
pub fn config_args(options: &Options) -> Vec<String> {
    let mut args = vec![
        "--url".to_owned(),
        options.url.clone(),
        "--token".to_owned(),
        options.token.clone(),
        "--name".to_owned(),
        options.name.clone(),
        "--unattended".to_owned(),
    ];

    if options.replace {
        args.push("--replace".to_owned());
    }
    if options.ephemeral {
        args.push("--ephemeral".to_owned());
    }
    if let Some(labels) = &options.labels {
        args.push("--labels".to_owned());
        args.push(labels.clone());
    }

    args
}

/// Build the argument list used to remove a runner.
#[must_use]
pub fn remove_args(token: &str) -> Vec<String> {
    vec!["remove".to_owned(), "--token".to_owned(), token.to_owned()]
}

/// Configure the runner in `dir`.
///
/// The configuration script inherits the current standard streams so the user
/// can see its output.
///
/// # Errors
///
/// Returns an error if the script cannot be started or exits unsuccessfully.
pub fn configure(dir: &Path, options: &Options) -> Result<()> {
    let mut command = script_command();
    command.args(config_args(options));
    run_script(dir, "configuration", command)
}

/// Deregister the runner in `dir` using `token`.
///
/// # Errors
///
/// Returns an error if the script cannot be started or exits unsuccessfully.
pub fn remove(dir: &Path, token: &str) -> Result<()> {
    let mut command = script_command();
    command.args(remove_args(token));
    run_script(dir, "removal", command)
}

fn script_command() -> Command {
    if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.arg("/C").arg("config.cmd");
        command
    } else {
        Command::new("./config.sh")
    }
}

fn run_script(dir: &Path, action: &str, mut command: Command) -> Result<()> {
    command
        .current_dir(dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = command.status()?;
    if !status.success() {
        return Err(Error::Command(format!(
            "runner {action} failed with {status}"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{Options, config_args, configure, remove, remove_args};

    fn options() -> Options {
        Options {
            url: "https://github.com/acme/app".to_owned(),
            token: "secret".to_owned(),
            name: "actions-runner".to_owned(),
            replace: false,
            labels: None,
            ephemeral: false,
        }
    }

    #[test]
    fn builds_minimal_arguments() {
        assert_eq!(
            config_args(&options()),
            vec![
                "--url",
                "https://github.com/acme/app",
                "--token",
                "secret",
                "--name",
                "actions-runner",
                "--unattended",
            ]
        );
    }

    #[test]
    fn adds_optional_arguments() {
        let options = Options {
            replace: true,
            labels: Some("self-hosted,linux".to_owned()),
            ..options()
        };

        assert_eq!(
            config_args(&options),
            vec![
                "--url",
                "https://github.com/acme/app",
                "--token",
                "secret",
                "--name",
                "actions-runner",
                "--unattended",
                "--replace",
                "--labels",
                "self-hosted,linux",
            ]
        );
    }

    #[test]
    fn builds_remove_arguments() {
        assert_eq!(remove_args("secret"), vec!["remove", "--token", "secret"]);
    }

    #[test]
    fn adds_ephemeral_flag() {
        let options = Options {
            ephemeral: true,
            ..options()
        };

        assert!(config_args(&options).iter().any(|arg| arg == "--ephemeral"));
    }

    fn script_name() -> &'static str {
        if cfg!(windows) {
            "config.cmd"
        } else {
            "config.sh"
        }
    }

    #[cfg(unix)]
    fn script_body(exit_code: i32) -> String {
        format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\nexit {exit_code}\n")
    }

    #[cfg(windows)]
    fn script_body(exit_code: i32) -> String {
        format!("@echo off\r\n(echo %*) > args.txt\r\nexit /b {exit_code}\r\n")
    }

    fn write_config_script(dir: &Path, exit_code: i32) {
        let path = dir.join(script_name());
        fs::write(&path, script_body(exit_code)).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).unwrap();
        }
    }

    #[test]
    fn runs_the_configuration_script() {
        let dir = tempfile::tempdir().unwrap();
        write_config_script(dir.path(), 0);

        configure(dir.path(), &options()).unwrap();

        let args = fs::read_to_string(dir.path().join("args.txt")).unwrap();
        assert!(args.contains("--url"));
        assert!(args.contains("https://github.com/acme/app"));
        assert!(args.contains("--token"));
        assert!(args.contains("secret"));
    }

    #[test]
    fn reports_a_failed_configuration_script() {
        let dir = tempfile::tempdir().unwrap();
        write_config_script(dir.path(), 1);

        assert!(configure(dir.path(), &options()).is_err());
    }

    #[test]
    fn runs_the_removal_script() {
        let dir = tempfile::tempdir().unwrap();
        write_config_script(dir.path(), 0);

        remove(dir.path(), "secret").unwrap();

        let args = fs::read_to_string(dir.path().join("args.txt")).unwrap();
        assert!(args.contains("remove"));
        assert!(args.contains("--token"));
    }

    #[test]
    fn reports_a_failed_removal_script() {
        let dir = tempfile::tempdir().unwrap();
        write_config_script(dir.path(), 1);

        assert!(remove(dir.path(), "secret").is_err());
    }
}
