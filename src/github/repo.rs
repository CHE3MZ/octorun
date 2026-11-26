//! Parsing and representation of GitHub repositories.

use std::fmt;

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
        format!("https://{}/{}/{}", self.host, self.owner, self.name)
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
}
