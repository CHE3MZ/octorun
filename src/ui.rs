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

/// Create a download progress bar when stderr is a terminal.
#[must_use]
pub(crate) fn download_bar(total: u64, message: &str) -> Option<ProgressBar> {
    if !Term::stderr().is_term() {
        return None;
    }

    let style = ProgressStyle::with_template(
        "{prefix} {spinner} {msg}  {bar:10} {percent}% {bytes_per_sec}",
    )
    .expect("valid download progress template")
    .tick_strings(SPINNER_FRAMES)
    .progress_chars("█░");

    let bar = ProgressBar::with_draw_target(Some(total), ProgressDrawTarget::stderr());
    bar.set_style(style);
    bar.set_prefix(styled_label(Level::INFO, next_id()));
    bar.set_message(message.to_owned());
    bar.enable_steady_tick(Duration::from_millis(80));

    Some(bar)
}
