//! Detection and selection of the runner platform.

use clap::ValueEnum;

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
    Ok(RunnerPlatform::detect()?.archive_name(version))
}

/// A supported runner platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RunnerPlatform {
    /// Windows on x86-64.
    #[value(name = "windows-amd64")]
    WindowsAmd64,
    /// Windows on ARM64.
    #[value(name = "windows-arm64")]
    WindowsArm64,
    /// macOS on x86-64.
    #[value(name = "macos-amd64")]
    MacosAmd64,
    /// macOS on ARM64.
    #[value(name = "macos-arm64")]
    MacosArm64,
    /// Linux on x86-64.
    #[value(name = "linux-amd64")]
    LinuxAmd64,
    /// Linux on ARM64.
    #[value(name = "linux-arm64")]
    LinuxArm64,
}

impl RunnerPlatform {
    /// Detect the platform of the current machine.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UnsupportedPlatform`] when the host is not one of the
    /// supported runner platforms.
    pub fn detect() -> Result<Self> {
        match (os()?, arch()?) {
            ("win", "x64") => Ok(Self::WindowsAmd64),
            ("win", "arm64") => Ok(Self::WindowsArm64),
            ("osx", "x64") => Ok(Self::MacosAmd64),
            ("osx", "arm64") => Ok(Self::MacosArm64),
            ("linux", "x64") => Ok(Self::LinuxAmd64),
            ("linux", "arm64") => Ok(Self::LinuxArm64),
            (os, arch) => Err(Error::UnsupportedPlatform(format!("{os}-{arch}"))),
        }
    }

    /// The operating system component, e.g. `linux`, `osx`, `win`.
    #[must_use]
    pub fn os(self) -> &'static str {
        match self {
            Self::WindowsAmd64 | Self::WindowsArm64 => "win",
            Self::MacosAmd64 | Self::MacosArm64 => "osx",
            Self::LinuxAmd64 | Self::LinuxArm64 => "linux",
        }
    }

    /// The architecture component, e.g. `x64` or `arm64`.
    #[must_use]
    pub fn arch(self) -> &'static str {
        match self {
            Self::WindowsAmd64 | Self::MacosAmd64 | Self::LinuxAmd64 => "x64",
            Self::WindowsArm64 | Self::MacosArm64 | Self::LinuxArm64 => "arm64",
        }
    }

    /// The archive extension for this platform.
    #[must_use]
    pub fn archive_extension(self) -> &'static str {
        match self {
            Self::WindowsAmd64 | Self::WindowsArm64 => "zip",
            Self::MacosAmd64 | Self::MacosArm64 | Self::LinuxAmd64 | Self::LinuxArm64 => "tar.gz",
        }
    }

    /// Build the runner archive name for `version`.
    #[must_use]
    pub fn archive_name(self, version: &str) -> String {
        let version = version.trim_start_matches('v');
        let os = self.os();
        let arch = self.arch();
        format!(
            "actions-runner-{os}-{arch}-{version}.{}",
            self.archive_extension()
        )
    }
}

fn map_os(os: &str) -> Option<&'static str> {
    match os {
        "linux" => Some("linux"),
        "macos" | "osx" | "darwin" | "mac" => Some("osx"),
        "windows" | "win" => Some("win"),
        _ => None,
    }
}

fn map_arch(arch: &str) -> Option<&'static str> {
    match arch {
        "x86_64" | "amd64" | "x64" => Some("x64"),
        "aarch64" | "arm64" => Some("arm64"),
        "arm" | "armv7" | "armv7l" => Some("arm"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{RunnerPlatform, archive_extension, archive_name, map_arch, map_os};

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
        assert_eq!(map_arch("amd64"), Some("x64"));
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

    #[test]
    fn builds_platform_archive_names() {
        assert_eq!(
            RunnerPlatform::LinuxAmd64.archive_name("v2.300.0"),
            "actions-runner-linux-x64-2.300.0.tar.gz"
        );
        assert_eq!(
            RunnerPlatform::WindowsArm64.archive_name("2.300.0"),
            "actions-runner-win-arm64-2.300.0.zip"
        );
        assert_eq!(
            RunnerPlatform::MacosAmd64.archive_name("2.300.0"),
            "actions-runner-osx-x64-2.300.0.tar.gz"
        );
    }

    #[test]
    fn reports_platform_components() {
        assert_eq!(RunnerPlatform::WindowsAmd64.os(), "win");
        assert_eq!(RunnerPlatform::WindowsAmd64.arch(), "x64");
        assert_eq!(RunnerPlatform::MacosArm64.os(), "osx");
        assert_eq!(RunnerPlatform::MacosArm64.arch(), "arm64");
    }
}
