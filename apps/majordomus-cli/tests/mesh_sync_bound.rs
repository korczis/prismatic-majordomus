//! A sync message stays under the link bound whatever the marks weigh (I2139).
//!
//! The event budget of a sync round was a constant, 600 KiB, that ignored the marks sent
//! with the events. A journal at its stream bounds — a thousand live streams, whose marks
//! carry a signed beat each, and four thousand tombstones of compacted ones — sends marks of
//! some 700 KiB, and with a full budget of events the message passed the 900 KiB link bound:
//! the peer refused it as oversized, every round, and the journal stopped replicating. Here
//! two runtimes hold that worst case — each has compacted the same four thousand streams, so
//! each is told all of the other's tombstones — and the one with the live streams must bring
//! an empty peer level over several rounds, every message under the bound.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use majordomus_cli::mesh::config::{CooperationConfig, TrustConfig};
use majordomus_cli::mesh::cooperation::{CheckoutFacts, Cooperation, CooperationSetup};
use majordomus_cli::mesh::identity::NodeIdentity;
use majordomus_cli::mesh::journal::{EventBody, Journal, Marks, MAX_STREAMS_PER_NODE};
use majordomus_cli::mesh::link::{LinkReply, LinkTransport, Signed, HELLO_PATH, MAX_LINK_MESSAGE};
use majordomus_cli::mesh::registry::MeshRegistry;
use majordomus_cli::mesh::repository::{of_root_commits, runtime_id};
use majordomus_cli::mesh::trust::TrustPolicy;

/// An in-process network that records the size of every message and every answer.
#[derive(Default)]
struct Net {
    peers: Mutex<BTreeMap<String, Weak<Cooperation>>>,
    sizes: Mutex<Vec<usize>>,
    refusals: Mutex<Vec<String>>,
}

impl LinkTransport for Net {
    fn post(&self, endpoint: &str, path: &str, message: &Signed) -> Result<LinkReply, String> {
        let peer = self
            .peers
            .lock()
            .unwrap()
            .get(endpoint)
            .and_then(Weak::upgrade)
            .ok_or_else(|| format!("{endpoint}: nobody there"))?;
        self.sizes
            .lock()
            .unwrap()
            .push(serde_json::to_vec(message).unwrap().len());
        let reply = if path == HELLO_PATH {
            peer.accept_hello(message)
        } else {
            peer.accept_sync(message)
        };
        if let Some(signed) = &reply.signed {
            self.sizes
                .lock()
                .unwrap()
                .push(serde_json::to_vec(signed).unwrap().len());
        }
        if let Some(refusal) = &reply.refusal {
            self.refusals.lock().unwrap().push(refusal.to_string());
        }
        Ok(reply)
    }
}

fn runtime(net: &Arc<Net>, endpoint: &str) -> Arc<Cooperation> {
    let c = Cooperation::new(CooperationSetup {
        identity: Arc::new(NodeIdentity::ephemeral().unwrap()),
        runtime: runtime_id(endpoint),
        repository: of_root_commits(&["root".into()]),
        endpoints: vec![endpoint.into()],
        version: "test".into(),
        config: CooperationConfig::default(),
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
    net.peers
        .lock()
        .unwrap()
        .insert(endpoint.into(), Arc::downgrade(&c));
    c
}

/// `count` streams of one event each, from nodes of at most [`MAX_STREAMS_PER_NODE`]
/// streams, numbered from `first` so that no two calls share a stream.
fn streams(repository: &str, first: usize, count: usize, beat: bool) -> Vec<Journal> {
    let mut out = Vec::with_capacity(count);
    let mut identity = Arc::new(NodeIdentity::ephemeral().unwrap());
    for n in 0..count {
        if n > 0 && n % MAX_STREAMS_PER_NODE == 0 {
            identity = Arc::new(NodeIdentity::ephemeral().unwrap());
        }
        let instance = format!("{:016x}", first + n + 1);
        let j = Journal::open(Arc::clone(&identity), &instance, repository.into(), None).unwrap();
        j.append_own(EventBody::SessionClosed {
            session: format!("s{}", first + n),
        })
        .unwrap();
        if beat {
            j.beat_own();
        }
        out.push(j);
    }
    out
}

fn hold(runtime: &Cooperation, streams: &[Journal]) {
    let expiry = Duration::from_secs(3600);
    for j in streams {
        let journal = runtime.journal();
        journal.ingest(&j.missing_for(&Marks::new(), 1 << 20), &|_| Ok(()));
        journal.merge_marks(&j.marks(), &|_| true, expiry);
    }
}

#[test]
fn sync_at_the_mark_bound_test() {
    let net = Arc::new(Net::default());
    let (a, b) = (runtime(&net, "a:1"), runtime(&net, "b:1"));
    let repository = a.journal().repository().to_string();
    let zero = Duration::ZERO;

    // both hold, then compact, the same four batches of streams: four thousand tombstones
    // on each side, every one of them for a stream the other side knows
    for batch in 0..4 {
        let dead = streams(&repository, batch * 1024, 1024, false);
        for side in [&a, &b] {
            hold(side, &dead);
            side.journal().compact(zero, zero, zero, true);
        }
    }
    // and a has a thousand live streams b has never heard, each with a signed beat
    let live = streams(&repository, 4 * 1024, 1022, true);
    hold(&a, &live);
    let marks = a.journal().marks_for(&b.journal().marks());
    let weight = serde_json::to_vec(&marks).unwrap().len();
    assert!(
        weight > MAX_LINK_MESSAGE - 600 * 1024,
        "the marks weigh {weight} bytes: too little to need what this test proves"
    );

    let key = a.dial(&["b:1".into()]).expect("a links to b");
    let want = a.journal().events().len();
    let mut rounds = 0;
    while b.journal().events().len() < want {
        rounds += 1;
        assert!(
            rounds <= 20,
            "b holds {} of {want} after 20 rounds: {:?}",
            b.journal().events().len(),
            net.refusals.lock().unwrap()
        );
        a.sync_with(&key).unwrap_or_else(|e| {
            panic!(
                "round {rounds} failed: {e:?}; refusals {:?}",
                net.refusals.lock().unwrap()
            )
        });
    }
    assert!(
        rounds > 1,
        "one round carried everything: the marks did not cost the budget"
    );
    let sizes = net.sizes.lock().unwrap();
    let largest = sizes.iter().copied().max().unwrap();
    assert!(
        largest <= MAX_LINK_MESSAGE,
        "a message of {largest} bytes left a runtime, over the {MAX_LINK_MESSAGE}-byte bound"
    );
    assert!(net.refusals.lock().unwrap().is_empty());
}
