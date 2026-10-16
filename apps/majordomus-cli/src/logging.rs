//! Where diagnostics go, and where they must never go.
//!
//! Every diagnostic this executable emits is written to **stderr**, structured, filtered by
//! [`LOG_ENV`] and defaulting to `info`. stdout is never touched here, and that is the whole
//! reason this module exists as a boundary rather than as two lines in `main`: stdout
//! belongs to the protocol. An MCP client reads JSON-RPC framing from it, a shell reads the
//! JSON a command printed, and one stray line of logging on that stream is a parse error in
//! the caller rather than a message to a person.
//!
//! Nothing the log writes carries a credential the redactor knows (I2708). A server started
//! in the background writes this stream to `server.log`, a file nobody reads before it is
//! kept, and a field logged at a call site — a request path, an origin, an error's text —
//! can hold whatever a caller sent. So every field of every event and span, the message
//! among them, is formatted through one layer, [`subscriber`]'s field formatter, which
//! passes the field's text through `redaction::redact_secrets` before it is written. There
//! is no second rule set and no per-call-site opt-in: a field added tomorrow is redacted
//! because it is a field. A string field is redacted before it is quoted, and a debugged
//! value whose escaped quotes hide a shape is redacted as if they were plain, so an escape
//! cannot hide a credential from the redactor.
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

use std::fmt;

use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::field::RecordFields;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FormatFields, MakeWriter};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use crate::redaction;

/// The environment variable that sets the log filter, e.g. `debug` or `majordomus_cli=trace`.
pub const LOG_ENV: &str = "MAJORDOMUS_LOG";

/// Install the stderr subscriber once; a second call is a no-op.
pub fn init() {
    let filter = EnvFilter::try_from_env(LOG_ENV).unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = subscriber(filter, std::io::stderr).try_init();
}

/// The subscriber [`init`] installs, writing to `writer` under `filter`.
///
/// It is the one place the log's format is decided: no target, no colour, and every field
/// redacted. [`init`] hands it stderr; a test hands it a file and reads back what was
/// written.
///
/// ```
/// use std::sync::{Arc, Mutex};
/// use majordomus_cli::logging;
/// use tracing_subscriber::EnvFilter;
///
/// let written = Arc::new(Mutex::new(Vec::new()));
/// let sink = written.clone();
/// let to_memory = move || Sink(sink.clone());
/// // assembled at run time, so that no committed file carries a credential's shape
/// let token = format!("{}{}", "ghp_", "a".repeat(24));
/// let log = logging::subscriber(EnvFilter::new("info"), to_memory);
/// tracing::subscriber::with_default(log, || {
///     tracing::info!(token = token.as_str(), "pushing with {token}");
/// });
/// let text = String::from_utf8(written.lock().unwrap().clone()).unwrap();
/// assert!(!text.contains(&token), "{text}");
/// assert!(text.contains("pushing with [redacted:github-token]"), "{text}");
/// assert!(text.contains(r#"token="[redacted:github-token]""#), "{text}");
///
/// struct Sink(Arc<Mutex<Vec<u8>>>);
/// impl std::io::Write for Sink {
///     fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
///         self.0.lock().unwrap().extend_from_slice(buf);
///         Ok(buf.len())
///     }
///     fn flush(&mut self) -> std::io::Result<()> {
///         Ok(())
///     }
/// }
/// ```
pub fn subscriber<W>(filter: EnvFilter, writer: W) -> impl Subscriber + Send + Sync + 'static
where
    W: for<'w> MakeWriter<'w> + Send + Sync + 'static,
{
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .fmt_fields(RedactedFields)
        .with_writer(writer)
        .with_target(false)
        .with_ansi(false)
        .finish()
}

/// The field formatter of [`subscriber`]: the default `name=value` layout, with every
/// value's text passed through [`redact`] first.
struct RedactedFields;

impl<'writer> FormatFields<'writer> for RedactedFields {
    fn format_fields<R: RecordFields>(&self, writer: Writer<'writer>, fields: R) -> fmt::Result {
        let mut visitor = Redacting {
            writer,
            empty: true,
            result: Ok(()),
        };
        fields.record(&mut visitor);
        visitor.result
    }
}

/// Writes the fields of one event or span, each redacted, separated by a space.
struct Redacting<'writer> {
    writer: Writer<'writer>,
    empty: bool,
    result: fmt::Result,
}

impl Redacting<'_> {
    /// Write one field whose value reads as `text`; `quoted` writes it as a string literal,
    /// as the default formatter writes a string field, after it is redacted.
    fn write(&mut self, field: &Field, text: &str, quoted: bool) {
        if self.result.is_err() {
            return;
        }
        let clean = redact(text);
        let gap = if self.empty { "" } else { " " };
        self.empty = false;
        let name = field.name();
        self.result = if name == "message" {
            write!(self.writer, "{gap}{clean}")
        } else {
            let name = name.strip_prefix("r#").unwrap_or(name);
            if quoted {
                write!(self.writer, "{gap}{name}={clean:?}")
            } else {
                write!(self.writer, "{gap}{name}={clean}")
            }
        };
    }
}

impl Visit for Redacting<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.write(field, value, true);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.write(
            field,
            &unescaped_where_it_hides(&format!("{value:?}")),
            false,
        );
    }
}

/// A debugged value's text, with its escaped quotes taken literally where that is what
/// hides a credential.
///
/// `Debug` writes a string inside a value as an escaped literal, so `"password": "…"` reaches
/// the redactor as `\"password\": \"…` and the assignment rule, which reads the quotes,
/// never fires. The text is returned as it is unless reading `\"` as `"` lets a shape fire;
/// then the unescaped text is returned, and `Redacting::write` redacts it.
fn unescaped_where_it_hides(text: &str) -> String {
    let loose = text.replace("\\\"", "\"");
    let markers = |t: &str| redact(t).matches("[redacted:").count();
    if loose != text && markers(&loose) > markers(text) {
        loose
    } else {
        text.to_string()
    }
}

/// The one place a log field's text meets the redactor.
fn redact(text: &str) -> String {
    redaction::redact_secrets(text).text
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

    #[test]
    fn an_escaped_quote_does_not_hide_an_assignment() {
        let value = format!("{}{}", "\"password\": \"", "s".repeat(16));
        let debugged = format!("{value:?}");
        let loose = unescaped_where_it_hides(&debugged);
        assert!(redact(&loose).contains("[redacted:assignment]"), "{loose}");
        // text the escape hides nothing in is written as it was debugged
        assert_eq!(
            unescaped_where_it_hides(r#""a \"quoted\" word""#),
            r#""a \"quoted\" word""#
        );
    }

    #[test]
    fn a_field_s_text_is_redacted_and_other_text_is_kept() {
        let key = format!("{}{}", "sk-ant-", "q".repeat(24));
        assert_eq!(
            redact(&format!("with {key}")),
            "with [redacted:anthropic-key]"
        );
        assert_eq!(redact("GET /api/v1/health"), "GET /api/v1/health");
    }
}
