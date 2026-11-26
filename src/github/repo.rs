//! Parsing and representation of GitHub repositories.

use std::fmt;
use std::process::Command;

use serde::Deserialize;

use crate::error::{Error, Result};

/// A GitHub repository identified by host, owner and name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    host: String,
    owner: String,
    name: String,
}

impl Repository {
    /// Parse a repository reference.
    ///
    /// Accepts either `OWNER/REPO` (the host defaults to `github.com`) or
    /// `HOST/OWNER/REPO` for GitHub Enterprise hosts.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when the reference is empty or malformed.
    pub fn parse(reference: &str) -> Result<Self> {
        let parts: Vec<&str> = reference.trim().trim_matches('/').split('/').collect();
        let (host, owner, name) = match parts.as_slice() {
            [owner, name] => ("github.com", *owner, *name),
            [host, owner, name] => (*host, *owner, *name),
            _ => return Err(Self::invalid(reference)),
        };

        if host.is_empty() || owner.is_empty() || name.is_empty() {
            return Err(Self::invalid(reference));
        }

        Ok(Self {
            host: host.to_owned(),
            owner: owner.to_owned(),
            name: name.to_owned(),
        })
    }

    fn invalid(reference: &str) -> Error {
        Error::Config(format!(
            "invalid repository `{reference}`, expected OWNER/REPO"
        ))
    }

    /// The GitHub host, e.g. `github.com`.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The repository owner (a user or an organization).
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// The repository name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The HTTPS URL of the repository.
    #[must_use]
    pub fn url(&self) -> String {
        let host = &self.host;
        let owner = &self.owner;
        let name = &self.name;
        format!("https://{host}/{owner}/{name}")
    }

    /// Resolve the repository for the current directory.
    ///
    /// Tries the GitHub CLI first, then the `origin` git remote.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when no repository can be determined.
    pub fn current() -> Result<Self> {
        if let Some(repository) = Self::from_gh() {
            return Ok(repository);
        }
        if let Some(repository) = Self::from_git_remote() {
            return Ok(repository);
        }
        Err(Error::Config(
            "could not determine the current repository; pass --repo OWNER/REPO".to_owned(),
        ))
    }

    /// Parse a git remote URL into a repository.
    ///
    /// Handles `https://`, `ssh://git@`, and `git@host:owner/repo` forms.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] if the URL is not a supported git remote.
    pub fn from_remote_url(url: &str) -> Result<Self> {
        let url = url.trim();
        let (host, path) = if let Some(rest) = url.strip_prefix("git@") {
            rest.split_once(':').ok_or_else(|| Self::invalid(url))?
        } else if let Some(rest) = url.strip_prefix("ssh://git@") {
            rest.split_once('/').ok_or_else(|| Self::invalid(url))?
        } else if let Some(rest) = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
        {
            rest.split_once('/').ok_or_else(|| Self::invalid(url))?
        } else {
            return Err(Self::invalid(url));
        };

        let path = path.trim_end_matches('/');
        let path = path.strip_suffix(".git").unwrap_or(path);
        Self::parse(&format!("{host}/{path}"))
    }

    fn from_gh() -> Option<Self> {
        #[derive(Deserialize)]
        struct RepoView {
            #[serde(rename = "nameWithOwner")]
            name_with_owner: String,
            url: String,
        }

        let output = Command::new("gh")
            .args(["repo", "view", "--json", "nameWithOwner,url"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }

        let view: RepoView = serde_json::from_slice(&output.stdout).ok()?;
        let host = view.url.split("://").nth(1)?.split('/').next()?;
        let (owner, name) = view.name_with_owner.split_once('/')?;
        Self::parse(&format!("{host}/{owner}/{name}")).ok()
    }

    fn from_git_remote() -> Option<Self> {
        let output = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }

        let url = String::from_utf8(output.stdout).ok()?;
        Self::from_remote_url(url.trim()).ok()
    }
}

impl fmt::Display for Repository {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::Repository;

    #[test]
    fn parses_owner_and_repo() {
        let repo = Repository::parse("acme/app").unwrap();
        assert_eq!(repo.host(), "github.com");
        assert_eq!(repo.owner(), "acme");
        assert_eq!(repo.name(), "app");
        assert_eq!(repo.to_string(), "acme/app");
        assert_eq!(repo.url(), "https://github.com/acme/app");
    }

    #[test]
    fn parses_explicit_host() {
        let repo = Repository::parse("github.example.com/acme/app").unwrap();
        assert_eq!(repo.host(), "github.example.com");
        assert_eq!(repo.url(), "https://github.example.com/acme/app");
    }

    #[test]
    fn trims_whitespace_and_slashes() {
        let repo = Repository::parse("  acme/app/  ").unwrap();
        assert_eq!(repo.owner(), "acme");
        assert_eq!(repo.name(), "app");
    }

    #[test]
    fn rejects_malformed_references() {
        for reference in ["", "acme", "acme/", "/app", "a/b/c/d"] {
            assert!(
                Repository::parse(reference).is_err(),
                "expected `{reference}` to be rejected"
            );
        }
    }

    #[test]
    fn parses_https_remote() {
        let repo = Repository::from_remote_url("https://github.com/acme/app.git").unwrap();
        assert_eq!(repo.host(), "github.com");
        assert_eq!(repo.to_string(), "acme/app");
    }

    #[test]
    fn parses_ssh_remote() {
        let repo = Repository::from_remote_url("git@github.com:acme/app.git").unwrap();
        assert_eq!(repo.host(), "github.com");
        assert_eq!(repo.to_string(), "acme/app");
    }

    #[test]
    fn parses_ssh_scheme_remote() {
        let repo = Repository::from_remote_url("ssh://git@github.example.com/acme/app").unwrap();
        assert_eq!(repo.host(), "github.example.com");
        assert_eq!(repo.to_string(), "acme/app");
    }

    #[test]
    fn rejects_unknown_remote_forms() {
        assert!(Repository::from_remote_url("file:///tmp/app").is_err());
    }
}
