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
                        match register_once(&endpoint, &beacon.next_envelope()) {
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

/// One registration round-trip. Plain HTTP through the same minimal client the MCP
/// bridge uses; any non-200, timeout or unparsable body is one failure.
fn register_once(endpoint: &str, envelope: &Envelope) -> Result<RegisterAnswer, MeshError> {
    let body = serde_json::json!({ "envelope": envelope }).to_string();
    let reply = crate::mcp::bridge::request(
        endpoint,
        "POST",
        REGISTER_PATH,
        &[("Content-Type", "application/json")],
        Some(&body),
        REQUEST_TIMEOUT,
    )
    .map_err(|e| MeshError::Provider(format!("{endpoint}: {e}")))?;
    if reply.status != 200 {
        return Err(MeshError::Provider(format!(
            "{endpoint}: status {}",
            reply.status
        )));
    }
    serde_json::from_str(&reply.body)
        .map_err(|e| MeshError::Provider(format!("{endpoint}: not a register answer: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_endpoints_means_a_stopped_provider_with_the_reason() {
        let mut provider = RendezvousProvider::new(vec![], 60);
        let (tx, _rx) = std::sync::mpsc::channel();
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
        let error = register_once("http://127.0.0.1:9", &envelope)
            .unwrap_err()
            .to_string();
        assert!(error.contains("127.0.0.1:9"), "{error}");
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
}
