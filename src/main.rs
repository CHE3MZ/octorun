use std::process::ExitCode;

use clap::builder::styling::{AnsiColor, RgbColor, Styles};
use clap::{CommandFactory, FromArgMatches};
use octorun::cli::Cli;
use octorun::commands;

const ABOUT_TEXT: &str = "Create self hosted GitHub Actions runners";
const ACCENT_BLUE: RgbColor = RgbColor(41, 184, 219);
const HEADING_GREEN: RgbColor = RgbColor(35, 209, 139);

fn main() -> ExitCode {
    let matches = Cli::command()
        .about(styled_about())
        .styles(styles())
        .get_matches();
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
        .header(HEADING_GREEN.on_default().bold())
        .usage(HEADING_GREEN.on_default().bold())
        .literal(ACCENT_BLUE.on_default().bold())
        .placeholder(ACCENT_BLUE.on_default())
}

fn styled_about() -> String {
    let style = AnsiColor::White.on_default().bold();
    let start = style.render();
    let end = style.render_reset();
    format!("{start}{ABOUT_TEXT}{end}")
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
    use super::{ABOUT_TEXT, ACCENT_BLUE, HEADING_GREEN, styled_about, styles};
    use clap::builder::styling::{AnsiColor, Color, Effects};

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
            Some(Color::Rgb(HEADING_GREEN))
        );
        assert_eq!(
            (*styles.get_usage()).get_fg_color(),
            Some(Color::Rgb(HEADING_GREEN))
        );
    }

    #[test]
    fn about_is_bold_white() {
        let style = AnsiColor::White.on_default().bold();
        let about = styled_about();

        assert!(about.contains(ABOUT_TEXT));
        assert!(about.starts_with(&style.render().to_string()));
        assert!(about.ends_with(&style.render_reset().to_string()));
        assert_eq!(style.get_fg_color(), Some(Color::Ansi(AnsiColor::White)));
        assert!(style.get_effects().contains(Effects::BOLD));
    }
}
