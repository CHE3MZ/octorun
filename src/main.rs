use std::process::ExitCode;

use clap::builder::styling::{AnsiColor, Styles};
use clap::{CommandFactory, FromArgMatches};
use octorun::cli::Cli;
use octorun::commands;

fn main() -> ExitCode {
    let matches = Cli::command().styles(styles()).get_matches();
    let cli = match Cli::from_arg_matches(&matches) {
        Ok(cli) => cli,
        Err(error) => error.exit(),
    };

    init_tracing(cli.verbose);

    match commands::run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::White.on_default().bold())
        .usage(AnsiColor::White.on_default().bold())
}

fn init_tracing(verbose: bool) {
    let level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    let _ = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .try_init();
}
