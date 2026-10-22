//! What the monitor's tests share: a context over a synthetic repository that is also a git
//! repository, a cooperation runtime that writes no file, and a session opened in it.

use std::sync::Arc;

use crate::capability::Context;
use crate::mesh::config::{CooperationConfig, TrustConfig};
use crate::mesh::cooperation::{CheckoutFacts, Cooperation, CooperationSetup};
use crate::mesh::identity::NodeIdentity;
use crate::mesh::journal::SessionInfo;
use crate::mesh::link::HttpTransport;
use crate::mesh::registry::MeshRegistry;
use crate::mesh::repository::of_root_commits;
use crate::synthetic::SyntheticRepository;

/// A context over `repo`, made a git repository first so the executor's trail has a place.
pub(crate) fn context(repo: &SyntheticRepository) -> Context {
    let git = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.root())
        .status()
        .expect("git");
    assert!(git.success());
    let registry = crate::capability::CapabilityRegistry::builder()
        .with_modules(crate::capability::builtin::modules())
        .build()
        .unwrap();
    Context::new(Arc::new(repo.index().unwrap()), Arc::new(registry))
}

/// A cooperation runtime with an ephemeral identity and no journal file.
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

/// Open `session` for `worker`, saying `intent`.
pub(crate) fn session(c: &Cooperation, session: &str, worker: &str, intent: &str) {
    c.open_session(SessionInfo {
        session: session.into(),
        client: "test".into(),
        worker: Some(worker.into()),
        intent: Some(intent.into()),
        task: None,
        issue: None,
        milestone: None,
        checkout: None,
        branch: None,
        head: None,
        dirty: None,
        context: None,
    })
    .unwrap();
}
