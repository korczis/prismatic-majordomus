//! The typed execution event: one envelope, a closed set of payloads, and a version.
//!
//! An event is a domain fact, not a message of a transport. The WebSocket endpoint, the
//! history route, the audit line and anything added later render the same value; nothing
//! downstream may add a field, and nothing upstream may send a string a consumer has to
//! parse. The schema every client validates against is derived from these types by
//! `schemars`, exactly as a capability's input and output schemas are.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::model::{
    ExecutionDiagnostic, ExecutionError, ExecutionId, ExecutionState, LogStream, ProgressView,
};

/// The version of the event protocol this executable speaks.
///
/// It is carried on every event and negotiated when a client connects. A change that a
/// client written against version 1 could not ignore safely — a renamed field, a removed
/// variant, a changed meaning — increments it. A new variant does not, and neither does a
/// new state or a new envelope field: a client is required to ignore a payload type it
/// does not know, which is what [`EventPayload::is_terminal`] and the Cockpit's own reader
/// are written to do. A reader of this executable refuses an event of a newer version by
/// name rather than guessing at it ([`check_schema_version`]).
pub const PROTOCOL_VERSION: &str = "1";

/// May a reader of this executable read an event of `version`? An older version or this
/// one, yes; a newer one, or a word that is not a version, is refused with a sentence that
/// names it, so a reader that meets an event from a newer writer says so instead of
/// misreading it.
///
/// ```
/// use majordomus_cli::execution::event::check_schema_version;
/// assert!(check_schema_version("1").is_ok());
/// let refused = check_schema_version("2").unwrap_err();
/// assert!(refused.contains("version 2") && refused.contains("newer"), "{refused}");
/// assert!(check_schema_version("one").is_err());
/// ```
pub fn check_schema_version(version: &str) -> Result<(), String> {
    let ours: u32 = PROTOCOL_VERSION.parse().unwrap_or(1);
    match version.parse::<u32>() {
        Ok(theirs) if (1..=ours).contains(&theirs) => Ok(()),
        Ok(theirs) if theirs > ours => Err(format!(
            "an execution event of schema version {theirs} is newer than version {ours}, the newest this executable reads; upgrade majordomus to read it"
        )),
        _ => Err(format!(
            "{version:?} is not an execution event schema version this executable reads (1 to {ours})"
        )),
    }
}

/// Deserialise `schema_version`, refusing one [`check_schema_version`] refuses: every
/// reader of an event, whatever transport it came by, refuses a newer one the same way.
fn read_schema_version<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let version = String::deserialize(deserializer)?;
    check_schema_version(&version).map_err(serde::de::Error::custom)?;
    Ok(version)
}

/// One event about one execution.
///
/// `sequence` is dense and starts at 1 within an execution: a client that has seen
/// sequence `n` knows it has missed something when the next event it reads is not `n + 1`,
/// and asks for the gap by cursor rather than reloading the world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExecutionEvent {
    /// The protocol version, [`PROTOCOL_VERSION`]. A reader refuses a newer one by name.
    #[serde(deserialize_with = "read_schema_version")]
    #[schemars(with = "String")]
    pub schema_version: String,
    /// The identity of this event: the execution's id and the sequence, so a client that
    /// deduplicates needs no composite key of its own.
    pub event_id: String,
    /// The execution it is about.
    pub execution_id: ExecutionId,
    /// Its position in that execution's stream, from 1, without gaps.
    pub sequence: u64,
    /// When the event was produced, RFC 3339 in UTC.
    pub timestamp: String,
    /// The id this execution's log lines and tracing spans carry: the execution id, under
    /// the name a reader of a log expects, as the snapshot's own `correlation_id` is. Absent
    /// from an event an older writer produced, and read as empty there.
    #[serde(default)]
    pub correlation_id: String,
    /// What happened.
    #[serde(flatten)]
    pub payload: EventPayload,
}

impl ExecutionEvent {
    /// The event for an execution at a sequence, stamped now.
    pub fn new(execution_id: ExecutionId, sequence: u64, payload: EventPayload) -> Self {
        ExecutionEvent {
            schema_version: PROTOCOL_VERSION.to_string(),
            event_id: format!("{execution_id}#{sequence}"),
            correlation_id: execution_id.to_string(),
            execution_id,
            sequence,
            timestamp: crate::peers::rfc3339(std::time::SystemTime::now()),
            payload,
        }
    }

    /// The discriminator on the wire, for a log line or a filter.
    pub fn type_name(&self) -> &'static str {
        self.payload.type_name()
    }
}

/// What happened, as a closed set.
///
/// The tag is `type` and the body is `data`, so every event on the wire has the same two
/// members whatever it says, and a client switches on one string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", content = "data")]
pub enum EventPayload {
    /// The execution was accepted and exists. Carries what it will run and with what,
    /// redacted, so a client that joins at the first event needs nothing else to render a
    /// heading.
    #[serde(rename = "execution.created")]
    Created {
        /// The capability's canonical id.
        capability: String,
        /// Its title.
        title: String,
        /// The input, with sensitive values already replaced.
        input: Value,
    },
    /// A policy decision on whether it may run is being taken.
    #[serde(rename = "execution.authorizing")]
    Authorizing,
    /// It is waiting for a worker.
    #[serde(rename = "execution.queued")]
    Queued {
        /// How many executions are ahead of it.
        ahead: usize,
    },
    /// A worker holds it and has not entered the handler yet.
    #[serde(rename = "execution.claimed")]
    Claimed {
        /// Who holds it, in the holder's own words: a worker, a node, a session.
        holder: String,
    },
    /// A worker picked it up and is about to enter the handler.
    #[serde(rename = "execution.started")]
    Started,
    /// The handler returned, and what it produced is being checked.
    #[serde(rename = "execution.verifying")]
    Verifying,
    /// How far along it is.
    #[serde(rename = "execution.progress")]
    Progress(ProgressView),
    /// A named phase was entered.
    #[serde(rename = "execution.step.started")]
    StepStarted {
        /// The step's stable name.
        name: String,
        /// One line for a reader.
        title: String,
    },
    /// A named phase finished.
    #[serde(rename = "execution.step.completed")]
    StepCompleted {
        /// The step's stable name.
        name: String,
        /// Whether what it was asked to do happened.
        ok: bool,
        /// What it said, when it said anything.
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    /// A line of output.
    #[serde(rename = "execution.log")]
    Log {
        /// Which stream it came from.
        stream: LogStream,
        /// The text, with control characters and terminal escapes already removed.
        message: String,
    },
    /// A structured finding, distinct from the outcome.
    #[serde(rename = "execution.diagnostic")]
    Diagnostic(ExecutionDiagnostic),
    /// Cancellation was asked for; the handler has not stopped yet.
    #[serde(rename = "execution.cancelling")]
    Cancelling {
        /// Who asked, in one word.
        by: String,
    },
    /// It stopped because it was asked to.
    #[serde(rename = "execution.cancelled")]
    Cancelled,
    /// It finished, and the handler returned an output.
    ///
    /// The output is on this event and not on one of its own, because a client must never
    /// be able to read "succeeded" and find no result: the state and the value it produced
    /// are one fact and travel together.
    #[serde(rename = "execution.completed")]
    Completed {
        /// What the handler returned.
        output: Value,
    },
    /// It finished, and it did not.
    #[serde(rename = "execution.failed")]
    Failed {
        /// Why.
        error: ExecutionError,
    },
    /// A policy refused it before it ran: at the request, or when a worker claimed it.
    #[serde(rename = "execution.refused")]
    Refused {
        /// Why, with the code `refused` or `forbidden`.
        error: ExecutionError,
        /// The policy that decided, when one can be named.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        policy: Option<String>,
    },
    /// Whether the work happened is not known: the worker went silent, or its answer could
    /// not be read. Not the last event: a reconciliation that reads evidence follows it.
    #[serde(rename = "execution.unknown_outcome")]
    UnknownOutcome {
        /// What is known about why the outcome is not.
        error: ExecutionError,
    },
}

impl EventPayload {
    /// The discriminator on the wire.
    ///
    /// ```
    /// use majordomus_cli::execution::EventPayload;
    /// assert_eq!(EventPayload::Started.type_name(), "execution.started");
    /// assert_eq!(EventPayload::Cancelled.type_name(), "execution.cancelled");
    /// ```
    pub fn type_name(&self) -> &'static str {
        match self {
            EventPayload::Created { .. } => "execution.created",
            EventPayload::Authorizing => "execution.authorizing",
            EventPayload::Queued { .. } => "execution.queued",
            EventPayload::Claimed { .. } => "execution.claimed",
            EventPayload::Started => "execution.started",
            EventPayload::Verifying => "execution.verifying",
            EventPayload::Progress(_) => "execution.progress",
            EventPayload::StepStarted { .. } => "execution.step.started",
            EventPayload::StepCompleted { .. } => "execution.step.completed",
            EventPayload::Log { .. } => "execution.log",
            EventPayload::Diagnostic(_) => "execution.diagnostic",
            EventPayload::Cancelling { .. } => "execution.cancelling",
            EventPayload::Cancelled => "execution.cancelled",
            EventPayload::Completed { .. } => "execution.completed",
            EventPayload::Failed { .. } => "execution.failed",
            EventPayload::Refused { .. } => "execution.refused",
            EventPayload::UnknownOutcome { .. } => "execution.unknown_outcome",
        }
    }

    /// Is this the last event of its execution? A subscriber that has read one may stop.
    ///
    /// ```
    /// use majordomus_cli::execution::EventPayload;
    /// assert!(EventPayload::Cancelled.is_terminal());
    /// assert!(!EventPayload::Started.is_terminal());
    /// assert!(!EventPayload::Verifying.is_terminal());
    /// ```
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            EventPayload::Completed { .. }
                | EventPayload::Failed { .. }
                | EventPayload::Cancelled
                | EventPayload::Refused { .. }
        )
    }

    /// The state this event puts the execution into, when it decides one.
    pub fn implies_state(&self) -> Option<ExecutionState> {
        match self {
            EventPayload::Authorizing => Some(ExecutionState::Authorizing),
            EventPayload::Queued { .. } => Some(ExecutionState::Queued),
            EventPayload::Claimed { .. } => Some(ExecutionState::Claimed),
            EventPayload::Started => Some(ExecutionState::Running),
            EventPayload::Verifying => Some(ExecutionState::Verifying),
            EventPayload::Cancelling { .. } => Some(ExecutionState::Cancelling),
            EventPayload::Cancelled => Some(ExecutionState::Cancelled),
            EventPayload::Completed { .. } => Some(ExecutionState::Succeeded),
            EventPayload::Failed { .. } => Some(ExecutionState::Failed),
            EventPayload::Refused { .. } => Some(ExecutionState::Refused),
            EventPayload::UnknownOutcome { .. } => Some(ExecutionState::UnknownOutcome),
            _ => None,
        }
    }
}

/// Text as an event may carry it: no control character, no terminal escape sequence, and
/// bounded.
///
/// Every line a handler or a child process produces goes through here. A browser is not
/// the only reason — a terminal reading `majordomus run --follow` would be repainted by an
/// escape it did not choose — and doing it once, here, is why no consumer has to.
///
/// ```
/// use majordomus_cli::execution::event::sanitise;
/// assert_eq!(sanitise("plain", 100), "plain");
/// assert_eq!(sanitise("\u{1b}[31mred\u{1b}[0m", 100), "red");
/// assert_eq!(sanitise("a\u{0}b\tc", 100), "a b\tc");
/// assert_eq!(sanitise("abcdef", 4), "abc…");
/// ```
pub fn sanitise(text: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(text.len().min(max_chars));
    let mut chars = text.chars().peekable();
    let mut count = 0usize;
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // the character after the escape says which sequence it is, and the two have
            // different terminators: a control sequence ends at its final byte, an
            // operating-system command at a BEL or a string terminator. Treating them
            // alike leaves half of one on the wire.
            match chars.next() {
                Some('[') => {
                    for next in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&next) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(next) = chars.next() {
                        if next == '\u{7}' {
                            break;
                        }
                        if next == '\u{1b}' {
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                // any other two-character escape: both are dropped
                _ => {}
            }
            continue;
        }
        let c = if c == '\t' || !c.is_control() { c } else { ' ' };
        if count + 1 >= max_chars {
            out.push('…');
            return out;
        }
        out.push(c);
        count += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(payload: EventPayload) -> ExecutionEvent {
        ExecutionEvent::new(ExecutionId::fresh(), 1, payload)
    }

    fn error(code: &str) -> ExecutionError {
        ExecutionError {
            code: code.into(),
            message: String::new(),
            suggestion: None,
            correlation_id: String::new(),
        }
    }

    #[test]
    fn an_event_is_one_envelope_with_a_type_and_a_data_member() {
        let e = event(EventPayload::Progress(ProgressView {
            current: 17,
            total: Some(42),
            message: Some("Validating".into()),
        }));
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["schema_version"], PROTOCOL_VERSION);
        assert_eq!(v["type"], "execution.progress");
        assert_eq!(v["data"]["current"], 17);
        assert_eq!(v["sequence"], 1);
        assert_eq!(v["event_id"], format!("{}#1", e.execution_id));
        assert_eq!(v["correlation_id"], e.execution_id.as_str());
        let back: ExecutionEvent = serde_json::from_value(v).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn every_payload_names_itself_and_round_trips() {
        let payloads = [
            EventPayload::Created {
                capability: "demo.x".into(),
                title: "Demo".into(),
                input: serde_json::json!({}),
            },
            EventPayload::Authorizing,
            EventPayload::Queued { ahead: 0 },
            EventPayload::Claimed {
                holder: "worker-1".into(),
            },
            EventPayload::Started,
            EventPayload::Verifying,
            EventPayload::Progress(ProgressView {
                current: 1,
                total: None,
                message: None,
            }),
            EventPayload::StepStarted {
                name: "a".into(),
                title: "A".into(),
            },
            EventPayload::StepCompleted {
                name: "a".into(),
                ok: true,
                detail: None,
            },
            EventPayload::Log {
                stream: LogStream::Stdout,
                message: "hi".into(),
            },
            EventPayload::Diagnostic(ExecutionDiagnostic {
                severity: crate::model::Severity::Warning,
                code: "slow".into(),
                summary: "s".into(),
                detail: None,
                suggestion: None,
            }),
            EventPayload::Cancelling {
                by: "client".into(),
            },
            EventPayload::Cancelled,
            EventPayload::Completed {
                output: serde_json::json!({ "ok": true }),
            },
            EventPayload::Failed {
                error: ExecutionError {
                    code: "internal".into(),
                    message: "m".into(),
                    suggestion: None,
                    correlation_id: "x".into(),
                },
            },
            EventPayload::Refused {
                error: ExecutionError {
                    code: "refused".into(),
                    message: "m".into(),
                    suggestion: None,
                    correlation_id: "x".into(),
                },
                policy: Some("p".into()),
            },
            EventPayload::UnknownOutcome {
                error: ExecutionError {
                    code: "unavailable".into(),
                    message: "m".into(),
                    suggestion: None,
                    correlation_id: "x".into(),
                },
            },
        ];
        let mut names = std::collections::BTreeSet::new();
        for p in payloads {
            let name = p.type_name();
            assert!(names.insert(name), "{name} is claimed twice");
            let v = serde_json::to_value(&p).unwrap();
            assert_eq!(v["type"], name);
            let back: EventPayload = serde_json::from_value(v).unwrap();
            assert_eq!(back.type_name(), name);
            assert_eq!(back.is_terminal(), p.is_terminal());
        }
        assert_eq!(names.len(), 17);
    }

    #[test]
    fn every_state_is_announced_by_exactly_one_payload() {
        let payloads = [
            EventPayload::Authorizing,
            EventPayload::Queued { ahead: 0 },
            EventPayload::Claimed { holder: "w".into() },
            EventPayload::Started,
            EventPayload::Verifying,
            EventPayload::Cancelling { by: "c".into() },
            EventPayload::UnknownOutcome {
                error: error("unavailable"),
            },
            EventPayload::Completed {
                output: Value::Null,
            },
            EventPayload::Failed {
                error: error("internal"),
            },
            EventPayload::Cancelled,
            EventPayload::Refused {
                error: error("refused"),
                policy: None,
            },
        ];
        let states: Vec<_> = payloads
            .iter()
            .filter_map(EventPayload::implies_state)
            .collect();
        assert_eq!(
            states,
            ExecutionState::ALL,
            "one payload per state, in the same order"
        );
        for p in &payloads {
            assert_eq!(
                p.is_terminal(),
                p.implies_state().is_some_and(ExecutionState::is_final),
                "{}",
                p.type_name()
            );
        }
    }

    #[test]
    fn a_reader_refuses_an_event_of_a_newer_schema_by_name() {
        let e = event(EventPayload::Started);
        let mut v = serde_json::to_value(&e).unwrap();
        // an older writer's event, without a correlation id, is still read
        v.as_object_mut().unwrap().remove("correlation_id");
        let older: ExecutionEvent = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(older.correlation_id, "");
        v["schema_version"] = "2".into();
        let refused = serde_json::from_value::<ExecutionEvent>(v.clone()).unwrap_err();
        let said = refused.to_string();
        assert!(
            said.contains("schema version 2") && said.contains("newer than version 1"),
            "{said}"
        );
        v["schema_version"] = "".into();
        assert!(
            serde_json::from_value::<ExecutionEvent>(v).is_err(),
            "not a version"
        );
    }

    #[test]
    fn a_terminal_payload_is_exactly_a_final_state() {
        for p in [
            EventPayload::Completed {
                output: Value::Null,
            },
            EventPayload::Cancelled,
            EventPayload::Failed {
                error: ExecutionError {
                    code: "internal".into(),
                    message: String::new(),
                    suggestion: None,
                    correlation_id: String::new(),
                },
            },
        ] {
            assert!(p.is_terminal());
            assert!(p.implies_state().is_some_and(ExecutionState::is_final));
        }
        assert_eq!(
            EventPayload::Started.implies_state(),
            Some(ExecutionState::Running)
        );
        assert_eq!(
            EventPayload::Started.implies_state().map(|s| s.is_final()),
            Some(false)
        );
    }

    #[test]
    fn text_is_stripped_of_escapes_and_bounded_once_here() {
        assert_eq!(sanitise("\u{1b}[1;32mok\u{1b}[0m done", 64), "ok done");
        assert_eq!(sanitise("\u{1b}]0;title\u{7}x", 64), "x");
        assert_eq!(sanitise("line\r\n", 64), "line  ");
        let long = "x".repeat(10_000);
        let cut = sanitise(&long, 80);
        assert!(cut.chars().count() <= 80, "{}", cut.chars().count());
        assert!(cut.ends_with('…'));
    }
}
