//! Detection of the current platform and the corresponding runner archive.

use crate::error::{Error, Result};

/// The operating system component used in runner archive names.
///
/// # Errors
///
/// Returns [`Error::UnsupportedPlatform`] when the host operating system is not
/// supported by GitHub's runner releases.
pub fn os() -> Result<&'static str> {
    map_os(std::env::consts::OS).ok_or_else(|| {
        Error::UnsupportedPlatform(format!("operating system `{}`", std::env::consts::OS))
    })
}

/// The architecture component used in runner archive names.
///
/// # Errors
///
/// Returns [`Error::UnsupportedPlatform`] when the host architecture has no
/// matching GitHub runner release.
pub fn arch() -> Result<&'static str> {
    map_arch(std::env::consts::ARCH).ok_or_else(|| {
        Error::UnsupportedPlatform(format!("architecture `{}`", std::env::consts::ARCH))
    })
}

/// The archive extension used for the current operating system.
#[must_use]
pub fn archive_extension() -> &'static str {
    if cfg!(windows) { "zip" } else { "tar.gz" }
}

/// Build the GitHub Actions runner archive name for `version`.
///
/// The version may optionally carry a leading `v` (as reported by the GitHub
/// releases API), which is stripped.
///
/// # Errors
///
/// Returns an error if the current platform is unsupported.
pub fn archive_name(version: &str) -> Result<String> {
    let version = version.trim_start_matches('v');
    let os = os()?;
    let arch = arch()?;
    Ok(format!(
        "actions-runner-{os}-{arch}-{version}.{}",
        archive_extension()
    ))
}

fn map_os(os: &str) -> Option<&'static str> {
    match os {
        "linux" => Some("linux"),
        "macos" => Some("osx"),
        "windows" => Some("win"),
        _ => None,
    }
}

fn map_arch(arch: &str) -> Option<&'static str> {
    match arch {
        "x86_64" | "x86" => Some("x64"),
        "aarch64" => Some("arm64"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{archive_extension, archive_name, map_arch, map_os};

    #[test]
    fn maps_known_operating_systems() {
        assert_eq!(map_os("linux"), Some("linux"));
        assert_eq!(map_os("macos"), Some("osx"));
        assert_eq!(map_os("windows"), Some("win"));
        assert_eq!(map_os("freebsd"), None);
    }

    #[test]
    fn maps_known_architectures() {
        assert_eq!(map_arch("x86_64"), Some("x64"));
        assert_eq!(map_arch("aarch64"), Some("arm64"));
        assert_eq!(map_arch("riscv64"), None);
    }

    #[test]
    fn builds_archive_name_without_version_prefix() {
        let name = archive_name("v2.300.0").unwrap();
        assert!(name.starts_with("actions-runner-"));
        assert!(name.contains("-2.300.0."));
        assert!(name.ends_with(archive_extension()));
    }
}
