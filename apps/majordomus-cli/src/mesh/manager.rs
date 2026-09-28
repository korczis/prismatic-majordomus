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
use super::cooperation::{Cooperation, CooperationStatus};
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
    /// The cooperation runtime, once a server attached one.
    cooperation: Mutex<Option<Arc<Cooperation>>>,
    /// Why cooperation is not running, when the mesh is but cooperation is not.
    cooperation_reason: Mutex<Option<String>>,
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
            cooperation: Mutex::new(None),
            cooperation_reason: Mutex::new(None),
        }
    }

    /// Attach the cooperation runtime a server built for this mesh. The runtime is
    /// already started; a second attachment replaces nothing.
    ///
    /// Discovery and cooperation are built separately — one finds runtimes, the other
    /// links to them — and this is where the surfaces find both behind one handle. The
    /// first attachment wins, so a second server in one process cannot quietly take over
    /// the journal the first one is already writing to.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use majordomus_cli::mesh::config::{CooperationConfig, TrustConfig};
    /// # use majordomus_cli::mesh::cooperation::{CheckoutFacts, Cooperation, CooperationSetup};
    /// # use majordomus_cli::mesh::identity::NodeIdentity;
    /// # use majordomus_cli::mesh::link::HttpTransport;
    /// # use majordomus_cli::mesh::registry::MeshRegistry;
    /// # use majordomus_cli::mesh::repository::of_root_commits;
    /// # let built = |slot: &str| Cooperation::new(CooperationSetup {
    /// #     identity: Arc::new(NodeIdentity::ephemeral().unwrap()), runtime: slot.into(),
    /// #     repository: of_root_commits(&["root".into()]), endpoints: vec!["127.0.0.1:9".into()],
    /// #     version: "doc".into(), config: CooperationConfig::default(), trust: TrustConfig::default(),
    /// #     journal_path: None, registry: Arc::new(MeshRegistry::new()),
    /// #     transport: Arc::new(HttpTransport), board: None, checkout: CheckoutFacts::default(),
    /// # }).unwrap();
    /// use majordomus_cli::mesh::manager::MeshRuntime;
    ///
    /// let mesh = MeshRuntime::new();
    /// let first = built("0000000000000001");
    /// mesh.attach_cooperation(Arc::clone(&first));
    /// mesh.attach_cooperation(built("0000000000000002"));
    ///
    /// let attached = mesh.cooperation().expect("a cooperation runtime");
    /// assert_eq!(attached.runtime_key(), first.runtime_key(), "the first one keeps the slot");
    /// ```
    pub fn attach_cooperation(&self, cooperation: Arc<Cooperation>) {
        let mut slot = self.cooperation.lock().expect("mesh cooperation");
        if slot.is_none() {
            *slot = Some(cooperation);
        }
    }

    /// Record why cooperation is not running while the mesh is.
    ///
    /// A mesh that discovers runtimes and links to none is the confusing case: everything
    /// looks switched on and nothing is shared. The server that decided not to start
    /// cooperation says so here, so the answer reaches whoever asks rather than living in
    /// the log of a process they cannot see.
    ///
    /// ```
    /// use majordomus_cli::mesh::manager::MeshRuntime;
    ///
    /// let mesh = MeshRuntime::new();
    /// mesh.decline_cooperation("no endpoint to be dialed at");
    ///
    /// let status = mesh.cooperation_status();
    /// assert!(!status.active);
    /// assert_eq!(status.reason.as_deref(), Some("no endpoint to be dialed at"));
    /// ```
    pub fn decline_cooperation(&self, reason: &str) {
        *self
            .cooperation_reason
            .lock()
            .expect("mesh cooperation reason") = Some(reason.into());
    }

    /// The cooperation runtime, while one is still running. A stopped runtime is not
    /// handed out: a caller that got it would write to a journal nobody is replicating and
    /// believe it had told the mesh something. `None` here means the same to every caller
    /// — do this locally, or say it cannot be done — however cooperation came to be absent.
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use majordomus_cli::mesh::config::{CooperationConfig, TrustConfig};
    /// # use majordomus_cli::mesh::cooperation::{CheckoutFacts, Cooperation, CooperationSetup};
    /// # use majordomus_cli::mesh::identity::NodeIdentity;
    /// # use majordomus_cli::mesh::link::HttpTransport;
    /// # use majordomus_cli::mesh::registry::MeshRegistry;
    /// # use majordomus_cli::mesh::repository::of_root_commits;
    /// # let cooperation = Cooperation::new(CooperationSetup {
    /// #     identity: Arc::new(NodeIdentity::ephemeral().unwrap()), runtime: "0000000000000001".into(),
    /// #     repository: of_root_commits(&["root".into()]), endpoints: vec!["127.0.0.1:9".into()],
    /// #     version: "doc".into(), config: CooperationConfig::default(), trust: TrustConfig::default(),
    /// #     journal_path: None, registry: Arc::new(MeshRegistry::new()),
    /// #     transport: Arc::new(HttpTransport), board: None, checkout: CheckoutFacts::default(),
    /// # }).unwrap();
    /// use majordomus_cli::mesh::manager::MeshRuntime;
    ///
    /// let mesh = MeshRuntime::new();
    /// assert!(mesh.cooperation().is_none(), "nothing has attached one");
    ///
    /// mesh.attach_cooperation(Arc::clone(&cooperation));
    /// assert!(mesh.cooperation().is_some());
    ///
    /// cooperation.stop();
    /// assert!(mesh.cooperation().is_none(), "a stopped runtime is not handed out");
    /// ```
    pub fn cooperation(&self) -> Option<Arc<Cooperation>> {
        self.cooperation
            .lock()
            .expect("mesh cooperation")
            .as_ref()
            .filter(|c| !c.stopped())
            .cloned()
    }

    /// Cooperation's status, or why there is none: the mesh's own reason when the mesh is
    /// off, cooperation's reason when only cooperation is.
    ///
    /// The two reasons are kept apart because they call for different acts. A mesh that was
    /// never switched on is a declaration to write; cooperation declined under a running
    /// mesh is something about this server — no endpoint, no trusted key — and the more
    /// specific reason is the one worth telling a person, so it wins when both exist.
    ///
    /// ```
    /// use majordomus_cli::mesh::manager::MeshRuntime;
    ///
    /// let mesh = MeshRuntime::new();
    /// let untouched = mesh.cooperation_status();
    /// assert!(untouched.reason.unwrap().contains("no shared server"), "the mesh's reason");
    ///
    /// mesh.decline_cooperation("the declaration enables the mesh but not cooperation");
    /// let declined = mesh.cooperation_status();
    /// assert!(declined.reason.unwrap().contains("not cooperation"), "the nearer reason wins");
    /// assert!(declined.peers.is_empty(), "and the shape is the one a live runtime answers");
    /// ```
    pub fn cooperation_status(&self) -> CooperationStatus {
        if let Some(c) = self.cooperation() {
            return c.status();
        }
        let reason = self
            .cooperation_reason
            .lock()
            .expect("mesh cooperation reason")
            .clone()
            .unwrap_or_else(|| self.reason.lock().expect("mesh reason").clone());
        CooperationStatus::inactive(&reason)
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
        self.activate_as(config, Arc::new(identity), "", endpoints, repos, version)
    }

    /// [`MeshRuntime::activate`] for one runtime slot of the node: what a shared server
    /// does, naming its checkout's runtime so that two servers of one machine announce,
    /// hear and link to each other as the two runtimes they are.
    ///
    /// Without the slot, two worktrees of one repository on one machine would announce
    /// under one identity and each would take the other's announcements for its own echo.
    /// The slot is what makes them two peers that cooperate, which is the ordinary case on
    /// a developer's machine and not an exotic one.
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use majordomus_cli::mesh::config::MeshConfig;
    /// use majordomus_cli::mesh::identity::NodeIdentity;
    /// use majordomus_cli::mesh::manager::MeshRuntime;
    ///
    /// // a declaration with every discovery mechanism off: nothing opens a socket
    /// let config: MeshConfig = serde_json::from_value(serde_json::json!({
    ///     "schema": "mesh/v1", "kind": "mesh", "id": "majordomus", "enabled": true,
    ///     "multicast": { "enabled": false },
    /// })).unwrap();
    ///
    /// let mesh = MeshRuntime::new();
    /// let identity = Arc::new(NodeIdentity::ephemeral().unwrap());
    /// mesh.activate_as(&config, Arc::clone(&identity), "0000000000000001",
    ///     vec!["127.0.0.1:8742".into()], vec!["root".into()], "doc").unwrap();
    ///
    /// let status = mesh.status();
    /// assert!(status.active);
    /// assert_eq!(status.identity.unwrap().node_id, identity.public.node_id);
    ///
    /// // idempotent: the second server of this process does not restart the first's mesh
    /// mesh.activate_as(&config, Arc::clone(&identity), "0000000000000002",
    ///     vec!["127.0.0.1:8743".into()], vec!["root".into()], "doc").unwrap();
    /// assert_eq!(mesh.status().started_at, status.started_at);
    /// ```
    pub fn activate_as(
        &self,
        config: &MeshConfig,
        identity: Arc<NodeIdentity>,
        runtime: &str,
        endpoints: Vec<String>,
        repos: Vec<String>,
        version: &str,
    ) -> Result<(), MeshError> {
        let mut state = self.state.lock().expect("mesh state");
        if state.is_some() {
            return Ok(());
        }
        if !config.enabled {
            *self.reason.lock().expect("mesh reason") =
                "not active: the mesh declaration is disabled".into();
            return Ok(());
        }
        let beacon = Arc::new(
            Beacon::new(
                identity,
                endpoints,
                vec!["http".into(), "mcp".into(), "ws".into()],
                repos,
                version,
            )
            .with_runtime(runtime),
        );
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
            let own = (
                beacon.identity().public.node_id.clone(),
                beacon.runtime().to_string(),
            );
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

    /// Tell the mesh to stop, without waiting for anything: every loop and handler reads the
    /// flag at once, and the draining that needs locks is left to [`stop`](Self::stop). A
    /// process that is shutting down calls this first, does the work that must not be
    /// delayed — closing its listeners, releasing its lease — and only then drains.
    ///
    /// ```
    /// use majordomus_cli::mesh::manager::MeshRuntime;
    ///
    /// // a mesh that never activated has nothing to tell, and says so rather than failing:
    /// // a shutdown path must not care whether the thing it is stopping ever started
    /// let mesh = MeshRuntime::new();
    /// mesh.begin_stop();
    /// assert!(!mesh.status().active);
    /// ```
    pub fn begin_stop(&self) {
        if let Some(cooperation) = self.cooperation.lock().expect("mesh cooperation").as_ref() {
            cooperation.begin_stop();
        }
        if let Some(active) = self.state.lock().expect("mesh state").as_ref() {
            active.stop.store(true, Ordering::SeqCst);
        }
    }

    /// Stop the mesh. Every provider thread and the manager end at their next bounded
    /// wait; the registry keeps what it saw for whoever still asks.
    pub fn stop(&self) {
        if let Some(cooperation) = self.cooperation.lock().expect("mesh cooperation").as_ref() {
            cooperation.stop();
        }
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
        let own = (
            active.beacon.identity().public.node_id.clone(),
            active.beacon.runtime().to_string(),
        );
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
    own: &(super::identity::NodeId, String),
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
    // Only this very runtime is "own": another server of the same machine carries the
    // same key under another runtime slot, and is a runtime to hear, not an echo.
    if node_id == own.0 && envelope.adv.rt == own.1 {
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
            cooperation: Default::default(),
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
