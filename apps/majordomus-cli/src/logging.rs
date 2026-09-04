//! Diagnostics go to stderr, structured, filtered by `MAJORDOMUS_LOG` (default `info`).
//! stdout is never touched here: it belongs to the protocol.

use tracing_subscriber::EnvFilter;

/// The environment variable that sets the log filter, e.g. `debug` or `majordomus_cli=trace`.
pub const LOG_ENV: &str = "MAJORDOMUS_LOG";

pub fn init() {
    let filter = EnvFilter::try_from_env(LOG_ENV).unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_ansi(false)
        .try_init();
}
