//! Command-line interface definitions for the `octorun` binary.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::runner::RunnerPlatform;

/// Create self hosted GitHub Actions runners.
#[derive(Debug, Parser)]
#[command(name = "octorun", version, about, long_about = None)]
pub struct Cli {
    /// Show verbose logging text.
    #[arg(short, long, global = true, display_order = 100)]
    pub verbose: bool,

    /// Custom name for the runner (defaults to "actions-runner").
    #[arg(
        short,
        long,
        global = true,
        value_name = "NAME",
        default_value = "actions-runner",
        hide_default_value = true,
        display_order = 110
    )]
    pub name: String,

    /// Change the directory to run in (defaults to the current directory).
    #[arg(short, long, global = true, value_name = "DIR", display_order = 120)]
    pub dir: Option<PathBuf>,

    /// Repository to use <OWNER/REPO> (defaults to the current repository).
    #[arg(
        short,
        long,
        global = true,
        value_name = "OWNER/REPO",
        display_order = 130
    )]
    pub repo: Option<String>,

    /// GitHub API token (defaults to the token found automatically).
    #[arg(short, long, global = true, value_name = "TOKEN", display_order = 140)]
    pub token: Option<String>,

    /// The subcommand to execute.
    #[command(subcommand)]
    pub command: Command,
}

/// The available octorun subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new runner.
    Create(CreateArgs),

    /// Delete a runner.
    Remove(RemoveArgs),

    /// Start a preconfigured runner.
    Start,

    /// Stop a preconfigured runner.
    Stop,

    /// Restart a runner started by octorun.
    Restart,

    /// Create a temporary runner and remove it when stopped.
    Temp(TempArgs),
}

/// Options accepted by `octorun create`.
#[derive(Debug, Clone, Args)]
pub struct CreateArgs {
    /// Overwrite any existing runner with the same name.
    #[arg(short, long, display_order = 10)]
    pub overwrite: bool,

    /// Set the runner location manually instead of downloading one.
    #[arg(short, long, value_name = "LOCATION", display_order = 20)]
    pub location: Option<String>,

    /// Set the runner platform (defaults to auto detected system architecture).
    #[arg(
        short,
        long,
        value_enum,
        value_name = "PLATFORM",
        hide_possible_values = true,
        display_order = 30,
        long_help = "Set the runner platform (defaults to auto detected system architecture).\n\nAvailable values:\n  - windows-amd64\n  - windows-arm64\n  - macos-amd64\n  - macos-arm64\n  - linux-amd64\n  - linux-arm64"
    )]
    pub platform: Option<RunnerPlatform>,

    /// Comma separated list of labels to add to the runner.
    #[arg(long, value_name = "LABELS", display_order = 40)]
    pub labels: Option<String>,

    /// Add the runner at the Enterprise level with the enterprise's name.
    #[arg(long, value_name = "ENTERPRISE", display_order = 50)]
    pub enterprise: Option<String>,

    /// Add the runner at the Organization level with the organization's name.
    #[arg(long, value_name = "ORG", display_order = 60)]
    pub org: Option<String>,

    /// Download the runner without registering or configuring it.
    #[arg(long, display_order = 70)]
    pub no_configuration: bool,
}

/// Options accepted by `octorun remove`.
#[derive(Debug, Clone, Args)]
pub struct RemoveArgs {
    /// Remove the runner at the Enterprise level with the enterprise's name.
    #[arg(long, value_name = "ENTERPRISE", display_order = 10)]
    pub enterprise: Option<String>,

    /// Remove the runner at the Organization level with the organization's name.
    #[arg(long, value_name = "ORG", display_order = 20)]
    pub org: Option<String>,
}

/// Options accepted by `octorun temp`.
#[derive(Debug, Clone, Args)]
pub struct TempArgs {
    /// Run a single job, then remove the runner.
    #[arg(long, display_order = 5)]
    pub ephemeral: bool,

    /// Options used to create the runner.
    #[command(flatten)]
    pub create: CreateArgs,
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use crate::runner::RunnerPlatform;
    use clap::Parser;

    #[test]
    fn global_defaults() {
        let cli = Cli::try_parse_from(["octorun", "start"]).unwrap();
        assert_eq!(cli.name, "actions-runner");
        assert!(cli.repo.is_none());
        assert!(cli.token.is_none());
        assert!(cli.dir.is_none());
        assert!(!cli.verbose);
    }

    #[test]
    fn global_options_are_accepted_before_subcommand() {
        let cli = Cli::try_parse_from([
            "octorun", "-v", "-n", "fast", "-r", "acme/app", "-t", "secret", "stop",
        ])
        .unwrap();
        assert!(cli.verbose);
        assert_eq!(cli.name, "fast");
        assert_eq!(cli.repo.as_deref(), Some("acme/app"));
        assert_eq!(cli.token.as_deref(), Some("secret"));
        assert!(matches!(cli.command, Command::Stop));
    }

    #[test]
    fn create_flags_parse() {
        let cli = Cli::try_parse_from([
            "octorun",
            "create",
            "-o",
            "-l",
            "https://example.com/runner.tar.gz",
            "-p",
            "linux-arm64",
            "--labels",
            "self-hosted,linux",
            "--org",
            "acme",
        ])
        .unwrap();
        let Command::Create(args) = cli.command else {
            panic!("expected create subcommand");
        };
        assert!(args.overwrite);
        assert_eq!(
            args.location.as_deref(),
            Some("https://example.com/runner.tar.gz")
        );
        assert_eq!(args.platform, Some(RunnerPlatform::LinuxArm64));
        assert_eq!(args.labels.as_deref(), Some("self-hosted,linux"));
        assert_eq!(args.org.as_deref(), Some("acme"));
        assert!(args.enterprise.is_none());
        assert!(!args.no_configuration);
    }

    #[test]
    fn create_no_configuration_flag_parses() {
        let cli = Cli::try_parse_from(["octorun", "create", "--no-configuration"]).unwrap();
        let Command::Create(args) = cli.command else {
            panic!("expected create subcommand");
        };
        assert!(args.no_configuration);
    }

    #[test]
    fn subcommand_is_required() {
        assert!(Cli::try_parse_from(["octorun"]).is_err());
    }

    #[test]
    fn restart_parses() {
        let cli = Cli::try_parse_from(["octorun", "restart"]).unwrap();
        assert!(matches!(cli.command, Command::Restart));
    }

    #[test]
    fn temp_flags_parse() {
        let cli = Cli::try_parse_from([
            "octorun",
            "temp",
            "--ephemeral",
            "-l",
            "https://example.com/runner.zip",
            "-p",
            "macos-arm64",
        ])
        .unwrap();

        let Command::Temp(args) = cli.command else {
            panic!("expected temp subcommand");
        };
        assert!(args.ephemeral);
        assert_eq!(args.create.platform, Some(RunnerPlatform::MacosArm64));
        assert_eq!(
            args.create.location.as_deref(),
            Some("https://example.com/runner.zip")
        );
    }

    #[test]
    fn rejects_unknown_platform() {
        assert!(Cli::try_parse_from(["octorun", "create", "-p", "plan9-amd64"]).is_err());
    }
}
