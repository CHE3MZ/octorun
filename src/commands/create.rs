//! `octorun create` — create a new runner.

use std::fs;
use std::path::PathBuf;

use crate::cli::{Cli, CreateArgs};
use crate::error::{Error, Result};
use crate::github::{Client, Repository, Scope, TokenKind, resolve_token};
use crate::runner::config::{self, Options};
use crate::runner::install;

/// Create a runner from the given arguments.
///
/// # Errors
///
/// Returns an error if the repository, target, or token cannot be resolved, the
/// runner files cannot be installed, or the configuration script fails.
pub fn run(cli: &Cli, args: &CreateArgs) -> Result<()> {
    validate_name(&cli.name)?;

    let base_dir = cli.dir.clone().unwrap_or_else(|| PathBuf::from("."));
    fs::create_dir_all(&base_dir)?;
    let runner_dir = base_dir.join(&cli.name);

    if runner_dir.exists() {
        if !args.overwrite {
            return Err(Error::Config(format!(
                "runner directory `{}` already exists; use --overwrite to replace it",
                runner_dir.display()
            )));
        }
        fs::remove_dir_all(&runner_dir)?;
        tracing::info!("removed existing runner directory {}", runner_dir.display());
    }

    let repository = match &cli.repo {
        Some(reference) => Repository::parse(reference)?,
        None => Repository::current()?,
    };
    let host = repository.host();

    let scope = scope_for(&repository, args)?;
    let url = runner_url(host, &scope);

    let token = resolve_token(host)?;
    let api = Client::new(host, token)?;

    install::install(&api, args.location.as_deref(), &runner_dir)?;

    let registration_token = api.runner_token(&scope, TokenKind::Registration)?;
    let options = Options {
        url,
        token: registration_token,
        name: cli.name.clone(),
        replace: args.overwrite,
        labels: args.labels.clone(),
        group: args.group.clone(),
    };
    config::configure(&runner_dir, &options)?;

    tracing::info!("runner `{}` is ready at {}", cli.name, runner_dir.display());
    Ok(())
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(Error::Config(format!("invalid runner name `{name}`")));
    }
    Ok(())
}

fn scope_for(repository: &Repository, args: &CreateArgs) -> Result<Scope> {
    match (&args.org, &args.enterprise) {
        (Some(_), Some(_)) => Err(Error::Config(
            "--org and --enterprise cannot be used together".to_owned(),
        )),
        (Some(organization), None) => Ok(Scope::Organization(organization.clone())),
        (None, Some(enterprise)) => Ok(Scope::Enterprise(enterprise.clone())),
        (None, None) => Ok(Scope::Repository {
            owner: repository.owner().to_owned(),
            name: repository.name().to_owned(),
        }),
    }
}

fn runner_url(host: &str, scope: &Scope) -> String {
    match scope {
        Scope::Repository { owner, name } => format!("https://{host}/{owner}/{name}"),
        Scope::Organization(organization) => format!("https://{host}/{organization}"),
        Scope::Enterprise(enterprise) => format!("https://{host}/enterprises/{enterprise}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{runner_url, scope_for, validate_name};
    use crate::cli::CreateArgs;
    use crate::error::Error;
    use crate::github::{Repository, Scope};

    fn args() -> CreateArgs {
        CreateArgs {
            overwrite: false,
            location: None,
            group: None,
            labels: None,
            enterprise: None,
            org: None,
        }
    }

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
    fn defaults_to_repository_scope() {
        let repository = Repository::parse("acme/app").unwrap();
        let scope = scope_for(&repository, &args()).unwrap();

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
        let args = CreateArgs {
            org: Some("acme-org".to_owned()),
            ..args()
        };

        let scope = scope_for(&repository, &args).unwrap();
        assert_eq!(scope, Scope::Organization("acme-org".to_owned()));
        assert_eq!(
            runner_url("github.com", &scope),
            "https://github.com/acme-org"
        );
    }

    #[test]
    fn enterprise_overrides_repository() {
        let repository = Repository::parse("acme/app").unwrap();
        let args = CreateArgs {
            enterprise: Some("acme-inc".to_owned()),
            ..args()
        };

        let scope = scope_for(&repository, &args).unwrap();
        assert_eq!(scope, Scope::Enterprise("acme-inc".to_owned()));
        assert_eq!(
            runner_url("github.example.com", &scope),
            "https://github.example.com/enterprises/acme-inc"
        );
    }

    #[test]
    fn rejects_org_with_enterprise() {
        let repository = Repository::parse("acme/app").unwrap();
        let args = CreateArgs {
            org: Some("acme-org".to_owned()),
            enterprise: Some("acme-inc".to_owned()),
            ..args()
        };

        assert!(matches!(
            scope_for(&repository, &args),
            Err(Error::Config(_))
        ));
    }
}
