//! Subcommand handlers and dispatch.

pub mod create;
pub mod remove;
pub mod restart;
pub mod start;
pub mod stop;
pub mod temp;

mod cleanup;

use std::path::{Path, PathBuf};

use crate::cli::{Cli, Command};
use crate::error::{Error, Result};
use crate::github::{Client, Repository, Scope, TokenKind, resolve_tokens_with};
use crate::runner::config;

/// Dispatch a parsed [`Cli`] to the matching subcommand handler.
///
/// # Errors
///
/// Returns an error if the selected subcommand fails.
pub fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Create(args) => create::run(cli, args),
        Command::Remove(args) => remove::run(cli, args),
        Command::Start => start::run(cli),
        Command::Stop => stop::run(cli),
        Command::Restart => restart::run(cli),
        Command::Temp(args) => temp::run(cli, args),
    }
}

pub(crate) fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(Error::Config(format!("invalid runner name `{name}`")));
    }
    Ok(())
}

#[must_use]
pub(crate) fn runner_dir(cli: &Cli) -> PathBuf {
    cli.dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(&cli.name)
}

pub(crate) fn scope_for(
    repository: &Repository,
    org: Option<&str>,
    enterprise: Option<&str>,
) -> Result<Scope> {
    match (org, enterprise) {
        (Some(_), Some(_)) => Err(Error::Config(
            "--org and --enterprise cannot be used together".to_owned(),
        )),
        (Some(organization), None) => Ok(Scope::Organization(organization.to_owned())),
        (None, Some(enterprise)) => Ok(Scope::Enterprise(enterprise.to_owned())),
        (None, None) => Ok(Scope::Repository {
            owner: repository.owner().to_owned(),
            name: repository.name().to_owned(),
        }),
    }
}

#[must_use]
pub(crate) fn runner_url(host: &str, scope: &Scope) -> String {
    match scope {
        Scope::Repository { owner, name } => format!("https://{host}/{owner}/{name}"),
        Scope::Organization(organization) => format!("https://{host}/{organization}"),
        Scope::Enterprise(enterprise) => format!("https://{host}/enterprises/{enterprise}"),
    }
}

/// Deregister the runner in `dir` from GitHub.
///
/// # Errors
///
/// Returns an error if the token cannot be resolved or the configuration
/// script fails.
pub(crate) fn deregister(dir: &Path, host: &str, scope: &Scope, token: Option<&str>) -> Result<()> {
    let tokens = resolve_tokens_with(token, host)?;
    let api = Client::with_tokens(host, tokens)?;
    deregister_with(&api, dir, scope)
}

fn deregister_with(api: &Client, dir: &Path, scope: &Scope) -> Result<()> {
    let removal_token = api.runner_token(scope, TokenKind::Remove)?;
    config::remove(dir, &removal_token)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;

    use clap::Parser;

    use super::{deregister_with, runner_dir, runner_url, scope_for, validate_name};
    use crate::cli::Cli;
    use crate::error::Error;
    use crate::github::{Client, Repository, Scope};

    #[test]
    fn rejects_unsafe_runner_names() {
        for name in ["", ".", "..", "a/b", "a\\b"] {
            assert!(
                validate_name(name).is_err(),
                "expected `{name}` to be rejected"
            );
        }
        assert!(validate_name("actions-runner").is_ok());
    }

    #[test]
    fn builds_runner_dir_from_cli() {
        let cli = Cli::try_parse_from(["octorun", "-d", "work", "-n", "fast", "start"]).unwrap();
        assert_eq!(runner_dir(&cli), PathBuf::from("work").join("fast"));
    }

    #[test]
    fn defaults_to_repository_scope() {
        let repository = Repository::parse("acme/app").unwrap();
        let scope = scope_for(&repository, None, None).unwrap();

        assert_eq!(
            scope,
            Scope::Repository {
                owner: "acme".to_owned(),
                name: "app".to_owned(),
            }
        );
        assert_eq!(
            runner_url("github.com", &scope),
            "https://github.com/acme/app"
        );
    }

    #[test]
    fn organization_overrides_repository() {
        let repository = Repository::parse("acme/app").unwrap();
        let scope = scope_for(&repository, Some("acme-org"), None).unwrap();

        assert_eq!(scope, Scope::Organization("acme-org".to_owned()));
        assert_eq!(
            runner_url("github.com", &scope),
            "https://github.com/acme-org"
        );
    }

    #[test]
    fn enterprise_overrides_repository() {
        let repository = Repository::parse("acme/app").unwrap();
        let scope = scope_for(&repository, None, Some("acme-inc")).unwrap();

        assert_eq!(scope, Scope::Enterprise("acme-inc".to_owned()));
        assert_eq!(
            runner_url("github.example.com", &scope),
            "https://github.example.com/enterprises/acme-inc"
        );
    }

    #[test]
    fn rejects_org_with_enterprise() {
        let repository = Repository::parse("acme/app").unwrap();

        assert!(matches!(
            scope_for(&repository, Some("acme-org"), Some("acme-inc")),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn deregisters_a_runner_with_a_removal_token() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join(if cfg!(windows) {
            "config.cmd"
        } else {
            "config.sh"
        });

        #[cfg(unix)]
        fs::write(&script, "#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\n").unwrap();
        #[cfg(windows)]
        fs::write(&script, "@echo off\r\n(echo %*) > args.txt\r\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&script).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script, permissions).unwrap();
        }

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 512];
            let _ = stream.read(&mut buffer);
            let body = r#"{"token":"remove-token"}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            stream.flush().unwrap();
        });
        let client = Client::with_base_url(base, "token").unwrap();
        let scope = Scope::Organization("acme".to_owned());

        deregister_with(&client, dir.path(), &scope).unwrap();
        server.join().unwrap();

        let args = fs::read_to_string(dir.path().join("args.txt")).unwrap();
        assert!(args.contains("remove"));
        assert!(args.contains("--token"));
        assert!(args.contains("remove-token"));
    }
}
