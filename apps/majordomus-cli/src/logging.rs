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

use std::io::Write;

use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::EnvFilter;

/// The environment variable that sets the log filter, e.g. `debug` or `majordomus_cli=trace`.
pub const LOG_ENV: &str = "MAJORDOMUS_LOG";

/// Install the stderr subscriber once; a second call is a no-op.
pub fn init() {
    let _ = subscriber(Redacting::stderr).try_init();
}

/// The subscriber every process installs, writing through `make_writer`: one place, so the
/// test that proves redaction proves the subscriber `init` installs.
fn subscriber<W>(
    make_writer: W,
) -> tracing_subscriber::fmt::SubscriberBuilder<
    tracing_subscriber::fmt::format::DefaultFields,
    tracing_subscriber::fmt::format::Format,
    EnvFilter,
    W,
>
where
    W: for<'w> MakeWriter<'w> + Send + Sync + 'static,
{
    let filter = EnvFilter::try_from_env(LOG_ENV).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(make_writer)
        .with_target(false)
        .with_ansi(false)
}

/// One formatted log record with every credential shape the redactor knows taken out.
///
/// The formatter hands its writer one whole record at a time, message and fields together,
/// so redacting the record is redacting every tracing field: a field added at any call site
/// tomorrow is covered without that call site knowing.
///
/// ```
/// use majordomus_cli::logging::redact_line;
/// // assembled at run time, so that no committed file carries a credential's shape
/// let token = format!("{}{}", "ghp_", "a".repeat(24));
/// let line = format!("WARN push failed token={token}\n");
/// assert_eq!(redact_line(line.as_bytes()), b"WARN push failed token=[redacted:github-token]\n");
/// ```
pub fn redact_line(record: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(record);
    crate::redaction::redact_secrets(&text).text.into_bytes()
}

/// A writer that redacts each record before it reaches `inner`: stderr in every process,
/// a buffer in a test.
///
/// ```
/// use std::io::Write;
/// use majordomus_cli::logging::Redacting;
/// // assembled at run time, so that no committed file carries a credential's shape
/// let token = format!("{}{}", "ghp_", "a".repeat(24));
/// let mut out = Redacting(Vec::new());
/// // the formatter writes one whole record at a time
/// out.write_all(format!("header={token}\n").as_bytes()).unwrap();
/// assert_eq!(String::from_utf8(out.0).unwrap(), "header=[redacted:github-token]\n");
/// ```
pub struct Redacting<W: Write>(pub W);

impl Redacting<std::io::Stderr> {
    /// The process's stderr, redacted: what [`init`] installs.
    ///
    /// ```
    /// use std::io::Write;
    /// let mut err = majordomus_cli::logging::Redacting::stderr();
    /// err.flush().unwrap();
    /// ```
    pub fn stderr() -> Self {
        Redacting(std::io::stderr())
    }
}

impl<W: Write> Write for Redacting<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // the whole record or nothing: a short write would split a credential across two
        // redactions, and neither half would match its shape
        self.0.write_all(&redact_line(buf))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
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

    /// A buffer every writer of the test subscriber appends to.
    #[derive(Clone, Default)]
    struct Shared(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_credential_in_any_tracing_field_never_reaches_the_log() {
        let log = Shared::default();
        let sink = log.clone();
        let subscriber = subscriber(move || Redacting(sink.clone())).finish();
        // assembled at run time, so that no committed file carries a credential's shape
        let token = format!("{}{}", "ghp_", "a".repeat(24));
        tracing::subscriber::with_default(subscriber, || {
            // a field nobody planned for, a display field, and the message itself
            tracing::warn!(origin = %token, header = ?token, "refused a request with {token}");
        });
        let written = String::from_utf8(log.0.lock().unwrap().clone()).unwrap();
        assert!(written.contains("refused a request"), "{written}");
        assert!(!written.contains(&token), "{written}");
        assert!(written.contains("[redacted:github-token]"), "{written}");
    }

    #[test]
    fn the_filter_variable_is_the_one_the_documentation_names() {
        // read by a person from the documentation and by this module from the environment;
        // if they were two strings they could differ, so they are one
        assert_eq!(LOG_ENV, "MAJORDOMUS_LOG");
    }
}
