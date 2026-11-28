//! Terminal styling and progress output.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use console::{Term, style};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use tracing::Level;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

/// Return the next log sequence number.
#[must_use]
pub(crate) fn next_id() -> usize {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Format a coloured `LEVEL[0001]` label for stderr.
#[must_use]
pub(crate) fn styled_label(level: Level, id: usize) -> String {
    let name = match level {
        Level::ERROR => "ERROR",
        Level::WARN => "WARN",
        Level::INFO => "INFO",
        Level::DEBUG => "DEBUG",
        Level::TRACE => "TRACE",
    };

    let painted = match level {
        Level::ERROR => style(name).red(),
        Level::WARN => style(name).yellow(),
        Level::INFO => style(name).green(),
        Level::DEBUG => style(name).blue(),
        Level::TRACE => style(name).magenta(),
    };

    let painted = painted.bold().for_stderr().to_string();
    let id = style(format!("[{id:04}]"))
        .white()
        .bold()
        .for_stderr()
        .to_string();

    format!("{painted}{id}")
}

/// Underline an artifact name for stderr.
#[must_use]
pub(crate) fn underlined(name: &str) -> String {
    style(name).underlined().for_stderr().to_string()
}

/// Create a download progress bar when stderr is a terminal.
#[must_use]
pub(crate) fn download_bar(total: u64, message: &str) -> Option<ProgressBar> {
    Term::stderr()
        .is_term()
        .then(|| build_download_bar(ProgressDrawTarget::stderr(), total, message))
}

fn build_download_bar(target: ProgressDrawTarget, total: u64, message: &str) -> ProgressBar {
    let style = ProgressStyle::with_template(
        "{prefix} {spinner} {msg}  {bar:10} {percent}% {bytes_per_sec}",
    )
    .expect("valid download progress template")
    .tick_strings(SPINNER_FRAMES)
    .progress_chars("█░");

    let bar = ProgressBar::with_draw_target(Some(total), target);
    bar.set_style(style);
    bar.set_prefix(styled_label(Level::INFO, next_id()));
    bar.set_message(message.to_owned());
    bar.enable_steady_tick(Duration::from_millis(80));
    bar
}

#[cfg(test)]
mod tests {
    use indicatif::ProgressDrawTarget;
    use tracing::Level;

    use super::{build_download_bar, next_id, styled_label, underlined};

    #[test]
    fn labels_every_level() {
        for (level, name) in [
            (Level::ERROR, "ERROR"),
            (Level::WARN, "WARN"),
            (Level::INFO, "INFO"),
            (Level::DEBUG, "DEBUG"),
            (Level::TRACE, "TRACE"),
        ] {
            let label = styled_label(level, 7);
            assert!(label.contains(name), "missing {name} in {label:?}");
            assert!(label.contains("[0007]"));
        }
    }

    #[test]
    fn underlines_the_name() {
        assert!(underlined("runner.tar.gz").contains("runner.tar.gz"));
    }

    #[test]
    fn ids_keep_increasing() {
        assert!(next_id() < next_id());
    }

    #[test]
    fn builds_a_progress_bar_with_the_total() {
        let bar = build_download_bar(ProgressDrawTarget::hidden(), 42, "Downloading runner");
        assert_eq!(bar.length(), Some(42));
        bar.finish_and_clear();
    }
}
