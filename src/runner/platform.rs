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
    os_for(std::env::consts::OS)
}

/// The architecture component used in runner archive names.
///
/// # Errors
///
/// Returns [`Error::UnsupportedPlatform`] when the host architecture has no
/// matching GitHub runner release.
pub fn arch() -> Result<&'static str> {
    arch_for(std::env::consts::ARCH)
}

fn os_for(value: &str) -> Result<&'static str> {
    map_os(value).ok_or_else(|| Error::UnsupportedPlatform(format!("operating system `{value}`")))
}

fn arch_for(value: &str) -> Result<&'static str> {
    map_arch(value).ok_or_else(|| Error::UnsupportedPlatform(format!("architecture `{value}`")))
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
        Self::detect_for(std::env::consts::OS, std::env::consts::ARCH)
    }

    fn detect_for(os: &str, arch: &str) -> Result<Self> {
        match (os_for(os)?, arch_for(arch)?) {
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
    use super::{
        RunnerPlatform, arch, arch_for, archive_extension, archive_name, map_arch, map_os, os,
        os_for,
    };
    use crate::error::Error;

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
    fn maps_the_remaining_names() {
        assert_eq!(map_arch("x64"), Some("x64"));
        assert_eq!(map_arch("arm64"), Some("arm64"));
        assert_eq!(map_arch("armv7"), Some("arm"));
        assert_eq!(map_os("osx"), Some("osx"));
        assert_eq!(map_os("darwin"), Some("osx"));
        assert_eq!(map_os("mac"), Some("osx"));
        assert_eq!(map_os("win"), Some("win"));
    }

    #[test]
    fn rejects_unsupported_host_components() {
        assert!(matches!(
            os_for("freebsd"),
            Err(Error::UnsupportedPlatform(_))
        ));
        assert!(matches!(
            arch_for("riscv64"),
            Err(Error::UnsupportedPlatform(_))
        ));
    }

    #[test]
    fn detects_every_supported_pair() {
        assert_eq!(
            RunnerPlatform::detect_for("windows", "x86_64").unwrap(),
            RunnerPlatform::WindowsAmd64
        );
        assert_eq!(
            RunnerPlatform::detect_for("windows", "aarch64").unwrap(),
            RunnerPlatform::WindowsArm64
        );
        assert_eq!(
            RunnerPlatform::detect_for("macos", "x86_64").unwrap(),
            RunnerPlatform::MacosAmd64
        );
        assert_eq!(
            RunnerPlatform::detect_for("macos", "arm64").unwrap(),
            RunnerPlatform::MacosArm64
        );
        assert_eq!(
            RunnerPlatform::detect_for("linux", "x64").unwrap(),
            RunnerPlatform::LinuxAmd64
        );
        assert_eq!(
            RunnerPlatform::detect_for("linux", "aarch64").unwrap(),
            RunnerPlatform::LinuxArm64
        );
        assert!(matches!(
            RunnerPlatform::detect_for("linux", "armv7"),
            Err(Error::UnsupportedPlatform(_))
        ));
    }

    #[test]
    fn host_detection_matches_the_mapped_constants() {
        assert_eq!(os().unwrap(), map_os(std::env::consts::OS).unwrap());
        assert_eq!(arch().unwrap(), map_arch(std::env::consts::ARCH).unwrap());
        assert_eq!(
            RunnerPlatform::detect().unwrap(),
            RunnerPlatform::detect_for(std::env::consts::OS, std::env::consts::ARCH).unwrap()
        );
        assert_eq!(
            archive_extension(),
            if cfg!(windows) { "zip" } else { "tar.gz" }
        );
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
