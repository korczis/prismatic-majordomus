//! What a runtime believes at a restart, and how far ahead of it an event may say it is
//! (issue I2124).
//!
//! A journal file is input, the same as a sync round: a runtime restarted after a key was
//! withdrawn from `trust.allow` must not bring that key's sessions, claims and handovers
//! back from its own disk, because the live path would refuse every one of them the moment
//! they arrived over a link. And a Lamport stamp is a claim its writer makes about itself:
//! a trusted key that stamps an event near `u64::MAX` would otherwise drag every clock in
//! the mesh to the end of its range in one round.
//!
//! The runtimes are the library's own `Cooperation`, linked through the real signed link
//! protocol over an in-process transport, with a journal file and a node identity file
//! under a temporary `XDG_STATE_HOME`, so a restart is a new process run of the same node:
//! the same key, a new instance, the same journal on disk.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use majordomus_cli::mesh::config::{CooperationConfig, TrustConfig};
use majordomus_cli::mesh::cooperation::{CheckoutFacts, Cooperation, CooperationSetup};
use majordomus_cli::mesh::identity::NodeIdentity;
use majordomus_cli::mesh::journal::{
    ClaimMode, EventBody, Journal, MeshEvent, Rejection, SessionInfo, MAX_LAMPORT_LEAD,
};
use majordomus_cli::mesh::link::{LinkReply, LinkTransport, Signed, HELLO_PATH, SYNC_PATH};
use majordomus_cli::mesh::repository::{of_root_commits, runtime_id};
use majordomus_cli::mesh::{MeshRegistry, TrustPolicy};

/// Runtimes in one process, linked through this transport: the protocol is the real one;
/// only the socket is replaced.
#[derive(Default)]
struct InProcess {
    runtimes: Mutex<BTreeMap<String, Weak<Cooperation>>>,
}

impl LinkTransport for InProcess {
    fn post(&self, endpoint: &str, path: &str, message: &Signed) -> Result<LinkReply, String> {
        let target = self
            .runtimes
            .lock()
            .unwrap()
            .get(endpoint)
            .and_then(Weak::upgrade)
            .ok_or_else(|| format!("{endpoint}: connection refused"))?;
        match path {
            HELLO_PATH => Ok(target.accept_hello(message)),
            SYNC_PATH => Ok(target.accept_sync(message)),
            other => Err(format!("no route {other}")),
        }
    }
}

/// The state directory of this test process: `XDG_STATE_HOME` points into a temporary
/// directory for the whole run, set once so that tests running in parallel never see it
/// change under them. Each test then works in its own subdirectory of it.
fn state_home() -> &'static Path {
    static HOME: OnceLock<tempfile::TempDir> = OnceLock::new();
    HOME.get_or_init(|| {
        let dir = tempfile::tempdir().expect("a temporary state directory");
        std::env::set_var("XDG_STATE_HOME", dir.path());
        dir
    })
    .path()
}

/// A node's identity, loaded from its file under the state directory: every load is the
/// same key under a fresh instance, which is what a restarted process is.
fn node(test: &str, name: &str) -> Arc<NodeIdentity> {
    let path = state_home()
        .join(test)
        .join(name)
        .join("majordomus")
        .join("node.json");
    Arc::new(NodeIdentity::load_or_create(&path).expect("a node identity"))
}

fn runtime(
    net: &Arc<InProcess>,
    endpoint: &str,
    identity: Arc<NodeIdentity>,
    allow: Vec<String>,
    journal: Option<PathBuf>,
) -> Arc<Cooperation> {
    let cooperation = Cooperation::new(CooperationSetup {
        identity,
        runtime: runtime_id(endpoint),
        repository: of_root_commits(&["root".into()]),
        endpoints: vec![endpoint.into()],
        version: "test".into(),
        config: CooperationConfig {
            heartbeat_seconds: 1,
            expiry_seconds: 30,
            ..CooperationConfig::default()
        },
        trust: TrustConfig {
            policy: TrustPolicy::DenyUnknown,
            allow,
        },
        journal_path: journal,
        registry: Arc::new(MeshRegistry::new()),
        transport: Arc::clone(net) as Arc<dyn LinkTransport>,
        board: None,
        checkout: CheckoutFacts::default(),
    })
    .unwrap();
    net.runtimes
        .lock()
        .unwrap()
        .insert(endpoint.into(), Arc::downgrade(&cooperation));
    cooperation
}

fn claims_of(c: &Cooperation, node_id: &str) -> usize {
    c.state()
        .claims
        .iter()
        .filter(|claim| claim.stream.node() == node_id)
        .count()
}

fn events_of(c: &Cooperation, node_id: &str) -> usize {
    c.journal()
        .events()
        .iter()
        .filter(|e| e.stream.node() == node_id)
        .count()
}

#[test]
fn a_key_withdrawn_from_the_allowlist_is_not_believed_from_the_journal_file() {
    let test = "withdrawn";
    let journal = state_home().join(test).join("desk").join("journal.jsonl");
    let net = Arc::new(InProcess::default());
    let laptop_id = node(test, "laptop");
    let desk_id = node(test, "desk");
    let laptop_key = laptop_id.public.public_key.clone();
    let laptop_node = laptop_id.public.node_id.as_str().to_string();
    let desk_node = desk_id.public.node_id.as_str().to_string();

    // Each trusts the other by its key, and the laptop's claim reaches the desk's disk.
    let laptop = runtime(
        &net,
        "laptop:1",
        laptop_id,
        vec![desk_id.public.public_key.clone()],
        None,
    );
    let desk = runtime(
        &net,
        "desk:1",
        desk_id,
        vec![laptop_key.clone()],
        Some(journal.clone()),
    );
    let desk_key = laptop.dial(&["desk:1".into()]).expect("the desk welcomes");
    laptop
        .claim(
            &SessionInfo::named("s1", "test"),
            vec!["apps".into()],
            None,
            ClaimMode::Exclusive,
            None,
        )
        .unwrap();
    laptop.journal().beat_own();
    laptop.sync_with(&desk_key).expect("a sync round");
    assert_eq!(
        claims_of(&desk, &laptop_node),
        1,
        "the desk holds the claim"
    );
    // and the desk has work of its own, on the same disk
    desk.claim(
        &SessionInfo::named("d1", "test"),
        vec!["docs".into()],
        None,
        ClaimMode::Exclusive,
        None,
    )
    .unwrap();
    let laptop_events = events_of(&desk, &laptop_node);
    assert!(laptop_events >= 2, "a session and its claim");
    drop(desk);

    // The desk restarts with the laptop's key withdrawn from `trust.allow`: nothing the
    // laptop signed comes back from the file, and the desk's own work does.
    let restarted = runtime(
        &net,
        "desk:1",
        node(test, "desk"),
        vec![],
        Some(journal.clone()),
    );
    assert_eq!(
        events_of(&restarted, &laptop_node),
        0,
        "the withdrawn key's events came back from the journal file"
    );
    assert_eq!(
        restarted
            .journal()
            .reload_report()
            .rejected
            .get(&Rejection::Untrusted),
        Some(&(laptop_events as u64)),
        "every event refused at the reload is reported, by reason"
    );
    assert_eq!(
        claims_of(&restarted, &laptop_node),
        0,
        "the withdrawn key's claim holds again after the restart"
    );
    assert_eq!(
        claims_of(&restarted, &desk_node),
        1,
        "this machine's own claim is its own, whatever the allowlist says"
    );
    drop(restarted);

    // With the key allowed again, the same file gives the claim back: the refusal above
    // was the policy, not a file that did not reload.
    let allowed = runtime(
        &net,
        "desk:1",
        node(test, "desk"),
        vec![laptop_key],
        Some(journal),
    );
    assert_eq!(events_of(&allowed, &laptop_node), laptop_events);
    assert_eq!(claims_of(&allowed, &laptop_node), 1);
    assert_eq!(allowed.journal().reload_report().rejected_total(), 0);
}

/// A trusted key's event stamped far ahead of this runtime's clock.
fn far_ahead(identity: &NodeIdentity, journal: &Journal, lamport: u64) -> MeshEvent {
    let mut event = journal
        .append_own(EventBody::SessionClosed {
            session: "s2".into(),
        })
        .unwrap();
    event.lamport = lamport;
    event.sig = identity.sign(&event.signing_bytes());
    event
}

#[test]
fn a_lamport_stamp_far_ahead_of_the_receiver_is_refused() {
    let writer = Arc::new(NodeIdentity::ephemeral().unwrap());
    let theirs =
        Journal::open(Arc::clone(&writer), "0000000000000001", "repo".into(), None).unwrap();
    let ours = Journal::open(
        Arc::new(NodeIdentity::ephemeral().unwrap()),
        "0000000000000002",
        "repo".into(),
        None,
    )
    .unwrap();
    let first = theirs
        .append_own(EventBody::SessionClosed {
            session: "s1".into(),
        })
        .unwrap();
    let forged = far_ahead(&writer, &theirs, u64::MAX - 1);

    let report = ours.ingest(&[first, forged], &|_| Ok(()));
    assert_eq!(report.accepted, 1, "the first event is an ordinary one");
    assert_eq!(report.rejected_total(), 1, "{report:?}");
    assert_eq!(report.rejected[&Rejection::ClockAhead], 1, "{report:?}");
    assert!(
        ours.tallies().lamport < 1 << 20,
        "one trusted event moved this runtime's clock to {}",
        ours.tallies().lamport
    );
    assert_eq!(ours.events().len(), 1);

    // The bound is relative to the receiver's clock, so a mesh with a long history is
    // heard by a runtime that has just joined it: a stamp at the bound is an ordinary one.
    let at_the_bound = far_ahead(&writer, &theirs, ours.tallies().lamport + MAX_LAMPORT_LEAD);
    let another = Journal::open(
        Arc::new(NodeIdentity::ephemeral().unwrap()),
        "0000000000000003",
        "repo".into(),
        None,
    )
    .unwrap();
    let mut events = theirs.events();
    events.retain(|e| e.seq < at_the_bound.seq);
    events.push(at_the_bound);
    let report = another.ingest(&events, &|_| Ok(()));
    assert_eq!(report.rejected_total(), 0, "{report:?}");
    assert_eq!(report.accepted, 3, "{report:?}");
}
