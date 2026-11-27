//! Resolving GitHub tokens for API calls.

use std::env;
use std::process::Command;

use crate::error::{Error, Result};

const TOKEN_ENV_VARS: [&str; 2] = ["GH_TOKEN", "GITHUB_TOKEN"];

/// Find GitHub tokens for `host`, in the order they should be tried.
///
/// `GH_TOKEN` and `GITHUB_TOKEN` come first, then the token from
/// `gh auth token`. Duplicate tokens are removed.
///
/// # Errors
///
/// Returns [`Error::MissingToken`] when no token can be found.
pub fn resolve_tokens(host: &str) -> Result<Vec<String>> {
    let mut tokens = Vec::new();

    for name in TOKEN_ENV_VARS {
        if let Ok(value) = env::var(name) {
            let value = value.trim();
            if !value.is_empty() {
                push_unique(&mut tokens, value.to_owned());
            }
        }
    }

    if let Some(token) = token_from_gh(host) {
        push_unique(&mut tokens, token);
    }

    if tokens.is_empty() {
        Err(Error::MissingToken)
    } else {
        Ok(tokens)
    }
}

/// Find GitHub tokens, preferring an explicitly provided token.
///
/// When `explicit` is missing or blank, the usual `GH_TOKEN`, `GITHUB_TOKEN`,
/// and `gh auth token` lookup is used.
///
/// # Errors
///
/// Returns [`Error::MissingToken`] when no token can be found.
pub fn resolve_tokens_with(explicit: Option<&str>, host: &str) -> Result<Vec<String>> {
    if let Some(token) = explicit {
        let token = token.trim();
        if !token.is_empty() {
            return Ok(vec![token.to_owned()]);
        }
    }
    resolve_tokens(host)
}

/// Find the first GitHub token for `host`.
///
/// # Errors
///
/// Returns [`Error::MissingToken`] when no token can be found.
pub fn resolve_token(host: &str) -> Result<String> {
    resolve_tokens(host)?
        .into_iter()
        .next()
        .ok_or(Error::MissingToken)
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
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_owned())
    }
}

fn push_unique(tokens: &mut Vec<String>, token: String) {
    if !tokens.iter().any(|existing| existing == &token) {
        tokens.push(token);
    }
}

#[cfg(test)]
mod tests {
    use super::{push_unique, resolve_tokens_with};

    #[test]
    fn deduplicates_tokens_in_order() {
        let mut tokens = vec!["first".to_owned()];

        push_unique(&mut tokens, "first".to_owned());
        push_unique(&mut tokens, "second".to_owned());
        push_unique(&mut tokens, "second".to_owned());

        assert_eq!(tokens, ["first", "second"]);
    }

    #[test]
    fn explicit_token_takes_precedence() {
        assert_eq!(
            resolve_tokens_with(Some("  secret  "), "github.com").unwrap(),
            ["secret"]
        );
    }
}
