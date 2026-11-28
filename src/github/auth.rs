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
    let env_tokens = TOKEN_ENV_VARS
        .iter()
        .filter_map(|name| env::var(name).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());

    collect_tokens(env_tokens.chain(token_from_gh(host)))
}

fn collect_tokens(tokens: impl IntoIterator<Item = String>) -> Result<Vec<String>> {
    let mut unique = Vec::new();

    for token in tokens {
        push_unique(&mut unique, token);
    }

    if unique.is_empty() {
        Err(Error::MissingToken)
    } else {
        Ok(unique)
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
    let output = gh_command(host).output().ok()?;
    token_from_output(output.status.success(), &output.stdout)
}

fn gh_command(host: &str) -> Command {
    let mut command = Command::new("gh");
    command.args(["auth", "token"]);
    if host != "github.com" {
        command.args(["--hostname", host]);
    }
    command
}

fn token_from_output(success: bool, stdout: &[u8]) -> Option<String> {
    if !success {
        return None;
    }

    let token = String::from_utf8(stdout.to_vec()).ok()?;
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
    use std::ffi::OsStr;

    use super::{collect_tokens, gh_command, push_unique, resolve_tokens_with, token_from_output};
    use crate::error::Error;

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

    #[test]
    fn parses_gh_tokens() {
        assert_eq!(token_from_output(true, b" tok \n").as_deref(), Some("tok"));
        assert_eq!(token_from_output(false, b"tok"), None);
        assert_eq!(token_from_output(true, b"   "), None);
        assert_eq!(token_from_output(true, &[0xff]), None);
    }

    #[test]
    fn collects_and_deduplicates_tokens() {
        assert!(matches!(
            collect_tokens(Vec::<String>::new()),
            Err(Error::MissingToken)
        ));
        assert_eq!(
            collect_tokens(["a".to_owned(), "a".to_owned(), "b".to_owned()]).unwrap(),
            ["a", "b"]
        );
    }

    #[test]
    fn builds_the_gh_command_for_the_host() {
        let github = gh_command("github.com");
        let args: Vec<_> = github.get_args().collect();
        assert!(args.contains(&OsStr::new("auth")));
        assert!(!args.contains(&OsStr::new("--hostname")));

        let enterprise = gh_command("github.example.com");
        let args: Vec<_> = enterprise.get_args().collect();
        assert!(args.contains(&OsStr::new("--hostname")));
        assert!(args.contains(&OsStr::new("github.example.com")));
    }
}
