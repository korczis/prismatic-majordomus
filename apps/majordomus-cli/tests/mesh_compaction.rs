//! Compaction keeps what is live, end to end: runtimes of one repository linked through the
//! real signed link protocol — hello, sync rounds, marks, beats — over an in-process
//! transport, so that the expiry is the only thing waited for. docs/CLAIMS.yaml's
//! `mesh-compaction-keeps-what-is-live` names test/cases/493_mesh_compaction_keeps_what_is_live.sh,
//! which runs this file; the claim's id is written here so the link reads from both ends.
//!
//! No black-box case reaches compaction in bounded time: a running server compacts every
//! sixty heartbeats and keeps a dead stream for fifteen minutes (`PEER_RETENTION`), and of
//! the two, only the heartbeat is declarable. So the runtimes here are the library's own
//! `Cooperation`, and the compaction is the journal's own, called with the arguments the
//! supervisor passes except the retention, which is zero. The journal's unit tests and its
//! property test (src/mesh/journal.rs) hold the same semantics in depth.
//!
//! - A runtime compacted away while it slept is heard again when it wakes, whole: the claim
//!   it took before it slept holds again on its peer, and refuses a conflicting claim there.
//! - Who took a handover and who answered a review stay with the handover and the review
//!   when the taker has stopped and the publisher has not.
//! - A handover somebody took keeps its stopped publisher nowhere; one nobody took keeps it
//!   for the handover retention and no longer.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use majordomus_cli::mesh::config::{CooperationConfig, TrustConfig};
use majordomus_cli::mesh::cooperation::{
    CheckoutFacts, Cooperation, CooperationError, CooperationSetup, RoundError,
};
use majordomus_cli::mesh::identity::NodeIdentity;
use majordomus_cli::mesh::journal::{ClaimMode, HandoverBody, SessionInfo, HANDOVER_RETENTION};
use majordomus_cli::mesh::link::{
    LinkReply, LinkTransport, RefusalCode, Signed, HELLO_PATH, SYNC_PATH,
};
use majordomus_cli::mesh::repository::{of_root_commits, runtime_id};
use majordomus_cli::mesh::state::{ClaimState, ReviewState};
use majordomus_cli::mesh::{MeshRegistry, TrustPolicy};

/// The declared expiry of every runtime here: three heartbeats of one second.
const EXPIRY: Duration = Duration::from_secs(3);

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

fn runtime(net: &Arc<InProcess>, endpoint: &str) -> Arc<Cooperation> {
    let cooperation = Cooperation::new(CooperationSetup {
        identity: Arc::new(NodeIdentity::ephemeral().unwrap()),
        runtime: runtime_id(endpoint),
        repository: of_root_commits(&["root".into()]),
        endpoints: vec![endpoint.into()],
        version: "test".into(),
        config: CooperationConfig {
            heartbeat_seconds: 1,
            expiry_seconds: EXPIRY.as_secs(),
            ..CooperationConfig::default()
        },
        trust: TrustConfig {
            policy: TrustPolicy::Tofu,
            allow: vec![],
        },
        journal_path: None,
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

/// One heartbeat and one sync round of `from` with the peer at `endpoint`, as the
/// supervisor runs them; a peer that no longer knows the link is greeted again.
fn round(from: &Cooperation, endpoint: &str, key: &str) {
    from.journal().beat_own();
    match from.sync_with(key) {
        Ok(()) => {}
        Err(RoundError::Refused(refusal)) if refusal.code == RefusalCode::UnknownLink => {
            let again = from.dial(&[endpoint.into()]).expect("a hello again");
            from.sync_with(&again).expect("a round after the hello");
        }
        Err(e) => panic!("a sync round failed: {e}"),
    }
}

/// Wait out the expiry: every runtime that did not beat meanwhile is dead to the others.
fn sleep_past_the_expiry() {
    std::thread::sleep(EXPIRY + Duration::from_millis(300));
}

fn handover(text: &str) -> HandoverBody {
    let body = format!("# Objective\n{text}\n");
    HandoverBody {
        id: HandoverBody::digest_of(&body),
        task: None,
        issue: None,
        milestone: None,
        branch: None,
        head: None,
        created_at: None,
        name: None,
        body,
    }
}

fn held(c: &Cooperation) -> usize {
    c.state()
        .claims
        .iter()
        .filter(|claim| claim.state == ClaimState::Held)
        .count()
}

#[test]
fn a_runtime_compacted_while_it_slept_is_heard_again_whole() {
    let net = Arc::new(InProcess::default());
    let laptop = runtime(&net, "laptop:1");
    let desk = runtime(&net, "desk:1");
    let desk_key = laptop
        .dial(&["desk:1".into()])
        .expect("the desk welcomes the laptop");
    let s1 = SessionInfo::named("s1", "test");
    laptop
        .claim(&s1, vec!["apps".into()], None, ClaimMode::Exclusive, None)
        .unwrap();
    round(&laptop, "desk:1", &desk_key);
    assert_eq!(held(&desk), 1, "the desk holds the laptop's claim");

    // The laptop sleeps past the expiry, and the desk compacts it away.
    sleep_past_the_expiry();
    let dropped = desk
        .journal()
        .compact(EXPIRY, Duration::ZERO, HANDOVER_RETENTION, true);
    assert_eq!(dropped, 2, "the laptop's session and its claim");
    assert!(desk.state().claims.is_empty());

    // It wakes — the same process, so the same stream — and claims something else.
    laptop
        .claim(&s1, vec!["docs".into()], None, ClaimMode::Exclusive, None)
        .unwrap();
    for _ in 0..3 {
        round(&laptop, "desk:1", &desk_key);
    }
    let stream = laptop.journal().own_stream().clone();
    let of_laptop = |c: &Cooperation| {
        let mut events = c.journal().events();
        events.retain(|e| e.stream == stream);
        events
    };
    assert_eq!(
        of_laptop(&desk),
        of_laptop(&laptop),
        "the desk holds the laptop's stream again, whole, from its first event"
    );
    assert_eq!(
        held(&desk),
        2,
        "the claim taken before the sleep holds again"
    );
    let s9 = SessionInfo::named("s9", "test");
    let refused = desk.claim(
        &s9,
        vec!["apps/majordomus-cli".into()],
        None,
        ClaimMode::Exclusive,
        None,
    );
    assert!(
        matches!(refused, Err(CooperationError::Conflict(ref c)) if c.len() == 1),
        "the desk admits a claim under the laptop's: {refused:?}"
    );
    assert_eq!(
        laptop.state().digest,
        desk.state().digest,
        "one state again"
    );
}

#[test]
fn who_took_a_handover_and_who_answered_a_review_outlive_the_takers_run() {
    let net = Arc::new(InProcess::default());
    let publisher = runtime(&net, "publisher:1");
    let taker = runtime(&net, "taker:1");
    let key = taker.dial(&["publisher:1".into()]).unwrap();
    let offered = handover("carry the mesh");
    publisher.publish_handover(offered.clone()).unwrap();
    let asked = publisher
        .request_review("s1", "feature/mesh".into(), Vec::new(), None, None)
        .unwrap();
    round(&taker, "publisher:1", &key);
    taker.consume_handover(&offered.id, "t1").unwrap();
    taker
        .answer_review(&asked.key, "t1", "approved".into(), None)
        .unwrap();
    round(&taker, "publisher:1", &key);
    let before = publisher.state();
    assert_eq!(before.handovers[0].consumed_by.len(), 1);
    assert_eq!(before.reviews[0].state, ReviewState::Answered);

    // The taker stops; past the expiry, the publisher compacts.
    sleep_past_the_expiry();
    publisher
        .journal()
        .compact(EXPIRY, Duration::ZERO, HANDOVER_RETENTION, true);
    let after = publisher.state();
    assert_eq!(
        after.handovers, before.handovers,
        "a handover still offered looks untaken again"
    );
    assert_eq!(
        after.reviews, before.reviews,
        "an answered review is open again"
    );
}

#[test]
fn a_taken_handover_keeps_its_stopped_publisher_nowhere_and_an_untaken_one_only_so_long() {
    let net = Arc::new(InProcess::default());
    let keeper = runtime(&net, "keeper:1");
    let taken = runtime(&net, "taken:1");
    let untaken = runtime(&net, "untaken:1");
    let (first, second) = (handover("picked up"), handover("left lying"));
    taken.publish_handover(first.clone()).unwrap();
    untaken.publish_handover(second.clone()).unwrap();
    for (publisher, endpoint) in [(&taken, "keeper:1"), (&untaken, "keeper:1")] {
        let key = publisher.dial(&[endpoint.into()]).unwrap();
        round(publisher, endpoint, &key);
    }
    keeper.consume_handover(&first.id, "k1").unwrap();
    assert_eq!(keeper.state().handovers.len(), 2);

    // Both publishers stop; past the expiry, the keeper compacts.
    sleep_past_the_expiry();
    let journal = keeper.journal();
    assert_eq!(
        journal.compact(EXPIRY, Duration::ZERO, HANDOVER_RETENTION, true),
        1,
        "the stream of the handover somebody took goes; the other stays"
    );
    let offered: Vec<String> = keeper.state().handovers.into_iter().map(|h| h.id).collect();
    assert_eq!(offered, vec![second.id]);
    // Past the handover retention (zero here), the untaken one lets its stream go too.
    assert_eq!(
        journal.compact(EXPIRY, Duration::ZERO, Duration::ZERO, true),
        1
    );
    assert!(keeper.state().handovers.is_empty());
}
