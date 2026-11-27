use std::process::ExitCode;

use clap::builder::styling::{AnsiColor, RgbColor, Styles};
use clap::{CommandFactory, FromArgMatches};
use octorun::cli::Cli;
use octorun::commands;

const ACCENT_BLUE: RgbColor = RgbColor(41, 184, 219);

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
        .literal(ACCENT_BLUE.on_default().bold())
        .placeholder(ACCENT_BLUE.on_default())
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

#[cfg(test)]
mod tests {
    use super::{ACCENT_BLUE, styles};
    use clap::builder::styling::{AnsiColor, Color};

    #[test]
    fn accent_and_heading_colors_are_set() {
        let styles = styles();

        assert_eq!(
            (*styles.get_literal()).get_fg_color(),
            Some(Color::Rgb(ACCENT_BLUE))
        );
        assert_eq!(
            (*styles.get_placeholder()).get_fg_color(),
            Some(Color::Rgb(ACCENT_BLUE))
        );
        assert_eq!(
            (*styles.get_header()).get_fg_color(),
            Some(Color::Ansi(AnsiColor::White))
        );
    }
}
