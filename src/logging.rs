//! Terminal logging setup.

use std::fmt;
use std::io;

use tracing::{Event, Level, Subscriber};
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::registry::LookupSpan;

use crate::ui;

/// Initialise logging to stderr.
pub fn init(verbose: bool) {
    let level = if verbose { Level::DEBUG } else { Level::INFO };

    let _ = tracing_subscriber::fmt()
        .event_format(LogFormat)
        .with_max_level(level)
        .with_writer(io::stderr)
        .try_init();
}

struct LogFormat;

impl<S, N> FormatEvent<S, N> for LogFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let label = ui::styled_label(*event.metadata().level(), ui::next_id());
        write!(writer, "{label} ")?;
        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}
