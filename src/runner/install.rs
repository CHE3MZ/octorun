//! Installing the runner files into a destination directory.

use std::fs;
use std::path::Path;

use crate::error::{Error, Result};
use crate::github::Client;
use crate::runner::{archive_name, download, extract};

/// Install runner files into `dest`.
///
/// `location` may be an archive URL, a local archive, or a directory. When it
/// is `None`, the latest runner release is resolved through `api`.
///
/// # Errors
///
/// Returns an error if the release cannot be resolved, the download fails, the
/// archive cannot be extracted, or the source is missing.
pub fn install(api: &Client, location: Option<&str>, dest: &Path) -> Result<()> {
    match location {
        Some(location) if is_url(location) => install_from_url(location, dest),
        Some(location) => install_from_path(Path::new(location), dest),
        None => {
            let release = api.latest_runner_release()?;
            let archive = archive_name(release.version())?;
            let tag = &release.tag_name;
            let url =
                format!("https://github.com/actions/runner/releases/download/{tag}/{archive}");
            install_from_url(&url, dest)
        }
    }
}

fn install_from_path(source: &Path, dest: &Path) -> Result<()> {
    if source.is_dir() {
        copy_dir(source, dest)
    } else if source.is_file() {
        extract(source, dest)
    } else {
        Err(Error::Config(format!(
            "runner location not found: {}",
            source.display()
        )))
    }
}

fn install_from_url(url: &str, dest: &Path) -> Result<()> {
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let file_name = archive_file_name(url);
    let archive_path = dest.with_file_name(format!(".octorun-{file_name}"));

    download(url, &archive_path)?;
    let result = extract(&archive_path, dest);

    if let Err(error) = fs::remove_file(&archive_path) {
        tracing::warn!(
            "could not remove temporary archive {}: {error}",
            archive_path.display()
        );
    }

    result
}

fn copy_dir(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());

        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }

    Ok(())
}

fn is_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn archive_file_name(url: &str) -> &str {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("runner.tar.gz")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{archive_file_name, copy_dir, is_url};

    #[test]
    fn recognizes_urls() {
        assert!(is_url("https://example.com/runner.tar.gz"));
        assert!(is_url("http://example.com/runner.zip"));
        assert!(!is_url("./runner.tar.gz"));
        assert!(!is_url("runner.zip"));
    }

    #[test]
    fn extracts_file_name_from_url() {
        assert_eq!(
            archive_file_name("https://example.com/actions-runner-linux-x64-2.1.0.tar.gz"),
            "actions-runner-linux-x64-2.1.0.tar.gz"
        );
        assert_eq!(
            archive_file_name("https://example.com/runner.zip?token=abc"),
            "runner.zip"
        );
        assert_eq!(archive_file_name("https://example.com/"), "runner.tar.gz");
    }

    #[test]
    fn copies_directory_tree() {
        let source = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();

        fs::create_dir_all(source.path().join("bin")).unwrap();
        fs::write(source.path().join("config.sh"), b"#!/bin/sh").unwrap();
        fs::write(source.path().join("bin").join("Runner.Listener"), b"bin").unwrap();

        copy_dir(source.path(), dest.path()).unwrap();

        assert_eq!(
            fs::read(dest.path().join("config.sh")).unwrap(),
            b"#!/bin/sh"
        );
        assert_eq!(
            fs::read(dest.path().join("bin").join("Runner.Listener")).unwrap(),
            b"bin"
        );
    }
}
