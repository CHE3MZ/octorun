//! Error and result types shared across the `octorun` crate.

/// Errors produced by octorun.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// An underlying filesystem or I/O operation failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// An HTTP request failed.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// A JSON payload could not be decoded or encoded.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A ZIP archive could not be read.
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// The current operating system or architecture is not supported.
    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(String),

    /// No GitHub token could be discovered.
    #[error("no GitHub token found: set GH_TOKEN or GITHUB_TOKEN, or run `gh auth login`")]
    MissingToken,

    /// The GitHub API returned an error response.
    #[error("github api error: {0}")]
    Api(String),

    /// The supplied configuration is invalid.
    #[error("configuration error: {0}")]
    Config(String),

    /// A command that is planned but not implemented yet.
    #[error("`{0}` is not implemented yet")]
    Unimplemented(String),
}

/// Convenience alias for results carrying an [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;
