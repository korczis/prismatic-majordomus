//! The discovery manager: provider lifecycle, the one verification path, and the one
//! registry. Providers hand raw bytes here; this thread parses, checks bounds and
//! staleness, verifies the signature, evaluates trust, and only then touches the
//! registry — in that order, for every transport alike. A provider cannot skip a step
//! because no provider ever parses.
//!
//! The runtime lives on the capability [`Context`](crate::capability::Context) from
//! process start, inactive, and a shared server activates it when the repository's mesh
//! declaration says so. Activation is idempotent, bounded, and never blocks startup:
//! sockets open on their own threads, and a provider that cannot start is a `Failed`
//! status, not a failed server.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::broadcast::BroadcastProvider;
use super::config::{BroadcastMode, MeshConfig, TrustConfig};
use super::identity::{NodeIdentity, PublicIdentity};
use super::multicast::MulticastProvider;
use super::protocol::{self, Refusal};
use super::provider::{Beacon, MeshProvider, Observation, ProviderContext, ProviderStatus};
use super::registry::{MeshRegistry, MeshSource, NodeRecord, Tallies};
use super::rendezvous::{RegisterAnswer, RendezvousProvider};
use super::trust;
use super::MeshError;

/// How often the manager thread checks the stop flag while nothing arrives.
const RECV_TIMEOUT: Duration = Duration::from_millis(500);

/// The most candidates one register answer carries.
const MAX_CANDIDATES: usize = 32;

/// Refused datagrams, by reason. The diagnosis of a hostile or misconfigured network:
/// a mesh that hears garbage says so, with numbers, instead of being quietly empty.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, JsonSchema)]
pub struct Refusals {
    /// Longer than the datagram bound.
    pub oversized: u64,
    /// Not an envelope.
    pub malformed: u64,
    /// A protocol version this executable does not read.
    pub version: u64,
    /// Out-of-bounds fields.
    pub bounds: u64,
    /// Outside the clock-skew window.
    pub stale: u64,
    /// A signature that does not verify.
    pub signature: u64,
    /// This node's own datagrams, heard back and skipped.
    pub self_heard: u64,
}

#[derive(Default)]
struct RefusalCounters {
    oversized: AtomicU64,
    malformed: AtomicU64,
    version: AtomicU64,
    bounds: AtomicU64,
    stale: AtomicU64,
    signature: AtomicU64,
    self_heard: AtomicU64,
}

impl RefusalCounters {
    fn count(&self, refusal: &Refusal) {
        let counter = match refusal {
            Refusal::Oversized => &self.oversized,
            Refusal::Malformed => &self.malformed,
            Refusal::Version => &self.version,
            Refusal::Bounds => &self.bounds,
            Refusal::Stale => &self.stale,
            Refusal::Signature => &self.signature,
        };
        counter.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> Refusals {
        Refusals {
            oversized: self.oversized.load(Ordering::Relaxed),
            malformed: self.malformed.load(Ordering::Relaxed),
            version: self.version.load(Ordering::Relaxed),
            bounds: self.bounds.load(Ordering::Relaxed),
            stale: self.stale.load(Ordering::Relaxed),
            signature: self.signature.load(Ordering::Relaxed),
            self_heard: self.self_heard.load(Ordering::Relaxed),
        }
    }
}

/// The whole mesh, as every surface reports it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MeshStatus {
    /// Whether the mesh is running in this process.
    pub active: bool,
    /// Why not, when it is not: no declaration, disabled, no identity, stopped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// This node, when the mesh has an identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<PublicIdentity>,
    /// The trust policy in force.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_policy: Option<String>,
    /// Every provider, canonical order (by id).
    pub providers: Vec<ProviderStatus>,
    /// The registry's tallies.
    pub tallies: Tallies,
    /// Refused datagrams, by reason.
    pub refusals: Refusals,
    /// When the mesh started, RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
}

struct Active {
    beacon: Arc<Beacon>,
    tx: Sender<Observation>,
    stop: Arc<AtomicBool>,
    providers: Vec<Box<dyn MeshProvider>>,
    trust: TrustConfig,
    started_at: String,
}

/// The runtime: the registry (always there, so a surface can answer before activation)
/// and the active machinery once a declaration switched it on.
pub struct MeshRuntime {
    registry: Arc<MeshRegistry>,
    refusals: Arc<RefusalCounters>,
    state: Mutex<Option<Active>>,
    /// Why the mesh is not active, when it is not.
    reason: Mutex<String>,
    /// Whether a shared server decided anything about this runtime — activated it or
    /// declined with a reason. A runtime nobody decided on is the command line's, and
    /// its inactivity is not a verdict the doctor may report.
    decided: AtomicBool,
}

impl Default for MeshRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshRuntime {
    /// An inactive runtime: an empty registry and the reason "nothing activated it".
    pub fn new() -> Self {
        MeshRuntime {
            registry: Arc::new(MeshRegistry::new()),
            refusals: Arc::new(RefusalCounters::default()),
            state: Mutex::new(None),
            reason: Mutex::new(
                "not active: no shared server activated the mesh in this process".into(),
            ),
            decided: AtomicBool::new(false),
        }
    }

    /// Whether a shared server activated or declined this runtime. `false` in every
    /// process that is not a server: there, `status()` reports an absence, not a decision.
    pub fn decided(&self) -> bool {
        self.decided.load(Ordering::SeqCst)
    }

    /// The registry behind this runtime, for surfaces that project it directly.
    pub fn registry(&self) -> &Arc<MeshRegistry> {
        &self.registry
    }

    /// Record why the mesh is not running, without activating anything: a disabled or
    /// malformed declaration is an answer `mesh.status` must be able to give.
    pub fn decline(&self, reason: &str) {
        let state = self.state.lock().expect("mesh state");
        if state.is_none() {
            *self.reason.lock().expect("mesh reason") = format!("not active: {reason}");
        }
        self.decided.store(true, Ordering::SeqCst);
    }

    /// Activate the mesh from a declaration. Idempotent: a second activation of an
    /// active runtime is a no-op. A disabled declaration records the reason and starts
    /// nothing. Never blocks: every socket lives on its own thread.
    pub fn activate(
        &self,
        config: &MeshConfig,
        identity: NodeIdentity,
        endpoints: Vec<String>,
        repos: Vec<String>,
        version: &str,
    ) -> Result<(), MeshError> {
        self.decided.store(true, Ordering::SeqCst);
        let mut state = self.state.lock().expect("mesh state");
        if state.is_some() {
            return Ok(());
        }
        if !config.enabled {
            *self.reason.lock().expect("mesh reason") =
                "not active: the mesh declaration is disabled".into();
            return Ok(());
        }
        let beacon = Arc::new(Beacon::new(
            Arc::new(identity),
            endpoints,
            vec!["http".into(), "mcp".into(), "ws".into()],
            repos,
            version,
        ));
        let (tx, rx) = channel::<Observation>();
        let stop = Arc::new(AtomicBool::new(false));
        let ctx = ProviderContext {
            tx: tx.clone(),
            stop: Arc::clone(&stop),
            beacon: Arc::clone(&beacon),
        };

        // The providers the declaration asks for. Each failure is that provider's
        // status; the loop never stops on one.
        let mut providers: Vec<Box<dyn MeshProvider>> = Vec::new();
        if config.multicast.enabled {
            providers.push(Box::new(MulticastProvider::new(config.multicast.clone())));
        }
        if config.broadcast.mode != BroadcastMode::Disabled {
            providers.push(Box::new(BroadcastProvider::new(
                config.broadcast.clone(),
                !config.multicast.enabled,
            )));
        }
        if !config.rendezvous.endpoints.is_empty() {
            providers.push(Box::new(RendezvousProvider::new(
                config.rendezvous.endpoints.clone(),
                config.rendezvous.interval_seconds,
            )));
        }
        for provider in providers.iter_mut() {
            if let Err(e) = provider.start(&ctx) {
                tracing::warn!(provider = provider.id(), error = %e, "mesh provider did not start; the others run on");
            }
        }

        // The manager thread: the one verification path.
        {
            let registry = Arc::clone(&self.registry);
            let refusals = Arc::clone(&self.refusals);
            let stop = Arc::clone(&stop);
            let own = beacon.identity().public.node_id.clone();
            let trust = config.trust.clone();
            let _ = std::thread::Builder::new()
                .name("majordomus-mesh".into())
                .spawn(move || {
                    while !stop.load(Ordering::SeqCst) {
                        let observation = match rx.recv_timeout(RECV_TIMEOUT) {
                            Ok(o) => o,
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                        };
                        ingest(&registry, &refusals, &trust, &own, &observation);
                    }
                });
        }

        *state = Some(Active {
            beacon,
            tx,
            stop,
            providers,
            trust: config.trust.clone(),
            started_at: crate::peers::rfc3339(std::time::SystemTime::now()),
        });
        Ok(())
    }

    /// Attach one more provider to an active mesh. This is the entire registration a
    /// new discovery mechanism needs: no surface, consumer or registry changes with it.
    pub fn attach(&self, mut provider: Box<dyn MeshProvider>) -> Result<(), MeshError> {
        let mut state = self.state.lock().expect("mesh state");
        let Some(active) = state.as_mut() else {
            return Err(MeshError::Provider("the mesh is not active".into()));
        };
        let ctx = ProviderContext {
            tx: active.tx.clone(),
            stop: Arc::clone(&active.stop),
            beacon: Arc::clone(&active.beacon),
        };
        let outcome = provider.start(&ctx);
        active.providers.push(provider);
        outcome
    }

    /// Stop the mesh. Every provider thread and the manager end at their next bounded
    /// wait; the registry keeps what it saw for whoever still asks.
    pub fn stop(&self) {
        let mut state = self.state.lock().expect("mesh state");
        if let Some(active) = state.take() {
            active.stop.store(true, Ordering::SeqCst);
            *self.reason.lock().expect("mesh reason") = "not active: stopped".into();
        }
    }

    /// The status, as every surface answers it.
    pub fn status(&self) -> MeshStatus {
        let state = self.state.lock().expect("mesh state");
        let mut status = match state.as_ref() {
            Some(active) => {
                let mut providers: Vec<ProviderStatus> =
                    active.providers.iter().map(|p| p.status()).collect();
                crate::order::canonical(&mut providers);
                MeshStatus {
                    active: true,
                    reason: None,
                    identity: Some(active.beacon.identity().public.clone()),
                    trust_policy: Some(active.trust.policy.as_str().into()),
                    providers,
                    tallies: Tallies::default(),
                    refusals: Refusals::default(),
                    started_at: Some(active.started_at.clone()),
                }
            }
            None => MeshStatus {
                active: false,
                reason: Some(self.reason.lock().expect("mesh reason").clone()),
                identity: None,
                trust_policy: None,
                providers: Vec::new(),
                tallies: Tallies::default(),
                refusals: Refusals::default(),
                started_at: None,
            },
        };
        status.tallies = self.registry.tallies();
        status.refusals = self.refusals.snapshot();
        status
    }

    /// Every observed node, in the registry's canonical node-id order.
    pub fn nodes(&self) -> Vec<NodeRecord> {
        self.registry.list()
    }

    /// Handle one rendezvous registration: verify the presented envelope exactly as a
    /// datagram, record it, and answer with this node's envelope and the candidates the
    /// registry holds. Registration grants nothing — the caller becomes a record, not a
    /// peer with rights.
    pub fn register(&self, envelope: &serde_json::Value, path: &str) -> RegisterAnswer {
        let state = self.state.lock().expect("mesh state");
        let Some(active) = state.as_ref() else {
            return RegisterAnswer {
                accepted: false,
                refusal: Some("the mesh is not active on this node".into()),
                candidates: Vec::new(),
            };
        };
        let own = active.beacon.identity().public.node_id.clone();
        let trust = active.trust.clone();
        let own_envelope = active.beacon.next_envelope();
        drop(state);

        let bytes = serde_json::to_vec(envelope).unwrap_or_default();
        let refusal = match ingest(
            &self.registry,
            &self.refusals,
            &trust,
            &own,
            &Observation {
                source: MeshSource::Rendezvous,
                path: path.into(),
                bytes,
            },
        ) {
            Ingest::Accepted => None,
            Ingest::Own => Some("that is this node's own envelope".into()),
            Ingest::Replay => Some("replayed".into()),
            Ingest::Refused(r) => Some(r.to_string()),
        };
        let mut candidates = Vec::with_capacity(MAX_CANDIDATES + 1);
        if let Ok(own_value) = serde_json::to_value(&own_envelope) {
            candidates.push(own_value);
        }
        candidates.extend(self.registry.candidates(MAX_CANDIDATES));
        let candidates = candidates
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect();
        RegisterAnswer {
            accepted: refusal.is_none(),
            refusal,
            candidates,
        }
    }
}

enum Ingest {
    Accepted,
    Own,
    Replay,
    Refused(Refusal),
}

/// The one verification path: bounds, shape, version, staleness, signature, identity,
/// trust, registry — in that order, for every provider alike.
fn ingest(
    registry: &MeshRegistry,
    refusals: &RefusalCounters,
    trust: &TrustConfig,
    own: &super::identity::NodeId,
    observation: &Observation,
) -> Ingest {
    let envelope = match protocol::parse(&observation.bytes) {
        Ok(e) => e,
        Err(refusal) => {
            refusals.count(&refusal);
            return Ingest::Refused(refusal);
        }
    };
    let Some(node_id) = envelope.adv.node_id() else {
        // Unreachable after a verified parse, but hostile input earns no unwrap.
        refusals.count(&Refusal::Signature);
        return Ingest::Refused(Refusal::Signature);
    };
    if node_id == *own {
        refusals.self_heard.fetch_add(1, Ordering::Relaxed);
        return Ingest::Own;
    }
    let known_key = registry.known_key(&node_id);
    let previous = registry.trust_of(&node_id);
    let verdict = trust::evaluate(
        &trust.policy,
        &trust.allow,
        &envelope.adv.pk,
        known_key.as_deref(),
        previous.as_ref(),
    );
    let raw = serde_json::to_value(&envelope).unwrap_or(serde_json::Value::Null);
    if registry.observe(
        &envelope.adv,
        node_id,
        verdict,
        observation.source,
        &observation.path,
        raw,
    ) {
        Ingest::Accepted
    } else {
        Ingest::Replay
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::config::{MeshConfig, MulticastConfig};
    use crate::mesh::identity::NodeIdentity;
    use crate::mesh::protocol::advertise;

    fn identity_in(dir: &tempfile::TempDir, name: &str) -> NodeIdentity {
        NodeIdentity::load_or_create(&dir.path().join(name)).unwrap()
    }

    fn quiet_config() -> MeshConfig {
        // Enabled, but with every transport off: what unit tests want.
        MeshConfig {
            schema: "mesh/v1".into(),
            kind: "mesh-declaration".into(),
            id: "test".into(),
            enabled: true,
            multicast: MulticastConfig {
                enabled: false,
                ..MulticastConfig::default()
            },
            broadcast: Default::default(),
            rendezvous: Default::default(),
            trust: Default::default(),
        }
    }

    /// The synthetic provider of the zero-registration test: implemented here, in a
    /// test, through the public contract alone. It proves a new mechanism needs no
    /// change to the registry, the manager's verification, or any surface.
    struct SyntheticProvider {
        datagrams: Vec<Vec<u8>>,
    }

    impl MeshProvider for SyntheticProvider {
        fn id(&self) -> &'static str {
            "synthetic"
        }
        fn start(&mut self, ctx: &ProviderContext) -> Result<(), MeshError> {
            for bytes in self.datagrams.drain(..) {
                let _ = ctx.tx.send(Observation {
                    source: MeshSource::Synthetic,
                    path: "test".into(),
                    bytes,
                });
            }
            Ok(())
        }
        fn status(&self) -> ProviderStatus {
            ProviderStatus {
                id: "synthetic".into(),
                state: crate::mesh::provider::MeshProviderState::Running,
                detail: None,
                sent: 0,
                received: 0,
            }
        }
    }

    fn wait_for<F: Fn() -> bool>(what: &str, check: F) {
        for _ in 0..100 {
            if check() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("timed out waiting for {what}");
    }

    #[test]
    fn a_synthetic_provider_reaches_the_registry_with_zero_registration() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = MeshRuntime::new();
        runtime
            .activate(
                &quiet_config(),
                identity_in(&dir, "self.json"),
                vec![],
                vec![],
                "0.5.0",
            )
            .unwrap();
        let other = identity_in(&dir, "other.json");
        let envelope = advertise(
            &other,
            1,
            &["127.0.0.1:1".into()],
            &["http".into()],
            &[],
            "0.5.0",
        );
        let bytes = serde_json::to_vec(&envelope).unwrap();
        runtime
            .attach(Box::new(SyntheticProvider {
                datagrams: vec![bytes],
            }))
            .unwrap();
        wait_for("the observation to land", || runtime.nodes().len() == 1);
        let nodes = runtime.nodes();
        assert_eq!(nodes[0].node_id, other.public.node_id);
        assert!(
            !nodes[0].trust.is_trusted(),
            "deny_unknown observes, never trusts"
        );
        runtime.stop();
    }

    #[test]
    fn own_datagrams_are_skipped_not_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = MeshRuntime::new();
        let own = identity_in(&dir, "self.json");
        let own_envelope = advertise(&own, 1, &[], &[], &[], "0.5.0");
        runtime
            .activate(&quiet_config(), own, vec![], vec![], "0.5.0")
            .unwrap();
        let bytes = serde_json::to_vec(&own_envelope).unwrap();
        runtime
            .attach(Box::new(SyntheticProvider {
                datagrams: vec![bytes],
            }))
            .unwrap();
        wait_for("the self-heard counter", || {
            runtime.status().refusals.self_heard == 1
        });
        assert!(runtime.nodes().is_empty());
        runtime.stop();
    }

    #[test]
    fn garbage_from_a_provider_is_counted_and_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = MeshRuntime::new();
        runtime
            .activate(
                &quiet_config(),
                identity_in(&dir, "self.json"),
                vec![],
                vec![],
                "0.5.0",
            )
            .unwrap();
        runtime
            .attach(Box::new(SyntheticProvider {
                datagrams: vec![b"garbage".to_vec(), vec![0xff; 2000]],
            }))
            .unwrap();
        wait_for("both refusals", || {
            let r = runtime.status().refusals;
            r.malformed == 1 && r.oversized == 1
        });
        assert!(runtime.nodes().is_empty());
        runtime.stop();
    }

    #[test]
    fn registration_records_and_answers_with_own_envelope() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = MeshRuntime::new();
        let own = identity_in(&dir, "self.json");
        let own_node = own.public.node_id.clone();
        runtime
            .activate(&quiet_config(), own, vec![], vec![], "0.5.0")
            .unwrap();
        let caller = identity_in(&dir, "caller.json");
        let envelope = advertise(&caller, 1, &["127.0.0.1:2".into()], &[], &[], "0.5.0");
        let answer = runtime.register(&serde_json::to_value(&envelope).unwrap(), "test");
        assert!(answer.accepted, "{:?}", answer.refusal);
        assert_eq!(
            answer.candidates[0].adv.node_id(),
            Some(own_node),
            "the answering node's own envelope leads the candidates"
        );
        assert_eq!(runtime.nodes().len(), 1);
        runtime.stop();
    }

    #[test]
    fn registration_refuses_garbage_and_an_inactive_mesh_says_so() {
        let inactive = MeshRuntime::new();
        let answer = inactive.register(&serde_json::json!({"v": 1}), "test");
        assert!(!answer.accepted);
        let dir = tempfile::tempdir().unwrap();
        let runtime = MeshRuntime::new();
        runtime
            .activate(
                &quiet_config(),
                identity_in(&dir, "self.json"),
                vec![],
                vec![],
                "0.5.0",
            )
            .unwrap();
        let answer = runtime.register(&serde_json::json!({"v": 1}), "test");
        assert!(!answer.accepted);
        assert_eq!(answer.refusal.as_deref(), Some("malformed"));
        runtime.stop();
    }

    #[test]
    fn a_disabled_declaration_activates_nothing_and_says_why() {
        let runtime = MeshRuntime::new();
        let dir = tempfile::tempdir().unwrap();
        let mut config = quiet_config();
        config.enabled = false;
        runtime
            .activate(
                &config,
                identity_in(&dir, "self.json"),
                vec![],
                vec![],
                "0.5.0",
            )
            .unwrap();
        let status = runtime.status();
        assert!(!status.active);
        assert!(status.reason.unwrap().contains("disabled"));
    }

    #[test]
    fn activation_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = MeshRuntime::new();
        let config = quiet_config();
        runtime
            .activate(
                &config,
                identity_in(&dir, "a.json"),
                vec![],
                vec![],
                "0.5.0",
            )
            .unwrap();
        runtime
            .activate(
                &config,
                identity_in(&dir, "b.json"),
                vec![],
                vec![],
                "0.5.0",
            )
            .unwrap();
        let status = runtime.status();
        assert!(status.active);
        runtime.stop();
    }
}
