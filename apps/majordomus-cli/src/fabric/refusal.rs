//! Why the host did not answer a request with an output: one stable code per reason, the
//! account of whether anything ran, and whether the connection survives it.
//!
//! The codes are the contract. A client branches on [`RefusalCode`] and never parses
//! [`Refusal::message`], the way an HTTP or MCP caller branches on
//! [`CapabilityError::code`] today; the six codes that a capability's own handler returns
//! are spelled here exactly as there, so one failure has one name on every transport (rule
//! `project.operation-transport-parity`).
//!
//! ```
//! use majordomus_cli::fabric::{Delivery, Refusal, RefusalCode};
//!
//! let refused = Refusal::new(RefusalCode::ContextMismatch, "the session is bound to another context");
//! assert_eq!(refused.code().as_str(), "context_mismatch");
//! assert_eq!(refused.delivery(), Delivery::NotExecuted, "nothing ran");
//! assert!(!refused.terminal(), "the connection carries on");
//! ```

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::{CapabilityError, ExecutionPolicy};

/// One reason a request was not answered with an output. Serialised in `snake_case`; the
/// spelling of a code never changes once released, and a new reason is a new code.
///
/// A client older than its host can meet a code it does not know. It reads that code as
/// [`RefusalCode::Unrecognised`] instead of failing to read the frame, and acts on the
/// refusal's [`Refusal::delivery`] and [`Refusal::terminal`], which travel beside the code
/// for exactly that reader.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    /// A frame's length prefix is over the bound; its body was not read. Terminal.
    FrameOversized,
    /// The connection ended, or failed, inside a frame. Terminal.
    FrameTruncated,
    /// A frame arrived whole and its body is not a message of this protocol: not JSON,
    /// nested beyond the parser's bound, a field this protocol does not have, a missing
    /// required field, or an identifier that is not one.
    MessageMalformed,
    /// The first message of a connection was not `hello`. Terminal.
    HelloRequired,
    /// A second `hello` on a connection that already negotiated.
    HelloRepeated,
    /// The client's protocol range and the host's do not overlap. Terminal.
    ProtocolUnsupported,
    /// The client requires a feature the host does not offer. Terminal.
    FeatureUnsupported,
    /// A message that only a negotiated feature permits, on a connection that did not
    /// negotiate it.
    FeatureNotNegotiated,
    /// A capability id or an idempotency key that is not one by its grammar.
    InvalidIdentifier,
    /// The checkout root a client named is not an absolute, normal path within the bound.
    RootInvalid,
    /// The root names no checkout this host can serve: it does not exist, or no `.ai/`
    /// layer contains it.
    RootUnresolved,
    /// The socket path is not absolute, or is longer than the platform's socket address
    /// can hold.
    SocketPathInvalid,
    /// This platform has no local transport in this build.
    TransportUnavailable,
    /// The process on the other end of the socket is not this OS user's. Terminal.
    PeerRefused,
    /// The session is not one this connection holds: never minted, detached, revoked, or
    /// another connection's. One code for all four, so that a guessed handle learns
    /// nothing about which.
    SessionUnknown,
    /// The request names a context other than the one its session is bound to.
    ContextMismatch,
    /// The registry has no capability of that id.
    CapabilityUnknown,
    /// The capability exists and may not be run here: a resource, or a capability whose
    /// stability makes it listed and not executable.
    CapabilityDenied,
    /// A deadline of zero, or over the host's bound.
    DeadlineInvalid,
    /// The deadline passed before an output.
    DeadlineExceeded,
    /// The client cancelled the request before an output.
    Cancelled,
    /// The request may have run and nobody knows whether it did: a worker that died
    /// mid-call. Never retried for a capability that is not idempotent.
    OutcomeUnknown,
    /// The answer is larger than the reply bound; the capability ran.
    OutputOversized,
    /// A request id already in flight on this connection.
    RequestDuplicate,
    /// The connection already has the most requests in flight it may.
    TooManyInFlight,
    /// [`CapabilityError::InvalidInput`].
    InvalidInput,
    /// [`CapabilityError::NotFound`].
    NotFound,
    /// [`CapabilityError::Refused`].
    Refused,
    /// [`CapabilityError::Internal`].
    Internal,
    /// [`CapabilityError::Forbidden`].
    Forbidden,
    /// [`CapabilityError::Conflict`].
    Conflict,
    /// A code this build does not know, sent by a newer host. Never sent.
    #[serde(other)]
    Unrecognised,
}

impl RefusalCode {
    /// Every code, in declaration order: the list the documentation and the tests read.
    pub const ALL: &'static [RefusalCode] = &[
        RefusalCode::FrameOversized,
        RefusalCode::FrameTruncated,
        RefusalCode::MessageMalformed,
        RefusalCode::HelloRequired,
        RefusalCode::HelloRepeated,
        RefusalCode::ProtocolUnsupported,
        RefusalCode::FeatureUnsupported,
        RefusalCode::FeatureNotNegotiated,
        RefusalCode::InvalidIdentifier,
        RefusalCode::RootInvalid,
        RefusalCode::RootUnresolved,
        RefusalCode::SocketPathInvalid,
        RefusalCode::TransportUnavailable,
        RefusalCode::PeerRefused,
        RefusalCode::SessionUnknown,
        RefusalCode::ContextMismatch,
        RefusalCode::CapabilityUnknown,
        RefusalCode::CapabilityDenied,
        RefusalCode::DeadlineInvalid,
        RefusalCode::DeadlineExceeded,
        RefusalCode::Cancelled,
        RefusalCode::OutcomeUnknown,
        RefusalCode::OutputOversized,
        RefusalCode::RequestDuplicate,
        RefusalCode::TooManyInFlight,
        RefusalCode::InvalidInput,
        RefusalCode::NotFound,
        RefusalCode::Refused,
        RefusalCode::Internal,
        RefusalCode::Forbidden,
        RefusalCode::Conflict,
        RefusalCode::Unrecognised,
    ];

    /// The code as it is serialised.
    ///
    /// ```
    /// use majordomus_cli::fabric::RefusalCode;
    /// assert_eq!(RefusalCode::SessionUnknown.as_str(), "session_unknown");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            RefusalCode::FrameOversized => "frame_oversized",
            RefusalCode::FrameTruncated => "frame_truncated",
            RefusalCode::MessageMalformed => "message_malformed",
            RefusalCode::HelloRequired => "hello_required",
            RefusalCode::HelloRepeated => "hello_repeated",
            RefusalCode::ProtocolUnsupported => "protocol_unsupported",
            RefusalCode::FeatureUnsupported => "feature_unsupported",
            RefusalCode::FeatureNotNegotiated => "feature_not_negotiated",
            RefusalCode::InvalidIdentifier => "invalid_identifier",
            RefusalCode::RootInvalid => "root_invalid",
            RefusalCode::RootUnresolved => "root_unresolved",
            RefusalCode::SocketPathInvalid => "socket_path_invalid",
            RefusalCode::TransportUnavailable => "transport_unavailable",
            RefusalCode::PeerRefused => "peer_refused",
            RefusalCode::SessionUnknown => "session_unknown",
            RefusalCode::ContextMismatch => "context_mismatch",
            RefusalCode::CapabilityUnknown => "capability_unknown",
            RefusalCode::CapabilityDenied => "capability_denied",
            RefusalCode::DeadlineInvalid => "deadline_invalid",
            RefusalCode::DeadlineExceeded => "deadline_exceeded",
            RefusalCode::Cancelled => "cancelled",
            RefusalCode::OutcomeUnknown => "outcome_unknown",
            RefusalCode::OutputOversized => "output_oversized",
            RefusalCode::RequestDuplicate => "request_duplicate",
            RefusalCode::TooManyInFlight => "too_many_in_flight",
            RefusalCode::InvalidInput => "invalid_input",
            RefusalCode::NotFound => "not_found",
            RefusalCode::Refused => "refused",
            RefusalCode::Internal => "internal",
            RefusalCode::Forbidden => "forbidden",
            RefusalCode::Conflict => "conflict",
            RefusalCode::Unrecognised => "unrecognised",
        }
    }

    /// Whether the host closes the connection after sending this refusal. A framing
    /// failure leaves no next frame boundary to read from; a failed handshake leaves
    /// nothing to talk about; a peer of another user is not talked to at all.
    ///
    /// ```
    /// use majordomus_cli::fabric::RefusalCode;
    /// assert!(RefusalCode::FrameOversized.terminal());
    /// assert!(!RefusalCode::MessageMalformed.terminal(), "the stream is still in step");
    /// ```
    pub fn terminal(self) -> bool {
        matches!(
            self,
            RefusalCode::FrameOversized
                | RefusalCode::FrameTruncated
                | RefusalCode::HelloRequired
                | RefusalCode::ProtocolUnsupported
                | RefusalCode::FeatureUnsupported
                | RefusalCode::PeerRefused
        )
    }
}

impl fmt::Display for RefusalCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a client may conclude about the request's effect from a refusal. The same three
/// answers [`crate::mcp::bridge::BridgeError::may_resend`] distinguishes, carried on the
/// wire so that a client does not have to infer them from the code.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    /// Provably not run: refused before it reached a handler. Sending it again cannot run
    /// it twice.
    NotExecuted,
    /// It ran, and its answer could not be delivered.
    Executed,
    /// It may have run. Only an idempotent capability may be sent again.
    Unknown,
}

/// A refusal: the code, a sentence for a person, what ran, and whether the connection
/// ends. Built by the host from what the host itself established; the message names the
/// caller's own request and never another context's root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Refusal {
    code: RefusalCode,
    message: String,
    delivery: Delivery,
    terminal: bool,
}

impl Refusal {
    /// A refusal of a request that did not run.
    pub fn new(code: RefusalCode, message: impl Into<String>) -> Self {
        Refusal {
            code,
            message: message.into(),
            delivery: Delivery::NotExecuted,
            terminal: code.terminal(),
        }
    }

    /// The same refusal, saying what ran.
    ///
    /// ```
    /// use majordomus_cli::fabric::{Delivery, Refusal, RefusalCode};
    /// let lost = Refusal::new(RefusalCode::OutcomeUnknown, "the worker exited").with(Delivery::Unknown);
    /// assert_eq!(lost.delivery(), Delivery::Unknown);
    /// ```
    pub fn with(mut self, delivery: Delivery) -> Self {
        self.delivery = delivery;
        self
    }

    /// The refusal of a request whose deadline passed: not run when it never started, an
    /// unknown outcome when it had.
    ///
    /// ```
    /// use majordomus_cli::fabric::{Delivery, Refusal};
    /// assert_eq!(Refusal::deadline_exceeded(false).delivery(), Delivery::NotExecuted);
    /// assert_eq!(Refusal::deadline_exceeded(true).delivery(), Delivery::Unknown);
    /// ```
    pub fn deadline_exceeded(started: bool) -> Self {
        Refusal::new(
            RefusalCode::DeadlineExceeded,
            "the deadline passed before the capability answered",
        )
        .with(Self::started(started))
    }

    /// The refusal of a request the client cancelled, with the same account of what ran.
    ///
    /// ```
    /// use majordomus_cli::fabric::{Delivery, Refusal};
    /// assert_eq!(Refusal::cancelled(true).delivery(), Delivery::Unknown);
    /// ```
    pub fn cancelled(started: bool) -> Self {
        Refusal::new(
            RefusalCode::Cancelled,
            "the client cancelled the request before an answer",
        )
        .with(Self::started(started))
    }

    fn started(started: bool) -> Delivery {
        if started {
            Delivery::Unknown
        } else {
            Delivery::NotExecuted
        }
    }

    /// The code.
    pub fn code(&self) -> RefusalCode {
        self.code
    }

    /// The sentence for a person. Never parsed.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// What ran.
    pub fn delivery(&self) -> Delivery {
        self.delivery
    }

    /// Whether the connection ends after it.
    pub fn terminal(&self) -> bool {
        self.terminal
    }

    /// Whether the request may be sent again, on this connection or a new one, without the
    /// risk of running a capability twice: always when it did not run, and otherwise only
    /// when the capability's own execution policy says a second run changes nothing. The
    /// policy comes from the registry's descriptor, never from a tool name.
    ///
    /// ```
    /// use majordomus_cli::capability::{CapabilityKind, ExecutionPolicy};
    /// use majordomus_cli::fabric::{Delivery, Refusal, RefusalCode};
    ///
    /// let read = ExecutionPolicy::classify(CapabilityKind::Query);
    /// let write = ExecutionPolicy::classify(CapabilityKind::Command).writes_repository();
    /// let lost = Refusal::new(RefusalCode::OutcomeUnknown, "lost").with(Delivery::Unknown);
    /// assert!(lost.may_resend(read));
    /// assert!(!lost.may_resend(write), "a write that may have run is not run again");
    /// assert!(Refusal::deadline_exceeded(false).may_resend(write), "it never started");
    /// ```
    pub fn may_resend(&self, policy: ExecutionPolicy) -> bool {
        match self.delivery {
            Delivery::NotExecuted => true,
            Delivery::Executed | Delivery::Unknown => policy.hints().idempotent,
        }
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Refusal {}

impl From<&CapabilityError> for Refusal {
    /// A handler's own error, under the code it already has on HTTP and MCP. A handler that
    /// failed internally may have changed something before it failed, so its outcome is
    /// unknown; every other error is the handler declining before it acted.
    fn from(error: &CapabilityError) -> Self {
        let (code, delivery) = match error {
            CapabilityError::InvalidInput(_) => (RefusalCode::InvalidInput, Delivery::NotExecuted),
            CapabilityError::NotFound(_) => (RefusalCode::NotFound, Delivery::NotExecuted),
            CapabilityError::Refused(_) => (RefusalCode::Refused, Delivery::NotExecuted),
            CapabilityError::Internal(_) => (RefusalCode::Internal, Delivery::Unknown),
            CapabilityError::Forbidden(_) => (RefusalCode::Forbidden, Delivery::NotExecuted),
            CapabilityError::Conflict(_) => (RefusalCode::Conflict, Delivery::NotExecuted),
        };
        Refusal::new(code, error.to_string()).with(delivery)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::CapabilityKind;

    #[test]
    fn every_code_serialises_as_its_stable_spelling_and_reads_back() {
        let mut seen = std::collections::BTreeSet::new();
        for code in RefusalCode::ALL {
            let wire = serde_json::to_value(code).unwrap();
            assert_eq!(wire, serde_json::json!(code.as_str()), "{code:?}");
            assert_eq!(code.to_string(), code.as_str());
            let back: RefusalCode = serde_json::from_value(wire).unwrap();
            assert_eq!(back, *code);
            assert!(seen.insert(code.as_str()), "{code} is spelled twice");
        }
        // ALL is in declaration order, which the derived Ord follows: a code declared and
        // left out of ALL, or listed out of place, breaks this
        let mut sorted = RefusalCode::ALL.to_vec();
        sorted.sort();
        assert_eq!(sorted, RefusalCode::ALL);
        assert_eq!(RefusalCode::ALL.last(), Some(&RefusalCode::Unrecognised));
    }

    #[test]
    fn a_code_from_a_newer_host_reads_as_unrecognised_and_keeps_its_account() {
        let wire = serde_json::json!({
            "code": "quota_exhausted",
            "message": "a newer host's reason",
            "delivery": "not_executed",
            "terminal": true,
        });
        let refusal: Refusal = serde_json::from_value(wire).unwrap();
        assert_eq!(refusal.code(), RefusalCode::Unrecognised);
        assert!(
            refusal.terminal(),
            "the wire's account stands, not this build's table"
        );
        assert_eq!(refusal.delivery(), Delivery::NotExecuted);
        assert_eq!(refusal.message(), "a newer host's reason");
    }

    #[test]
    fn only_framing_handshake_and_peer_failures_end_the_connection() {
        let terminal: Vec<&str> = RefusalCode::ALL
            .iter()
            .filter(|c| c.terminal())
            .map(|c| c.as_str())
            .collect();
        assert_eq!(
            terminal,
            [
                "frame_oversized",
                "frame_truncated",
                "hello_required",
                "protocol_unsupported",
                "feature_unsupported",
                "peer_refused"
            ]
        );
        assert!(Refusal::new(RefusalCode::PeerRefused, "x").terminal());
        assert!(!Refusal::new(RefusalCode::SessionUnknown, "x").terminal());
    }

    #[test]
    fn deadlines_and_cancellation_say_whether_the_call_had_started() {
        assert_eq!(Refusal::cancelled(false).delivery(), Delivery::NotExecuted);
        assert_eq!(Refusal::cancelled(true).delivery(), Delivery::Unknown);
        assert_eq!(Refusal::cancelled(true).code(), RefusalCode::Cancelled);
        let late = Refusal::deadline_exceeded(true);
        assert_eq!(late.code(), RefusalCode::DeadlineExceeded);
        assert_eq!(
            late.to_string(),
            format!("deadline_exceeded: {}", late.message())
        );
    }

    #[test]
    fn resending_follows_the_capabilitys_declared_policy() {
        let read = ExecutionPolicy::classify(CapabilityKind::Query);
        let write = ExecutionPolicy::classify(CapabilityKind::Command).writes_repository();
        let ran = Refusal::new(RefusalCode::OutputOversized, "big").with(Delivery::Executed);
        assert!(ran.may_resend(read));
        assert!(!ran.may_resend(write));
        let unknown = Refusal::new(RefusalCode::OutcomeUnknown, "lost").with(Delivery::Unknown);
        assert!(!unknown.may_resend(write));
        assert!(Refusal::new(RefusalCode::ContextMismatch, "no").may_resend(write));
    }

    #[test]
    fn a_handlers_error_keeps_its_code_and_says_what_ran() {
        let cases = [
            (
                CapabilityError::InvalidInput("i".into()),
                Delivery::NotExecuted,
            ),
            (CapabilityError::NotFound("n".into()), Delivery::NotExecuted),
            (CapabilityError::Refused("r".into()), Delivery::NotExecuted),
            (CapabilityError::Internal("x".into()), Delivery::Unknown),
            (
                CapabilityError::Forbidden("f".into()),
                Delivery::NotExecuted,
            ),
            (CapabilityError::Conflict("c".into()), Delivery::NotExecuted),
        ];
        for (error, delivery) in cases {
            let refusal = Refusal::from(&error);
            assert_eq!(
                refusal.code().as_str(),
                error.code(),
                "one name on every transport"
            );
            assert_eq!(refusal.delivery(), delivery, "{error}");
            assert_eq!(refusal.message(), error.to_string());
            assert!(!refusal.terminal());
        }
    }

    #[test]
    fn a_refusal_with_a_field_this_protocol_lacks_does_not_read() {
        let wire = serde_json::json!({
            "code": "refused", "message": "m", "delivery": "unknown", "terminal": false, "uid": 0
        });
        assert!(serde_json::from_value::<Refusal>(wire).is_err());
        let refusal: &dyn std::error::Error = &Refusal::new(RefusalCode::Internal, "m");
        assert_eq!(refusal.to_string(), "internal: m");
    }
}
