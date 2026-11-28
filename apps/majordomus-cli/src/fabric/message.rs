//! The messages, the envelope around them, and the rules of one conversation.
//!
//! A client frame is `{"request": <n>, "message": {"type": ..., ...}}`; a host frame is
//! `{"reply_to": <n>, "message": {"type": ..., ...}}`, every request answered by exactly one
//! host frame. A connection opens with `hello` and its `welcome`, which fix the protocol
//! version and the features for the life of the connection, so no later frame carries a
//! version. Then a client `attach`es to a context by naming a checkout root, is handed a
//! session bound to that context and to this connection, and names both on every `call`.
//!
//! Every message reads closed: a field this protocol does not define is
//! [`RefusalCode::MessageMalformed`], so a field a client invents — a `uid`, a `root` on a
//! call — is refused rather than ignored, and an extension travels only behind a negotiated
//! feature. A field documented as optional may be left out, and reads as its default.
//!
//! ```
//! use majordomus_cli::fabric::{ClientFrame, ClientMessage, RefusalCode};
//!
//! let hello = br#"{"request":1,"message":{"type":"hello","protocol":{"min":1,"max":1}}}"#;
//! let frame = ClientFrame::decode(hello).unwrap();
//! assert!(matches!(frame.message, ClientMessage::Hello(_)));
//!
//! let forged = br#"{"request":2,"message":{"type":"health","uid":0}}"#;
//! let rejected = ClientFrame::decode(forged).unwrap_err();
//! assert_eq!(rejected.refusal.code(), RefusalCode::MessageMalformed);
//! assert_eq!(rejected.reply_to.map(|r| r.0), Some(2), "the refusal answers request 2");
//! ```

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::frame::{self, MAX_REPLY_FRAME_BYTES, MAX_REQUEST_FRAME_BYTES};
use super::ids::{
    CheckoutRoot, ClientLabel, ContextDescriptor, ContextId, FeatureSet, RequestId, SessionId,
};
use super::refusal::{Delivery, Refusal, RefusalCode};
use super::version::{ProtocolRange, ProtocolVersion, PROTOCOL};
use crate::capability::{Capability, CapabilityId, CapabilityRegistry};
use crate::execution::engine::runnable;
use crate::execution::IdempotencyKey;
use crate::mesh::identity::{InstanceId, NodeId, PublicIdentity};

/// The feature that lets a client send [`Cancel`].
pub const FEATURE_CANCEL: &str = "cancel";
/// The features this build offers.
pub const FEATURES: &[&str] = &[FEATURE_CANCEL];
/// The deadline of a call that names none: the MCP bridge's request timeout.
pub const DEFAULT_DEADLINE: Duration = crate::mcp::bridge::REQUEST_TIMEOUT;
/// The longest deadline a call may ask for. Work that needs longer is an execution
/// (`executions.start`), which answers at once and is followed rather than waited on.
pub const MAX_DEADLINE_MS: u32 = 600_000;
/// The most requests one connection may have in flight.
pub const MAX_IN_FLIGHT: usize = 64;

fn malformed(message: String) -> Refusal {
    Refusal::new(RefusalCode::MessageMalformed, message)
}

fn empty_object() -> Value {
    Value::Object(Default::default())
}

/// The first message of a connection: which versions the client speaks, the features it
/// would use, the ones it cannot do without, and what it calls itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    /// The versions the client speaks.
    pub protocol: ProtocolRange,
    /// Features the client would use if offered. Optional; unknown ones are ignored.
    #[serde(default)]
    pub features: FeatureSet,
    /// Features without which the client cannot proceed. Optional; an unknown one is
    /// [`RefusalCode::FeatureUnsupported`].
    #[serde(default)]
    pub required: FeatureSet,
    /// What the client calls itself, for listings. Optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<ClientLabel>,
}

/// Attach to the context of a checkout. The root is where the host starts its walk, and
/// the context it finds is the host's answer ([`ContextDescriptor::resolve`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Attach {
    /// The checkout root the client is in.
    pub root: CheckoutRoot,
}

/// End a session. Its id is unknown from then on, to this connection and every other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Detach {
    /// The session to end.
    pub session: SessionId,
}

/// Run one capability of the registry in a context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Call {
    /// The session the call runs under.
    pub session: SessionId,
    /// The context the call names. It must be the session's own: naming it makes a
    /// misdirected call a refusal ([`RefusalCode::ContextMismatch`]) instead of a call
    /// quietly run somewhere the caller did not mean.
    pub context: ContextId,
    /// The capability, by its canonical id.
    pub capability: CapabilityId,
    /// The capability's input. Optional; an empty object when left out.
    #[serde(default = "empty_object")]
    pub input: Value,
    /// How long the client will wait, in milliseconds, from the host's receipt. Optional;
    /// [`DEFAULT_DEADLINE`] when left out, and at most [`MAX_DEADLINE_MS`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 1, max = 600_000))]
    pub deadline_ms: Option<u32>,
    /// The key that makes a repeated call one call, in the grammar executions use.
    /// Optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 128), regex(pattern = r"^[A-Za-z0-9._:-]+$"))]
    pub idempotency_key: Option<String>,
}

/// Ask the host to stop a request in flight on this connection. Answered with [`Ack`]
/// whether or not the target was still running; the target's own reply says how it ended.
/// Only on a connection that negotiated [`FEATURE_CANCEL`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Cancel {
    /// The request to stop.
    pub target: RequestId,
}

/// Ask whether the host is answering. Needs no session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HealthQuery {}

/// Everything a client may send.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// [`Hello`].
    Hello(Hello),
    /// [`Attach`].
    Attach(Attach),
    /// [`Detach`].
    Detach(Detach),
    /// [`Call`].
    Call(Call),
    /// [`Cancel`].
    Cancel(Cancel),
    /// [`HealthQuery`].
    Health(HealthQuery),
}

/// A request in its envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientFrame {
    /// The client's number for this request, echoed on its reply.
    pub request: RequestId,
    /// The request.
    pub message: ClientMessage,
}

/// A frame the host could not take, and the request it answers when the frame said which.
#[derive(Debug, Clone, PartialEq)]
pub struct Rejected {
    /// The request the refusal answers; `None` when the frame was not readable far enough
    /// to say.
    pub reply_to: Option<RequestId>,
    /// Why.
    pub refusal: Refusal,
}

impl Rejected {
    /// The host frame that says so.
    pub fn into_frame(self) -> HostFrame {
        HostFrame {
            reply_to: self.reply_to,
            message: HostMessage::Refused(self.refusal),
        }
    }
}

impl ClientFrame {
    /// Read a frame's body. JSON is parsed once into a value, nested no deeper than the
    /// parser's own bound, so that a frame that is JSON and not a message can still be
    /// answered on its request number.
    pub fn decode(body: &[u8]) -> Result<ClientFrame, Rejected> {
        let value: Value = serde_json::from_slice(body).map_err(|e| Rejected {
            reply_to: None,
            refusal: malformed(format!("the frame is not a JSON message: {e}")),
        })?;
        let reply_to = value.get("request").and_then(Value::as_u64).map(RequestId);
        serde_json::from_value(value).map_err(|e| Rejected {
            reply_to,
            refusal: malformed(format!("the frame is not a message of this protocol: {e}")),
        })
    }

    /// The frame on the wire, or `None` when it is over [`MAX_REQUEST_FRAME_BYTES`] and
    /// the host would refuse it unread: the client reports that to its caller instead of
    /// sending it.
    pub fn encode(&self) -> Option<Vec<u8>> {
        let body = serde_json::to_vec(self).expect("a client frame always serialises");
        frame::encode(&body, MAX_REQUEST_FRAME_BYTES)
    }
}

/// Who answers: the node's stable identity and this run of it, both the mesh's own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostIdentity {
    /// The node.
    pub node: NodeId,
    /// This run of the host. A client that reconnects and meets another instance knows
    /// every session it held is gone.
    pub instance: InstanceId,
}

impl HostIdentity {
    /// The host identity of a node's public identity.
    pub fn of(public: &PublicIdentity) -> Self {
        HostIdentity {
            node: public.node_id.clone(),
            instance: public.instance_id.clone(),
        }
    }
}

/// The bounds a host holds a connection to, sent in [`Welcome`] so a client can keep
/// within them instead of discovering them by refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// The largest request body.
    pub max_request_bytes: u32,
    /// The largest reply body.
    pub max_reply_bytes: u32,
    /// The most requests in flight.
    pub max_in_flight: u32,
    /// The deadline of a call that names none.
    pub default_deadline_ms: u32,
    /// The longest deadline a call may name.
    pub max_deadline_ms: u32,
}

impl Limits {
    /// This build's bounds.
    pub fn this_build() -> Self {
        Limits {
            max_request_bytes: MAX_REQUEST_FRAME_BYTES as u32,
            max_reply_bytes: MAX_REPLY_FRAME_BYTES as u32,
            max_in_flight: MAX_IN_FLIGHT as u32,
            default_deadline_ms: DEFAULT_DEADLINE.as_millis() as u32,
            max_deadline_ms: MAX_DEADLINE_MS,
        }
    }
}

/// The answer to [`Hello`]: the version and features the connection runs with, who
/// answers, and the bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Welcome {
    /// The version chosen.
    pub protocol: ProtocolVersion,
    /// The features both sides have.
    pub features: FeatureSet,
    /// Who answers.
    pub host: HostIdentity,
    /// The bounds.
    pub limits: Limits,
}

/// The answer to [`Attach`]: the session, and the context it is bound to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Attached {
    /// The session, honoured on this connection only.
    pub session: SessionId,
    /// The context, as the host resolved it.
    pub context: ContextDescriptor,
}

/// The answer to a [`Call`] that ran: the capability's output. A call answered with an
/// output ran exactly once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Output {
    /// What the capability returned.
    pub output: Value,
}

/// The answer to [`Detach`] and [`Cancel`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ack {}

/// The answer to [`HealthQuery`]: counts only, so that a health answer costs the host
/// nothing it would have to wait for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HealthReport {
    /// Contexts the host serves.
    pub contexts: u32,
    /// Sessions attached.
    pub sessions: u32,
}

/// Everything a host may send.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HostMessage {
    /// [`Welcome`].
    Welcome(Welcome),
    /// [`Attached`].
    Attached(Attached),
    /// [`Output`].
    Output(Output),
    /// [`Ack`].
    Ack(Ack),
    /// [`HealthReport`].
    Health(HealthReport),
    /// [`Refusal`].
    Refused(Refusal),
}

/// A reply in its envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HostFrame {
    /// The request this answers; absent on a frame the host sends unasked, such as the
    /// refusal of a peer of another user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<RequestId>,
    /// The reply.
    pub message: HostMessage,
}

impl HostFrame {
    /// Read a frame's body, on the client's side.
    pub fn decode(body: &[u8]) -> Result<HostFrame, Refusal> {
        serde_json::from_slice(body)
            .map_err(|e| malformed(format!("the frame is not a host message: {e}")))
    }

    /// The frame on the wire. A reply over [`MAX_REPLY_FRAME_BYTES`] is not sent: the
    /// request is answered instead with [`RefusalCode::OutputOversized`], saying the
    /// capability ran, so that a client never retries a write because its answer was big.
    pub fn encode(&self) -> Vec<u8> {
        let body = serde_json::to_vec(self).expect("a host frame always serialises");
        if let Some(bytes) = frame::encode(&body, MAX_REPLY_FRAME_BYTES) {
            return bytes;
        }
        let refusal = Refusal::new(
            RefusalCode::OutputOversized,
            format!(
                "the answer is {} bytes, over the {MAX_REPLY_FRAME_BYTES}-byte bound; \
                 the capability ran",
                body.len()
            ),
        )
        .with(Delivery::Executed);
        let replacement = HostFrame {
            reply_to: self.reply_to,
            message: HostMessage::Refused(refusal),
        };
        let body = serde_json::to_vec(&replacement).expect("a host frame always serialises");
        frame::encode(&body, MAX_REPLY_FRAME_BYTES).expect("a refusal is small")
    }
}

/// What a host offers every connection.
#[derive(Debug, Clone, PartialEq)]
pub struct HostOffer {
    /// The versions it speaks.
    pub range: ProtocolRange,
    /// The features it has.
    pub features: FeatureSet,
    /// Its bounds.
    pub limits: Limits,
    /// Who it is.
    pub host: HostIdentity,
}

impl HostOffer {
    /// This build's offer.
    pub fn this_build(host: HostIdentity) -> Self {
        HostOffer {
            range: PROTOCOL,
            features: FeatureSet::of(FEATURES).expect("this build's features are names"),
            limits: Limits::this_build(),
            host,
        }
    }
}

/// What one connection runs with.
#[derive(Debug, Clone, PartialEq)]
pub struct Negotiated {
    /// The version.
    pub version: ProtocolVersion,
    /// The features both sides have.
    pub features: FeatureSet,
}

/// Pick the version and the features a connection runs with, or refuse it.
///
/// ```
/// use majordomus_cli::fabric::{negotiate, FeatureSet, Hello, HostIdentity, HostOffer, ProtocolRange, RefusalCode};
/// use majordomus_cli::mesh::identity::NodeIdentity;
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let offer = HostOffer::this_build(HostIdentity::of(&node.public));
/// let hello = |min, max, required: &[&str]| Hello {
///     protocol: ProtocolRange::new(min, max),
///     features: FeatureSet::of(&["cancel", "from-the-future"]).unwrap(),
///     required: FeatureSet::of(required).unwrap(),
///     client: None,
/// };
/// let n = negotiate(&offer, &hello(1, 9, &[])).unwrap();
/// assert_eq!(n.version.0, 1);
/// assert!(n.features.contains("cancel") && !n.features.contains("from-the-future"));
/// assert_eq!(negotiate(&offer, &hello(2, 9, &[])).unwrap_err().code(), RefusalCode::ProtocolUnsupported);
/// assert_eq!(negotiate(&offer, &hello(1, 1, &["subscribe"])).unwrap_err().code(), RefusalCode::FeatureUnsupported);
/// ```
pub fn negotiate(offer: &HostOffer, hello: &Hello) -> Result<Negotiated, Refusal> {
    let Some(version) = hello.protocol.highest_common(&offer.range) else {
        return Err(Refusal::new(
            RefusalCode::ProtocolUnsupported,
            format!(
                "the client speaks versions {} to {} and this host {} to {}",
                hello.protocol.min.0, hello.protocol.max.0, offer.range.min.0, offer.range.max.0
            ),
        ));
    };
    let missing = hello.required.missing_from(&offer.features);
    if !missing.is_empty() {
        return Err(Refusal::new(
            RefusalCode::FeatureUnsupported,
            format!("this host does not offer {}", missing.join(", ")),
        ));
    }
    Ok(Negotiated {
        version,
        features: hello
            .features
            .union(&hello.required)
            .intersection(&offer.features),
    })
}

/// When a call's wait ends, on the host's monotonic clock. Relative to the host's receipt,
/// never a wall-clock time, so two processes need not agree on the time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Deadline {
    at: Instant,
}

impl Deadline {
    /// The deadline of a call received at `received`.
    pub fn of(call: &Call, received: Instant, limits: &Limits) -> Self {
        let ms = call.deadline_ms.unwrap_or(limits.default_deadline_ms);
        Deadline {
            at: received + Duration::from_millis(u64::from(ms)),
        }
    }

    /// How long is left at `now`; zero once it has passed.
    pub fn remaining(&self, now: Instant) -> Duration {
        self.at.saturating_duration_since(now)
    }

    /// Whether it has passed at `now`.
    pub fn passed(&self, now: Instant) -> bool {
        now >= self.at
    }
}

/// What the host holds about one session: the context it is bound to. Kept by the host,
/// keyed by connection, and never read from a frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBinding {
    /// The session.
    pub session: SessionId,
    /// Its context.
    pub context: ContextId,
}

fn session_unknown(session: &SessionId) -> Refusal {
    Refusal::new(
        RefusalCode::SessionUnknown,
        format!("{} is not a session of this connection", session.as_str()),
    )
}

/// The binding of the session a request names, when it is the one the host holds for
/// that session on this connection. `bound` is the host's own lookup: `None` for a session
/// it never minted, has detached or revoked, or minted on another connection.
pub fn admit_session<'b>(
    named: &SessionId,
    bound: Option<&'b SessionBinding>,
) -> Result<&'b SessionBinding, Refusal> {
    match bound {
        Some(binding) if binding.session == *named => Ok(binding),
        _ => Err(session_unknown(named)),
    }
}

impl Call {
    /// Whether this call may run, and the descriptor it runs under: the session must be
    /// this connection's, the context named must be the session's own, and the capability
    /// must be in the registry and executable. Being on the socket is not authority; this
    /// is checked on every call, before the handler, which keeps its own checks.
    pub fn admit<'r>(
        &self,
        bound: Option<&SessionBinding>,
        registry: &'r CapabilityRegistry,
    ) -> Result<&'r Capability, Refusal> {
        let binding = admit_session(&self.session, bound)?;
        if binding.context != self.context {
            return Err(Refusal::new(
                RefusalCode::ContextMismatch,
                format!(
                    "the call names context {} and its session is bound to another; attach \
                     to that context to work in it",
                    self.context.as_str()
                ),
            ));
        }
        let Some(capability) = registry.get(self.capability.as_str()) else {
            return Err(Refusal::new(
                RefusalCode::CapabilityUnknown,
                format!("no capability is called {}", self.capability),
            ));
        };
        // the question an execution asks before it runs anything, asked here the same way
        if let Some(why) = runnable(capability.kind, capability.stability) {
            return Err(Refusal::new(
                RefusalCode::CapabilityDenied,
                format!("{} does not run: {why}", capability.id),
            ));
        }
        Ok(capability)
    }
}

impl ClientMessage {
    /// The checks a message's grammar cannot make: identifiers whose type reads any
    /// string, the deadline's bounds, and the features a message needs.
    fn check(&self, negotiated: &Negotiated, limits: &Limits) -> Result<(), Refusal> {
        match self {
            ClientMessage::Call(call) => {
                CapabilityId::parse(call.capability.as_str()).map_err(|e| {
                    Refusal::new(
                        RefusalCode::InvalidIdentifier,
                        format!("the capability id: {e}"),
                    )
                })?;
                if let Some(key) = &call.idempotency_key {
                    IdempotencyKey::parse(key)
                        .map_err(|e| Refusal::new(RefusalCode::InvalidIdentifier, e))?;
                }
                match call.deadline_ms {
                    Some(ms) if ms == 0 || ms > limits.max_deadline_ms => Err(Refusal::new(
                        RefusalCode::DeadlineInvalid,
                        format!(
                            "a deadline is 1 to {} milliseconds, and this one is {ms}",
                            limits.max_deadline_ms
                        ),
                    )),
                    _ => Ok(()),
                }
            }
            ClientMessage::Cancel(_) if !negotiated.features.contains(FEATURE_CANCEL) => {
                Err(Refusal::new(
                    RefusalCode::FeatureNotNegotiated,
                    "cancel was not negotiated on this connection",
                ))
            }
            _ => Ok(()),
        }
    }
}

/// The rules of one connection, held by the host: `hello` first and once, then every
/// request checked and counted until it is answered. Pure: it reads no socket and runs no
/// capability, so the listener that drives it is free to be whatever it needs to be.
#[derive(Debug, Clone)]
pub struct Conversation {
    offer: HostOffer,
    negotiated: Option<Negotiated>,
    in_flight: BTreeSet<RequestId>,
}

impl Conversation {
    /// A connection that has said nothing yet.
    pub fn new(offer: HostOffer) -> Self {
        Conversation {
            offer,
            negotiated: None,
            in_flight: BTreeSet::new(),
        }
    }

    /// What the connection negotiated, once it has.
    pub fn negotiated(&self) -> Option<&Negotiated> {
        self.negotiated.as_ref()
    }

    /// The requests taken and not yet answered.
    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    /// Take one frame. `Ok(Some(welcome))` answers a `hello`; `Ok(None)` means the request
    /// is admitted to the protocol and in flight until [`Conversation::answered`]; an error
    /// is the refusal to send, and the request is not in flight.
    pub fn receive(&mut self, frame: &ClientFrame) -> Result<Option<Welcome>, Refusal> {
        let negotiated = match (&frame.message, &self.negotiated) {
            (ClientMessage::Hello(hello), None) => {
                let negotiated = negotiate(&self.offer, hello)?;
                let welcome = Welcome {
                    protocol: negotiated.version,
                    features: negotiated.features.clone(),
                    host: self.offer.host.clone(),
                    limits: self.offer.limits,
                };
                self.negotiated = Some(negotiated);
                return Ok(Some(welcome));
            }
            (ClientMessage::Hello(_), Some(_)) => {
                return Err(Refusal::new(
                    RefusalCode::HelloRepeated,
                    "this connection has negotiated already",
                ));
            }
            (_, None) => {
                return Err(Refusal::new(
                    RefusalCode::HelloRequired,
                    "a connection opens with hello",
                ));
            }
            (_, Some(negotiated)) => negotiated,
        };
        frame.message.check(negotiated, &self.offer.limits)?;
        if self.in_flight.contains(&frame.request) {
            return Err(Refusal::new(
                RefusalCode::RequestDuplicate,
                format!("request {} is already in flight", frame.request.0),
            ));
        }
        if self.in_flight.len() >= self.offer.limits.max_in_flight as usize {
            return Err(Refusal::new(
                RefusalCode::TooManyInFlight,
                format!(
                    "{} requests are in flight, the most this connection may have",
                    self.in_flight.len()
                ),
            ));
        }
        self.in_flight.insert(frame.request);
        Ok(None)
    }

    /// The request has been answered; its number may be used again. `false` when it was
    /// not in flight.
    pub fn answered(&mut self, request: RequestId) -> bool {
        self.in_flight.remove(&request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::builtin;
    use crate::mesh::identity::NodeIdentity;
    use proptest::prelude::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex, OnceLock};

    fn host() -> HostIdentity {
        HostIdentity::of(&NodeIdentity::ephemeral().unwrap().public)
    }

    fn offer() -> HostOffer {
        HostOffer::this_build(host())
    }

    fn registry() -> &'static CapabilityRegistry {
        static REGISTRY: OnceLock<CapabilityRegistry> = OnceLock::new();
        REGISTRY.get_or_init(|| {
            CapabilityRegistry::builder()
                .with_builtin(builtin::all())
                .build()
                .unwrap()
        })
    }

    fn frame(request: u64, message: Value) -> ClientFrame {
        ClientFrame::decode(
            &serde_json::to_vec(&json!({ "request": request, "message": message })).unwrap(),
        )
        .unwrap()
    }

    fn hello() -> Value {
        json!({ "type": "hello", "protocol": { "min": 1, "max": 1 }, "features": ["cancel"] })
    }

    fn context(n: u8) -> ContextId {
        ContextId::parse(&format!("{n:02x}").repeat(16)).unwrap()
    }

    fn session(n: u8) -> SessionId {
        SessionId::from_entropy([n; 16])
    }

    fn call(session: &SessionId, context: &ContextId, capability: &str) -> Call {
        Call {
            session: session.clone(),
            context: context.clone(),
            capability: CapabilityId::unchecked(capability),
            input: empty_object(),
            deadline_ms: None,
            idempotency_key: None,
        }
    }

    fn open() -> Conversation {
        let mut conversation = Conversation::new(offer());
        conversation.receive(&frame(0, hello())).unwrap().unwrap();
        conversation
    }

    #[test]
    fn every_message_round_trips_and_reencodes_to_the_same_bytes() {
        let messages = [
            json!({ "type": "hello", "protocol": { "min": 1, "max": 2 }, "features": ["b", "a"],
                    "required": ["a"], "client": "majordomus-mcp" }),
            json!({ "type": "attach", "root": "/src/repo" }),
            json!({ "type": "detach", "session": session(1).as_str() }),
            json!({ "type": "call", "session": session(1).as_str(), "context": context(2).as_str(),
                    "capability": "repository.info", "input": { "z": 1, "a": [true] },
                    "deadline_ms": 5, "idempotency_key": "k-1" }),
            json!({ "type": "cancel", "target": 9 }),
            json!({ "type": "health" }),
        ];
        for message in messages {
            let first = frame(3, message.clone());
            let bytes = first.encode().unwrap();
            let body = frame::read_frame(&mut &bytes[..], MAX_REQUEST_FRAME_BYTES)
                .unwrap()
                .unwrap();
            let second = ClientFrame::decode(&body).unwrap();
            assert_eq!(second, first, "{message}");
            assert_eq!(second.encode().unwrap(), bytes, "deterministic: {message}");
        }
    }

    #[test]
    fn a_missing_optional_field_reads_as_its_default() {
        let hello = frame(
            1,
            json!({ "type": "hello", "protocol": { "min": 1, "max": 1 } }),
        );
        let ClientMessage::Hello(hello) = hello.message else {
            panic!("a hello")
        };
        assert_eq!(hello.features, FeatureSet::default());
        assert!(hello.client.is_none());
        let call = frame(
            2,
            json!({ "type": "call", "session": session(1).as_str(),
                    "context": context(1).as_str(), "capability": "repository.info" }),
        );
        let ClientMessage::Call(call) = call.message else {
            panic!("a call")
        };
        assert_eq!(call.input, json!({}));
        assert!(call.deadline_ms.is_none() && call.idempotency_key.is_none());
    }

    #[test]
    fn a_field_this_protocol_lacks_is_refused_and_answered_on_its_request() {
        let forged = [
            json!({ "request": 4, "message": { "type": "health", "uid": 0 } }),
            json!({ "request": 4, "message": { "type": "call", "session": session(1).as_str(),
                    "context": context(1).as_str(), "capability": "repository.info",
                    "root": "/elsewhere" } }),
            json!({ "request": 4, "uid": 0, "message": { "type": "health" } }),
            json!({ "request": 4, "message": { "type": "subscribe" } }),
            json!({ "request": 4, "message": { "type": "detach", "session": context(1).as_str() } }),
        ];
        for value in forged {
            let rejected = ClientFrame::decode(&serde_json::to_vec(&value).unwrap()).unwrap_err();
            assert_eq!(
                rejected.refusal.code(),
                RefusalCode::MessageMalformed,
                "{value}"
            );
            assert_eq!(rejected.reply_to, Some(RequestId(4)), "{value}");
            assert!(
                !rejected.refusal.terminal(),
                "the stream is in step: {value}"
            );
        }
    }

    #[test]
    fn a_frame_that_is_not_json_or_nests_too_deep_is_refused_without_a_request() {
        for body in [&b"not json"[..], b"", b"{\"request\":", &[0xff, 0xfe][..]] {
            let rejected = ClientFrame::decode(body).unwrap_err();
            assert_eq!(rejected.refusal.code(), RefusalCode::MessageMalformed);
            assert_eq!(rejected.reply_to, None);
        }
        let deep = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000));
        let rejected = ClientFrame::decode(deep.as_bytes()).unwrap_err();
        assert_eq!(rejected.refusal.code(), RefusalCode::MessageMalformed);
        let frame = rejected.into_frame();
        assert!(frame.reply_to.is_none());
        assert!(matches!(frame.message, HostMessage::Refused(_)));
        assert!(!HostFrame::decode(&frame.encode()[frame::HEADER_BYTES..]).is_err());
    }

    #[test]
    fn a_connection_opens_with_hello_once_and_the_welcome_says_what_was_chosen() {
        let mut conversation = Conversation::new(offer());
        let early = conversation
            .receive(&frame(1, json!({ "type": "health" })))
            .unwrap_err();
        assert_eq!(early.code(), RefusalCode::HelloRequired);
        assert!(early.terminal());
        assert!(conversation.negotiated().is_none());

        let welcome = conversation.receive(&frame(2, hello())).unwrap().unwrap();
        assert_eq!(welcome.protocol, ProtocolVersion(1));
        assert!(welcome.features.contains(FEATURE_CANCEL));
        assert_eq!(welcome.limits, Limits::this_build());
        assert_eq!(
            conversation.negotiated().unwrap().version,
            ProtocolVersion(1)
        );
        assert_eq!(conversation.in_flight(), 0, "a hello is answered at once");

        let again = conversation.receive(&frame(3, hello())).unwrap_err();
        assert_eq!(again.code(), RefusalCode::HelloRepeated);
        assert!(!again.terminal());
    }

    #[test]
    fn a_hello_with_no_common_version_or_an_unknown_requirement_ends_the_connection() {
        let mut conversation = Conversation::new(offer());
        let refused = conversation
            .receive(&frame(
                1,
                json!({ "type": "hello", "protocol": { "min": 2, "max": 3 } }),
            ))
            .unwrap_err();
        assert_eq!(refused.code(), RefusalCode::ProtocolUnsupported);
        assert!(
            refused.terminal() && refused.message().contains("2 to 3"),
            "{refused}"
        );
        let refused = Conversation::new(offer())
            .receive(&frame(
                1,
                json!({ "type": "hello", "protocol": { "min": 1, "max": 1 }, "required": ["subscribe"] }),
            ))
            .unwrap_err();
        assert_eq!(refused.code(), RefusalCode::FeatureUnsupported);
        assert!(refused.message().contains("subscribe"));
    }

    #[test]
    fn cancel_needs_the_feature_and_health_needs_only_the_hello() {
        let mut plain = Conversation::new(offer());
        plain
            .receive(&frame(
                1,
                json!({ "type": "hello", "protocol": { "min": 1, "max": 1 } }),
            ))
            .unwrap();
        let refused = plain
            .receive(&frame(2, json!({ "type": "cancel", "target": 1 })))
            .unwrap_err();
        assert_eq!(refused.code(), RefusalCode::FeatureNotNegotiated);
        assert_eq!(
            plain
                .receive(&frame(3, json!({ "type": "health" })))
                .unwrap(),
            None
        );

        let mut cancelling = open();
        assert_eq!(
            cancelling
                .receive(&frame(2, json!({ "type": "cancel", "target": 1 })))
                .unwrap(),
            None
        );
    }

    #[test]
    fn a_request_number_in_flight_is_refused_until_it_is_answered() {
        let mut conversation = open();
        let health = frame(7, json!({ "type": "health" }));
        assert_eq!(conversation.receive(&health).unwrap(), None);
        let duplicate = conversation.receive(&health).unwrap_err();
        assert_eq!(duplicate.code(), RefusalCode::RequestDuplicate);
        assert_eq!(duplicate.delivery(), Delivery::NotExecuted);
        assert!(conversation.answered(RequestId(7)));
        assert!(!conversation.answered(RequestId(7)), "answered once");
        assert_eq!(
            conversation.receive(&health).unwrap(),
            None,
            "the number is free again"
        );
    }

    #[test]
    fn a_connection_holds_at_most_its_bound_in_flight() {
        let mut conversation = open();
        for n in 0..MAX_IN_FLIGHT as u64 {
            conversation
                .receive(&frame(100 + n, json!({ "type": "health" })))
                .unwrap();
        }
        let refused = conversation
            .receive(&frame(1, json!({ "type": "health" })))
            .unwrap_err();
        assert_eq!(refused.code(), RefusalCode::TooManyInFlight);
        conversation.answered(RequestId(100));
        assert!(conversation
            .receive(&frame(1, json!({ "type": "health" })))
            .is_ok());
    }

    #[test]
    fn concurrent_requests_with_one_number_admit_exactly_one() {
        let conversation = Arc::new(Mutex::new(open()));
        let health = frame(42, json!({ "type": "health" }));
        let admitted: usize = (0..16)
            .map(|_| {
                let conversation = Arc::clone(&conversation);
                let health = health.clone();
                std::thread::spawn(move || conversation.lock().unwrap().receive(&health).is_ok())
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| usize::from(t.join().unwrap()))
            .sum();
        assert_eq!(admitted, 1);
        assert_eq!(conversation.lock().unwrap().in_flight(), 1);
    }

    #[test]
    fn a_call_with_a_bad_identifier_or_deadline_is_refused_before_it_is_in_flight() {
        let mut conversation = open();
        let base = |extra: Value| {
            let mut message = json!({ "type": "call", "session": session(1).as_str(),
                "context": context(1).as_str(), "capability": "repository.info" });
            message
                .as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            frame(5, message)
        };
        let code = |c: &mut Conversation, extra: Value| c.receive(&base(extra)).unwrap_err().code();
        assert_eq!(
            code(&mut conversation, json!({ "capability": "Not.valid" })),
            RefusalCode::InvalidIdentifier
        );
        assert_eq!(
            code(&mut conversation, json!({ "idempotency_key": "two words" })),
            RefusalCode::InvalidIdentifier
        );
        assert_eq!(
            code(&mut conversation, json!({ "deadline_ms": 0 })),
            RefusalCode::DeadlineInvalid
        );
        assert_eq!(
            code(
                &mut conversation,
                json!({ "deadline_ms": MAX_DEADLINE_MS + 1 })
            ),
            RefusalCode::DeadlineInvalid
        );
        assert_eq!(conversation.in_flight(), 0);
        let ok = json!({ "deadline_ms": MAX_DEADLINE_MS, "idempotency_key": "retry.1" });
        assert_eq!(conversation.receive(&base(ok)).unwrap(), None);
    }

    #[test]
    fn deadline_arithmetic_is_relative_to_receipt_and_saturates() {
        let limits = Limits::this_build();
        let received = Instant::now();
        let mut named = call(&session(1), &context(1), "repository.info");
        named.deadline_ms = Some(250);
        let deadline = Deadline::of(&named, received, &limits);
        assert_eq!(deadline.remaining(received), Duration::from_millis(250));
        assert_eq!(
            deadline.remaining(received + Duration::from_millis(100)),
            Duration::from_millis(150)
        );
        assert!(!deadline.passed(received + Duration::from_millis(249)));
        assert!(deadline.passed(received + Duration::from_millis(250)));
        assert_eq!(
            deadline.remaining(received + Duration::from_secs(9)),
            Duration::ZERO
        );
        named.deadline_ms = None;
        let default = Deadline::of(&named, received, &limits);
        assert_eq!(default.remaining(received), DEFAULT_DEADLINE);
        assert!(default > deadline);
    }

    #[test]
    fn a_call_runs_only_under_its_own_session_and_context() {
        let a = SessionBinding {
            session: session(1),
            context: context(1),
        };
        let read = call(&session(1), &context(1), "repository.info");
        let capability = read.admit(Some(&a), registry()).unwrap();
        assert_eq!(capability.id.as_str(), "repository.info");

        // S04 case 1: a session of context A naming context B
        let elsewhere = call(&session(1), &context(2), "plan.transition");
        let refused = elsewhere.admit(Some(&a), registry()).unwrap_err();
        assert_eq!(refused.code(), RefusalCode::ContextMismatch);
        assert_eq!(refused.delivery(), Delivery::NotExecuted);
        assert!(
            !refused.message().contains('/'),
            "no root in the refusal: {refused}"
        );

        // S04 case 4: a detached, guessed or other connection's session
        assert_eq!(
            read.admit(None, registry()).unwrap_err().code(),
            RefusalCode::SessionUnknown
        );
        let other = SessionBinding {
            session: session(9),
            context: context(1),
        };
        assert_eq!(
            read.admit(Some(&other), registry()).unwrap_err().code(),
            RefusalCode::SessionUnknown
        );
        assert!(admit_session(&session(1), Some(&a)).is_ok());
    }

    #[test]
    fn a_capability_runs_only_when_the_registry_has_it_executable() {
        let a = SessionBinding {
            session: session(1),
            context: context(1),
        };
        let unknown = call(&session(1), &context(1), "no.such-capability");
        assert_eq!(
            unknown.admit(Some(&a), registry()).unwrap_err().code(),
            RefusalCode::CapabilityUnknown
        );
        // a declarative object is listed in the registry as a resource, and is read, not run
        let repository = crate::synthetic::SyntheticRepository::small().unwrap();
        let with_resources = repository.context().unwrap();
        let resource = with_resources
            .registry
            .iter()
            .find(|c| !c.kind.is_executable())
            .expect("a synthetic layer has rules")
            .id
            .clone();
        let denied = call(&session(1), &context(1), resource.as_str());
        let refused = denied
            .admit(Some(&a), &with_resources.registry)
            .unwrap_err();
        assert_eq!(refused.code(), RefusalCode::CapabilityDenied);
        assert!(refused.message().contains("resource"), "{refused}");
    }

    #[test]
    fn an_answer_over_the_reply_bound_becomes_a_refusal_that_says_it_ran() {
        let big = HostFrame {
            reply_to: Some(RequestId(3)),
            message: HostMessage::Output(Output {
                output: Value::String("x".repeat(MAX_REPLY_FRAME_BYTES)),
            }),
        };
        let bytes = big.encode();
        let body = frame::read_frame(&mut &bytes[..], MAX_REPLY_FRAME_BYTES)
            .unwrap()
            .unwrap();
        let read = HostFrame::decode(&body).unwrap();
        assert_eq!(read.reply_to, Some(RequestId(3)));
        let HostMessage::Refused(refusal) = read.message else {
            panic!("a refusal")
        };
        assert_eq!(refusal.code(), RefusalCode::OutputOversized);
        assert_eq!(refusal.delivery(), Delivery::Executed);
        assert!(!refusal.terminal());
    }

    #[test]
    fn a_request_over_the_request_bound_is_not_framed() {
        let mut huge = call(&session(1), &context(1), "repository.info");
        huge.input = Value::String("x".repeat(MAX_REQUEST_FRAME_BYTES));
        let frame = ClientFrame {
            request: RequestId(1),
            message: ClientMessage::Call(huge),
        };
        assert!(frame.encode().is_none());
    }

    #[test]
    fn every_host_message_round_trips() {
        let descriptor = ContextDescriptor {
            id: context(1),
            root: "/src/repo".into(),
            repository: None,
            linked: false,
        };
        let messages = [
            HostMessage::Welcome(Welcome {
                protocol: ProtocolVersion(1),
                features: FeatureSet::of(FEATURES).unwrap(),
                host: host(),
                limits: Limits::this_build(),
            }),
            HostMessage::Attached(Attached {
                session: session(1),
                context: descriptor,
            }),
            HostMessage::Output(Output {
                output: json!({ "ok": true }),
            }),
            HostMessage::Ack(Ack {}),
            HostMessage::Health(HealthReport {
                contexts: 2,
                sessions: 3,
            }),
            HostMessage::Refused(Refusal::new(RefusalCode::PeerRefused, "another user")),
        ];
        for message in messages {
            let frame = HostFrame {
                reply_to: Some(RequestId(1)),
                message,
            };
            let bytes = frame.encode();
            let back = HostFrame::decode(&bytes[frame::HEADER_BYTES..]).unwrap();
            assert_eq!(back, frame);
        }
        assert_eq!(
            HostFrame::decode(b"{\"message\":{\"type\":\"ack\",\"x\":1}}")
                .unwrap_err()
                .code(),
            RefusalCode::MessageMalformed
        );
    }

    #[test]
    fn the_schemas_compile_and_hold_the_frames_this_module_writes() {
        let client = serde_json::to_value(schemars::schema_for!(ClientFrame)).unwrap();
        let host_schema = serde_json::to_value(schemars::schema_for!(HostFrame)).unwrap();
        let client = jsonschema::validator_for(&client).expect("the client schema compiles");
        let host_validator =
            jsonschema::validator_for(&host_schema).expect("the host schema compiles");
        let call = frame(
            1,
            json!({ "type": "call", "session": session(1).as_str(),
                    "context": context(1).as_str(), "capability": "repository.info" }),
        );
        assert!(client.is_valid(&serde_json::to_value(&call).unwrap()));
        assert!(client.is_valid(&serde_json::to_value(frame(2, hello())).unwrap()));
        assert!(!client.is_valid(&json!({ "request": 1, "message": { "type": "call",
            "session": "nope", "context": context(1).as_str(), "capability": "repository.info" } })));
        let welcome = HostFrame {
            reply_to: Some(RequestId(2)),
            message: HostMessage::Welcome(
                Conversation::new(offer())
                    .receive(&frame(2, hello()))
                    .unwrap()
                    .unwrap(),
            ),
        };
        assert!(host_validator.is_valid(&serde_json::to_value(&welcome).unwrap()));
        let refused = Rejected {
            reply_to: None,
            refusal: Refusal::cancelled(false),
        }
        .into_frame();
        assert!(host_validator.is_valid(&serde_json::to_value(&refused).unwrap()));
    }

    proptest! {
        #[test]
        fn no_byte_string_makes_decoding_panic(body in proptest::collection::vec(any::<u8>(), 0..256)) {
            let _ = ClientFrame::decode(&body);
            let _ = HostFrame::decode(&body);
        }

        #[test]
        fn any_request_number_and_input_round_trips(request in any::<u64>(), text in ".{0,64}", n in any::<i64>()) {
            let mut named = call(&session(1), &context(1), "repository.info");
            named.input = json!({ "text": text, "n": n });
            let frame = ClientFrame { request: RequestId(request), message: ClientMessage::Call(named) };
            let bytes = frame.encode().unwrap();
            prop_assert_eq!(ClientFrame::decode(&bytes[frame::HEADER_BYTES..]).unwrap(), frame);
        }
    }
}
