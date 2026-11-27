//! Repository resolution and GitHub API access.

pub mod api;
pub mod auth;
pub mod repo;

pub use api::{Client, Release, ReleaseAsset, Scope, TokenKind};
pub use auth::{resolve_token, resolve_tokens};
pub use repo::Repository;
