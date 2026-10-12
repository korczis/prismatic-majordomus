//! The mesh's test fixtures, for the tests of every module.
//!
//! The one peer store is built inside `src/mesh/`, a link travels through the transports of
//! `src/mesh/`, and a link message is answered by the mesh module alone (ADR 0050, ADR 0067;
//! scripts/ci/mesh-check holds it). A test elsewhere that needs a runtime, a linked pair or a
//! context with cooperation asks for one here, so the fixture is the mesh's and no second
//! construction site grows in another module.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};

use crate::capability::Context;
use crate::mesh::config::{CooperationConfig, TrustConfig};
use crate::mesh::cooperation::{CheckoutFacts, Cooperation, CooperationSetup};
use crate::mesh::identity::NodeIdentity;
use crate::mesh::link::{HttpTransport, LinkReply, LinkTransport, Signed, HELLO_PATH};
use crate::mesh::registry::MeshRegistry;
use crate::mesh::repository::{of_root_commits, runtime_id};
use crate::mesh::trust::TrustPolicy;

/// A cooperation runtime with an ephemeral identity and no journal file; it dials nobody
/// until it is started, which a test of a projection never does.
pub(crate) fn cooperation() -> Arc<Cooperation> {
    Cooperation::new(CooperationSetup {
        identity: Arc::new(NodeIdentity::ephemeral().unwrap()),
        runtime: "0000000000000001".into(),
        repository: of_root_commits(&["root".into()]),
        endpoints: vec!["127.0.0.1:9".into()],
        version: "test".into(),
        config: CooperationConfig::default(),
        trust: TrustConfig::default(),
        journal_path: None,
        registry: Arc::new(MeshRegistry::new()),
        transport: Arc::new(HttpTransport),
        board: None,
        checkout: CheckoutFacts::default(),
    })
    .unwrap()
}

/// `c` attached to the context's mesh, as a server whose cooperation started has it.
pub(crate) fn attach(ctx: &Context, c: Arc<Cooperation>) {
    ctx.mesh.attach_cooperation(c);
}

/// An in-memory link network: runtimes of one process, the socket the only thing replaced.
#[derive(Default)]
pub(crate) struct Net(Mutex<BTreeMap<String, Weak<Cooperation>>>);

impl LinkTransport for Net {
    fn post(&self, endpoint: &str, path: &str, m: &Signed) -> Result<LinkReply, String> {
        let peer = (self.0.lock().unwrap().get(endpoint).and_then(Weak::upgrade))
            .ok_or_else(|| format!("{endpoint}: nobody there"))?;
        Ok(if path == HELLO_PATH {
            peer.accept_hello(m)
        } else {
            peer.accept_sync(m)
        })
    }
}

/// A runtime at `endpoint` on `net` whose links expire three seconds after their last
/// exchange: the shortest expiry the declaration admits (three heartbeats).
pub(crate) fn runtime(net: &Arc<Net>, endpoint: &str) -> Arc<Cooperation> {
    let c = Cooperation::new(CooperationSetup {
        identity: Arc::new(NodeIdentity::ephemeral().unwrap()),
        runtime: runtime_id(endpoint),
        repository: of_root_commits(&["root".into()]),
        endpoints: vec![endpoint.into()],
        version: "test".into(),
        config: CooperationConfig {
            heartbeat_seconds: 1,
            expiry_seconds: 3,
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
    net.0
        .lock()
        .unwrap()
        .insert(endpoint.into(), Arc::downgrade(&c));
    c
}
