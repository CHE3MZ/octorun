//! Resolving a GitHub token for API calls.

use std::env;
use std::process::Command;

use crate::error::{Error, Result};

const TOKEN_ENV_VARS: [&str; 2] = ["GH_TOKEN", "GITHUB_TOKEN"];

/// Find a GitHub token for `host`.
///
/// `GH_TOKEN` and `GITHUB_TOKEN` are checked first, then the GitHub CLI is
/// asked with `gh auth token`.
///
/// # Errors
///
/// Returns [`Error::MissingToken`] when no token can be found.
pub fn resolve_token(host: &str) -> Result<String> {
    if let Some(token) = token_from_env() {
        return Ok(token);
    }
    if let Some(token) = token_from_gh(host) {
        return Ok(token);
    }
    Err(Error::MissingToken)
}

fn token_from_env() -> Option<String> {
    first_non_empty(TOKEN_ENV_VARS.iter().map(|name| env::var(name).ok()))
}

fn token_from_gh(host: &str) -> Option<String> {
    let mut command = Command::new("gh");
    command.args(["auth", "token"]);
    if host != "github.com" {
        command.args(["--hostname", host]);
    }

    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }

    let token = String::from_utf8(output.stdout).ok()?;
    first_non_empty(std::iter::once(Some(token)))
}

fn first_non_empty(values: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_owned())
        .find(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::first_non_empty;

    #[test]
    fn skips_missing_and_blank_values() {
        let values = [
            None,
            Some(String::new()),
            Some("  ".to_owned()),
            Some("abc".to_owned()),
        ];
        assert_eq!(first_non_empty(values), Some("abc".to_owned()));
    }

    #[test]
    fn returns_none_when_nothing_is_set() {
        assert_eq!(first_non_empty([None, Some(" ".to_owned())]), None);
    }
}
