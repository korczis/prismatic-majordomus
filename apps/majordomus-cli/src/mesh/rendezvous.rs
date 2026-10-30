//! The rendezvous provider: discovery across segments multicast cannot cross. Every
//! Majordomus server is a rendezvous — the `mesh.register` capability accepts a signed
//! envelope and answers with the envelopes it holds — so a "mothership" is any node the
//! declaration points at, not a different program and not a canonical database. The
//! local registry stays the only truth; a rendezvous is one more source of observations,
//! its failure stretches the retry and stops nothing else, and its answers are signed
//! end-to-end: a poisoned rendezvous can withhold nodes, it cannot invent one.
//!
//! ```
//! use majordomus_cli::mesh::rendezvous::RendezvousProvider;
//! use majordomus_cli::mesh::provider::{MeshProvider, MeshProviderState};
//!
//! // Before the manager starts it, a provider is stopped; with no endpoints declared
//! // there is nothing to register with, and starting it says so.
//! let provider = RendezvousProvider::new(vec![], 60);
//! assert_eq!(provider.id(), "rendezvous");
//! assert_eq!(provider.status().state, MeshProviderState::Stopped);
//! ```

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::protocol::Envelope;
use super::provider::{
    jitter_ms, Counters, MeshProvider, MeshProviderState, Observation, ProviderContext,
    ProviderStatus,
};
use super::registry::MeshSource;
use super::MeshError;

/// The HTTP route `mesh.register` is served on. Named once, here: the capability
/// declaration and this client read the same word.
pub const REGISTER_PATH: &str = "/api/v1/mesh/register";

/// One request's bound.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// The most interval multiples backoff stretches to after consecutive failures.
const MAX_BACKOFF_FACTOR: u32 = 8;

/// A stop check happens at least this often while waiting.
const SLICE: Duration = Duration::from_millis(500);

/// How long a registration waits for the whole answer: [`REQUEST_TIMEOUT`] bounds the
/// silence between two reads, this a rendezvous that is never silent and never done.
const ANSWER_WITHIN: Duration = Duration::from_secs(15);

/// The most registry candidates a register answer carries besides the answering node's own
/// envelope: what a rendezvous answers and what a dialer accepts (I2133, I2704).
///
/// ```
/// use majordomus_cli::mesh::rendezvous::{MAX_ANSWER_CANDIDATES, MAX_CANDIDATES};
///
/// assert_eq!(MAX_ANSWER_CANDIDATES, MAX_CANDIDATES + 1, "the answering node leads");
/// ```
pub const MAX_CANDIDATES: usize = 32;

/// The most envelopes a dialer reads from one register answer: the answering node's own
/// and [`MAX_CANDIDATES`]. An answer that carries more is refused at the first envelope
/// over the bound, before that envelope is parsed (I2704).
pub const MAX_ANSWER_CANDIDATES: usize = MAX_CANDIDATES + 1;

/// The largest register answer a dialer reads, as compact JSON: every candidate it accepts
/// at twice the datagram bound an envelope was admitted under, and 4 KiB for the answer's
/// own fields. An answer over it is refused before a byte of it is parsed, rather than
/// read up to the HTTP client's general bound of 32 MiB (I2704).
///
/// ```
/// use majordomus_cli::mesh::protocol::MAX_DATAGRAM;
/// use majordomus_cli::mesh::rendezvous::{MAX_ANSWER_CANDIDATES, MAX_REGISTER_ANSWER};
///
/// assert!(MAX_REGISTER_ANSWER >= MAX_ANSWER_CANDIDATES * MAX_DATAGRAM);
/// assert!(MAX_REGISTER_ANSWER < 128 * 1024, "a bound, not a formality");
/// ```
pub const MAX_REGISTER_ANSWER: usize =
    MAX_ANSWER_CANDIDATES * 2 * super::protocol::MAX_DATAGRAM + 4096;

/// What a registration answers: whether the envelope was accepted, and the candidates
/// the rendezvous holds. Defined here beside the client; the capability re-exports it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct RegisterAnswer {
    /// Whether the presented envelope was accepted into the local registry.
    pub accepted: bool,
    /// Why not, when it was not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
    /// The signed envelopes of the nodes this rendezvous currently holds, the answering
    /// node's own included. Each verifies on its own signature; the rendezvous vouches
    /// for nothing.
    pub candidates: Vec<Envelope>,
}

/// The rendezvous provider: the declared endpoints, one registration thread each,
/// and the counters their status reports. Constructed by the manager from the declaration.
pub struct RendezvousProvider {
    endpoints: Vec<String>,
    interval: Duration,
    counters: Arc<Counters>,
    state: Arc<Mutex<(MeshProviderState, Option<String>)>>,
}

impl RendezvousProvider {
    /// A provider over its endpoints; nothing is contacted until the manager calls
    /// `start`, and each endpoint then gets its own bounded, backed-off loop.
    pub fn new(endpoints: Vec<String>, interval_seconds: u64) -> Self {
        RendezvousProvider {
            endpoints,
            interval: Duration::from_secs(interval_seconds),
            counters: Arc::new(Counters::default()),
            state: Arc::new(Mutex::new((MeshProviderState::Stopped, None))),
        }
    }
}

impl MeshProvider for RendezvousProvider {
    fn id(&self) -> &'static str {
        "rendezvous"
    }

    fn start(&mut self, ctx: &ProviderContext) -> Result<(), MeshError> {
        if self.endpoints.is_empty() {
            let reason = "no endpoints declared".to_string();
            *self.state.lock().expect("rendezvous state") =
                (MeshProviderState::Stopped, Some(reason.clone()));
            return Err(MeshError::Provider(reason));
        }
        *self.state.lock().expect("rendezvous state") = (
            MeshProviderState::Running,
            Some(format!("{} endpoint(s)", self.endpoints.len())),
        );
        // One thread per endpoint: endpoints are independent sources, and one that
        // hangs must not delay another. Each is bounded by REQUEST_TIMEOUT anyway.
        for endpoint in self.endpoints.clone() {
            let tx = ctx.tx.clone();
            let stop = Arc::clone(&ctx.stop);
            let beacon = Arc::clone(&ctx.beacon);
            let counters = Arc::clone(&self.counters);
            let interval = self.interval;
            let _ = std::thread::Builder::new()
                .name("majordomus-mesh-rendezvous".into())
                .spawn(move || {
                    let mut failures: u32 = 0;
                    while !stop.load(Ordering::SeqCst) {
                        match register(&endpoint, &beacon.next_envelope()) {
                            Ok(answer) => {
                                failures = 0;
                                counters.sent.fetch_add(1, Ordering::Relaxed);
                                for candidate in answer.candidates {
                                    if let Ok(bytes) = serde_json::to_vec(&candidate) {
                                        counters.received.fetch_add(1, Ordering::Relaxed);
                                        let _ = tx.send(Observation {
                                            source: MeshSource::Rendezvous,
                                            path: endpoint.clone(),
                                            bytes,
                                        });
                                    }
                                }
                            }
                            Err(_) => {
                                failures = (failures + 1).min(MAX_BACKOFF_FACTOR.ilog2());
                            }
                        }
                        // interval × 2^failures, capped at ×MAX_BACKOFF_FACTOR: an
                        // unreachable rendezvous is asked less and less, never hammered,
                        // never a tight loop.
                        let total =
                            interval * (1u32 << failures) + Duration::from_millis(jitter_ms(2000));
                        let mut slept = Duration::ZERO;
                        while slept < total && !stop.load(Ordering::SeqCst) {
                            let slice = SLICE.min(total - slept);
                            std::thread::sleep(slice);
                            slept += slice;
                        }
                    }
                });
        }
        Ok(())
    }

    fn status(&self) -> ProviderStatus {
        let (state, detail) = self.state.lock().expect("rendezvous state").clone();
        ProviderStatus {
            id: self.id().into(),
            state,
            detail,
            sent: self.counters.sent.load(Ordering::Relaxed),
            received: self.counters.received.load(Ordering::Relaxed),
        }
    }
}

/// Why one registration round-trip produced no answer to act on. Each is one failure of
/// the endpoint, which the provider backs off from; the variants say which bound or which
/// step a rendezvous broke, so that a test — or a person reading a log — can tell a hub
/// that is down from one that answers more than it may.
///
/// ```
/// use majordomus_cli::mesh::rendezvous::{RegisterError, MAX_ANSWER_CANDIDATES};
///
/// let error = RegisterError::TooManyCandidates {
///     endpoint: "http://10.0.0.2:8741".into(),
///     bound: MAX_ANSWER_CANDIDATES,
/// };
/// assert!(error.to_string().contains("10.0.0.2:8741"), "an error names its endpoint");
/// assert!(error.to_string().contains("33"), "{error}");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegisterError {
    /// No reply: unreachable, dropped, or silent past the request bound.
    #[error("{endpoint}: {reason}")]
    Transport {
        /// The endpoint asked.
        endpoint: String,
        /// What happened.
        reason: String,
    },
    /// A reply over [`MAX_REGISTER_ANSWER`], or one that did not arrive whole in time:
    /// refused before any of it was parsed.
    #[error("{endpoint}: {refused}")]
    Refused {
        /// The endpoint asked.
        endpoint: String,
        /// Which bound the reply broke.
        refused: crate::mcp::bridge::ReplyRefused,
    },
    /// A status other than 200.
    #[error("{endpoint}: status {status}")]
    Status {
        /// The endpoint asked.
        endpoint: String,
        /// The status it answered.
        status: u16,
    },
    /// An answer carrying more than [`MAX_ANSWER_CANDIDATES`] envelopes, refused at the
    /// first one over the bound.
    #[error("{endpoint}: the answer carries more than {bound} candidates and was not read")]
    TooManyCandidates {
        /// The endpoint asked.
        endpoint: String,
        /// The bound it went over.
        bound: usize,
    },
    /// A body that is not a register answer.
    #[error("{endpoint}: not a register answer: {reason}")]
    Malformed {
        /// The endpoint asked.
        endpoint: String,
        /// What the parser said.
        reason: String,
    },
}

/// One registration round-trip: present `envelope` to the rendezvous at `endpoint` and read
/// its answer, through the same minimal client the MCP bridge uses: over TLS to an `https://`
/// hub, in cleartext only to a hub whose address is not public (ADR 0220), with the
/// reply held at most at [`MAX_REGISTER_ANSWER`] and its candidates at
/// [`MAX_ANSWER_CANDIDATES`]: a rendezvous is somebody else's process, and what it answers
/// costs this one a bounded buffer and a bounded wait whatever it sends (I2704).
///
/// ```
/// use majordomus_cli::mesh::identity::NodeIdentity;
/// use majordomus_cli::mesh::protocol::advertise;
/// use majordomus_cli::mesh::rendezvous::{register, RegisterError};
///
/// let node = NodeIdentity::ephemeral().unwrap();
/// let envelope = advertise(&node, 1, &[], &[], &[], "doc");
/// // port 9 discards: nothing answers there
/// let error = register("http://127.0.0.1:9", &envelope).unwrap_err();
/// assert!(matches!(error, RegisterError::Transport { .. }), "{error}");
/// ```
pub fn register(endpoint: &str, envelope: &Envelope) -> Result<RegisterAnswer, RegisterError> {
    let body = serde_json::json!({ "envelope": envelope }).to_string();
    let reply = crate::mcp::bridge::request_json_within(
        endpoint,
        "POST",
        REGISTER_PATH,
        &[("Content-Type", "application/json")],
        Some(&body),
        crate::mcp::bridge::ReplyBound {
            bytes: MAX_REGISTER_ANSWER,
            idle: REQUEST_TIMEOUT,
            within: ANSWER_WITHIN,
        },
    )
    .map_err(|e| match crate::mcp::bridge::ReplyRefused::of(&e) {
        Some(refused) => RegisterError::Refused {
            endpoint: endpoint.to_string(),
            refused: refused.clone(),
        },
        None => RegisterError::Transport {
            endpoint: endpoint.to_string(),
            reason: e.to_string(),
        },
    })?;
    if reply.status != 200 {
        return Err(RegisterError::Status {
            endpoint: endpoint.to_string(),
            status: reply.status,
        });
    }
    let wire: WireAnswer = serde_json::from_str(&reply.body).map_err(|e| {
        if e.to_string().contains(TOO_MANY_CANDIDATES) {
            RegisterError::TooManyCandidates {
                endpoint: endpoint.to_string(),
                bound: MAX_ANSWER_CANDIDATES,
            }
        } else {
            RegisterError::Malformed {
                endpoint: endpoint.to_string(),
                reason: e.to_string(),
            }
        }
    })?;
    Ok(RegisterAnswer {
        accepted: wire.accepted,
        refusal: wire.refusal,
        candidates: wire.candidates,
    })
}

/// A register answer as a dialer reads it: [`RegisterAnswer`], with its candidates counted
/// as they are read.
#[derive(serde::Deserialize)]
struct WireAnswer {
    accepted: bool,
    #[serde(default)]
    refusal: Option<String>,
    #[serde(deserialize_with = "at_most_answer_candidates")]
    candidates: Vec<Envelope>,
}

/// The parser's word for an answer over [`MAX_ANSWER_CANDIDATES`], by which [`register`]
/// tells it from a malformed one.
const TOO_MANY_CANDIDATES: &str = "more candidates than a register answer carries";

/// Read at most [`MAX_ANSWER_CANDIDATES`] envelopes; at one more, stop before parsing it.
fn at_most_answer_candidates<'de, D>(deserializer: D) -> Result<Vec<Envelope>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Bounded;
    impl<'de> serde::de::Visitor<'de> for Bounded {
        type Value = Vec<Envelope>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "at most {MAX_ANSWER_CANDIDATES} candidate envelopes")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut candidates = Vec::new();
            while candidates.len() < MAX_ANSWER_CANDIDATES {
                match seq.next_element::<Envelope>()? {
                    Some(envelope) => candidates.push(envelope),
                    None => return Ok(candidates),
                }
            }
            // one more is the bound broken: it is skipped as JSON, never built
            match seq.next_element::<serde::de::IgnoredAny>()? {
                None => Ok(candidates),
                Some(_) => Err(serde::de::Error::custom(TOO_MANY_CANDIDATES)),
            }
        }
    }
    deserializer.deserialize_seq(Bounded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_endpoints_means_a_stopped_provider_with_the_reason() {
        let mut provider = RendezvousProvider::new(vec![], 60);
        let (tx, _rx) = crate::mesh::provider::ObservationSender::bounded(64);
        let ctx = crate::mesh::provider::ProviderContext {
            tx,
            stop: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            beacon: std::sync::Arc::new(crate::mesh::provider::Beacon::new(
                std::sync::Arc::new(crate::mesh::identity::NodeIdentity::ephemeral().unwrap()),
                vec![],
                vec![],
                vec![],
                "test",
            )),
        };
        assert!(provider.start(&ctx).is_err());
        let status = provider.status();
        assert_eq!(status.state, MeshProviderState::Stopped);
        assert_eq!(status.detail.as_deref(), Some("no endpoints declared"));
    }

    #[test]
    fn an_unreachable_endpoint_is_one_failure_not_a_panic() {
        let identity = crate::mesh::identity::NodeIdentity::ephemeral().unwrap();
        let envelope = crate::mesh::protocol::advertise(&identity, 1, &[], &[], &[], "test");
        // A port nothing listens on: connection refused, reported as the endpoint's error.
        let error = register("http://127.0.0.1:9", &envelope)
            .unwrap_err()
            .to_string();
        assert!(error.contains("127.0.0.1:9"), "{error}");
    }

    #[test]
    fn a_reachable_endpoint_yields_registrations_and_candidate_observations() {
        use std::io::{Read, Write};
        // A canned rendezvous: one thread, plain HTTP/1.1, answering every POST with
        // an accepted registration carrying one signed candidate.
        let candidate_identity = crate::mesh::identity::NodeIdentity::ephemeral().unwrap();
        let candidate = crate::mesh::protocol::advertise(
            &candidate_identity,
            1,
            &["127.0.0.1:3".into()],
            &[],
            &[],
            "canned",
        );
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let body = serde_json::to_string(&RegisterAnswer {
            accepted: true,
            refusal: None,
            candidates: vec![candidate.clone()],
        })
        .unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut buffer = [0u8; 4096];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                );
            }
        });

        let mut provider = RendezvousProvider::new(vec![endpoint.clone()], 1);
        let (tx, rx) = crate::mesh::provider::ObservationSender::bounded(64);
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ctx = crate::mesh::provider::ProviderContext {
            tx,
            stop: std::sync::Arc::clone(&stop),
            beacon: std::sync::Arc::new(crate::mesh::provider::Beacon::new(
                std::sync::Arc::new(crate::mesh::identity::NodeIdentity::ephemeral().unwrap()),
                vec![],
                vec![],
                vec![],
                "test",
            )),
        };
        provider.start(&ctx).unwrap();
        let heard = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("a candidate observation arrives");
        assert_eq!(heard.source, MeshSource::Rendezvous);
        assert_eq!(heard.path, endpoint);
        let parsed = crate::mesh::protocol::parse(&heard.bytes).expect("the candidate verifies");
        assert_eq!(
            parsed.adv.node_id(),
            Some(candidate_identity.public.node_id.clone())
        );
        assert!(provider.status().sent >= 1);
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// A hub declared in cleartext at a public address is never sent the envelope: the
    /// registration fails at once, naming the address and the correction.
    #[test]
    fn a_public_cleartext_hub_is_refused_before_the_envelope_is_sent() {
        let identity = crate::mesh::identity::NodeIdentity::ephemeral().unwrap();
        let envelope = crate::mesh::protocol::advertise(&identity, 1, &[], &[], &[], "test");
        let error = register("http://203.0.113.5:8791", &envelope)
            .unwrap_err()
            .to_string();
        assert!(error.contains("203.0.113.5 is a public address"), "{error}");
        assert!(error.contains("use https://203.0.113.5:8791"), "{error}");
    }

    #[test]
    fn a_register_answer_round_trips_as_json() {
        let answer = RegisterAnswer {
            accepted: true,
            refusal: None,
            candidates: vec![],
        };
        let text = serde_json::to_string(&answer).unwrap();
        let back: RegisterAnswer = serde_json::from_str(&text).unwrap();
        assert!(back.accepted && back.candidates.is_empty());
    }

    /// A canned rendezvous answering every POST with `status` and `body`; its endpoint.
    fn canned(status: u16, body: String) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut buffer = [0u8; 8192];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                );
            }
        });
        endpoint
    }

    fn an_envelope() -> Envelope {
        let identity = crate::mesh::identity::NodeIdentity::ephemeral().unwrap();
        crate::mesh::protocol::advertise(&identity, 1, &["127.0.0.1:3".into()], &[], &[], "t")
    }

    fn answer_of(candidates: usize) -> String {
        serde_json::to_string(&RegisterAnswer {
            accepted: true,
            refusal: None,
            candidates: vec![an_envelope(); candidates],
        })
        .unwrap()
    }

    #[test]
    fn a_status_other_than_200_is_a_status_error_naming_it() {
        let endpoint = canned(503, String::new());
        let error = register(&endpoint, &an_envelope()).unwrap_err();
        match error {
            RegisterError::Status {
                endpoint: e,
                status,
            } => {
                assert_eq!(e, endpoint);
                assert_eq!(status, 503);
            }
            other => panic!("expected a status error, got {other}"),
        }
    }

    #[test]
    fn an_answer_over_the_candidate_bound_is_refused_as_too_many() {
        let endpoint = canned(200, answer_of(MAX_ANSWER_CANDIDATES + 1));
        let error = register(&endpoint, &an_envelope()).unwrap_err();
        match error {
            RegisterError::TooManyCandidates { endpoint: e, bound } => {
                assert_eq!(e, endpoint);
                assert_eq!(bound, MAX_ANSWER_CANDIDATES);
            }
            other => panic!("expected too many candidates, got {other}"),
        }
    }

    #[test]
    fn candidates_that_are_not_a_list_say_what_was_expected() {
        let err = serde_json::from_str::<WireAnswer>(r#"{"accepted": true, "candidates": 3}"#)
            .err()
            .expect("a number is not a list of candidates");
        let expected = format!("at most {MAX_ANSWER_CANDIDATES} candidate envelopes");
        assert!(err.to_string().contains(&expected), "{err}");
    }

    #[test]
    fn a_candidate_that_is_not_an_envelope_is_refused_where_it_is_read() {
        let err = serde_json::from_str::<WireAnswer>(r#"{"accepted": true, "candidates": [3]}"#)
            .err()
            .expect("a number is not an envelope");
        assert!(!err.to_string().contains(TOO_MANY_CANDIDATES), "{err}");
    }

    #[test]
    fn a_broken_element_past_the_bound_is_malformed_not_too_many() {
        let full = serde_json::to_string(&vec![an_envelope(); MAX_ANSWER_CANDIDATES]).unwrap();
        let body = format!(
            r#"{{"accepted": true, "candidates": {}, nul]}}"#,
            full.strip_suffix(']').unwrap()
        );
        let err = serde_json::from_str::<WireAnswer>(&body)
            .err()
            .expect("one more element that is not JSON");
        assert!(!err.to_string().contains(TOO_MANY_CANDIDATES), "{err}");
    }

    #[test]
    fn an_answer_at_the_candidate_bound_is_read_whole() {
        let endpoint = canned(200, answer_of(MAX_ANSWER_CANDIDATES));
        let answer = register(&endpoint, &an_envelope()).expect("at the bound is within it");
        assert!(answer.accepted);
        assert_eq!(answer.candidates.len(), MAX_ANSWER_CANDIDATES);
    }

    #[test]
    fn a_body_that_is_not_an_answer_is_malformed_not_too_many() {
        let endpoint = canned(200, "{\"accepted\": 3}".into());
        let error = register(&endpoint, &an_envelope()).unwrap_err();
        match error {
            RegisterError::Malformed {
                endpoint: e,
                reason,
            } => {
                assert_eq!(e, endpoint);
                assert!(!reason.is_empty());
            }
            other => panic!("expected a malformed error, got {other}"),
        }
    }
}
