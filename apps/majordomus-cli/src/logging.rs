//! Where diagnostics go, and where they must never go.
//!
//! Every diagnostic this executable emits is written to **stderr**, structured, filtered by
//! [`LOG_ENV`] and defaulting to `info`. stdout is never touched here, and that is the whole
//! reason this module exists as a boundary rather than as two lines in `main`: stdout
//! belongs to the protocol. An MCP client reads JSON-RPC framing from it, a shell reads the
//! JSON a command printed, and one stray line of logging on that stream is a parse error in
//! the caller rather than a message to a person.
//!
//! The subscriber is installed once per process. A second call is a no-op rather than an
//! error, so a command that initialises defensively cannot fail because something already
//! did it.
//!
//! ```
//! use majordomus_cli::logging;
//!
//! // installing twice is safe, which is the property every caller relies on
//! logging::init();
//! logging::init();
//! assert_eq!(logging::LOG_ENV, "MAJORDOMUS_LOG");
//! ```

use tracing_subscriber::EnvFilter;

/// The environment variable that sets the log filter, e.g. `debug` or `majordomus_cli=trace`.
pub const LOG_ENV: &str = "MAJORDOMUS_LOG";

/// Install the stderr subscriber once; a second call is a no-op.
pub fn init() {
    let filter = EnvFilter::try_from_env(LOG_ENV).unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_ansi(false)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installing_the_subscriber_twice_is_not_an_error() {
        // `main` initialises, and so does anything that runs a command in-process; the
        // second caller must not fail because the first one won
        init();
        init();
    }

    #[test]
    fn the_filter_variable_is_the_one_the_documentation_names() {
        // read by a person from the documentation and by this module from the environment;
        // if they were two strings they could differ, so they are one
        assert_eq!(LOG_ENV, "MAJORDOMUS_LOG");
    }
}
