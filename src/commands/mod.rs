//! Subcommand handlers and dispatch.

pub mod create;
pub mod remove;
pub mod start;
pub mod stop;

use std::path::PathBuf;

use crate::cli::{Cli, Command};
use crate::error::{Error, Result};
use crate::github::{Repository, Scope};

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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::Parser;

    use super::{runner_dir, runner_url, scope_for, validate_name};
    use crate::cli::Cli;
    use crate::error::Error;
    use crate::github::{Repository, Scope};

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
}
