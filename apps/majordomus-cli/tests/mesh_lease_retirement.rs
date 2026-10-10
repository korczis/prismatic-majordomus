//! A replaced lease retires the old process from the mesh, without dropping its clients.
mod common;

use common::{Fixture, Served};
use majordomus_cli::mesh::identity::NodeIdentity;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct Node {
    fixture: Fixture,
    state: PathBuf,
    key: String,
}

impl Node {
    fn new() -> Self {
        let fixture = Fixture::new();
        let state = fixture.parent().join("xdg-state");
        let key = NodeIdentity::load_or_create(&state.join("majordomus/node.json"))
            .unwrap()
            .public
            .public_key;
        Self {
            fixture,
            state,
            key,
        }
    }

    fn declare(&self, other: &Node, seed: Option<&Served>) {
        let mut yaml = format!(
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntrust:\n  policy: deny_unknown\n  allow:\n    - {}\n    - {}\ncooperation:\n  repository: lease-retirement\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n",
            self.key, other.key
        );
        if let Some(seed) = seed {
            yaml.push_str(&format!("  seeds:\n    - http://{}\n", seed.address));
        }
        self.fixture.write(".ai/repo/mesh/majordomus.yaml", &yaml);
    }

    fn serve(&self) -> Served {
        Served::start_with_env(
            &self.fixture.root(),
            &["--host", "127.0.0.1", "--discovery", "filesystem"],
            &[("XDG_STATE_HOME", self.state.to_str().unwrap())],
        )
    }
}

fn wait(what: &str, mut check: impl FnMut() -> bool) {
    let patience = std::env::var("MAJORDOMUS_TEST_PATIENCE")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|n| (1..=20).contains(n))
        .unwrap_or(1);
    let end = Instant::now() + Duration::from_secs(15 * patience);
    while Instant::now() < end {
        if check() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("timed out waiting for {what}");
}

fn cooperation(s: &Served) -> Value {
    let (status, body) = s.get("/api/v1/mesh/cooperation");
    assert_eq!(status, 200, "{body}");
    body
}

fn connected(s: &Served) -> bool {
    cooperation(s)["peers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["state"] == "connected")
}

// Close even during a failed assertion: Served waits for attached clients on shutdown.
struct Session<'a> {
    server: &'a Served,
    id: String,
}
impl Drop for Session<'_> {
    fn drop(&mut self) {
        self.server
            .request_with("DELETE", "/mcp", None, &[("mcp-session-id", &self.id)]);
    }
}

#[test]
fn a_retired_server_leaves_the_mesh_but_keeps_its_attached_client() {
    let observer = Node::new();
    let node = Node::new();
    observer.declare(&node, None);
    let remote = observer.serve();
    node.declare(&observer, Some(&remote));
    let old = node.serve();
    wait("both original links", || {
        connected(&old) && connected(&remote)
    });
    let original = cooperation(&old);
    assert_eq!(old.get("/api/v1/mesh").1["active"], true);
    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-06-18","capabilities":{},
        "clientInfo":{"name":"retirement-test","version":"1"}}})
    .to_string();
    let (status, headers, body) = old.request("POST", "/mcp", Some(&init));
    assert_eq!(status, 200, "{body}");
    let session = Session {
        server: &old,
        id: headers
            .iter()
            .find(|(k, _)| k == "mcp-session-id")
            .unwrap()
            .1
            .clone(),
    };

    // A real election in the same checkout, after its lease disappeared. The old
    // process stays alive, with a client, exactly as during a rebuild takeover.
    std::fs::remove_file(node.fixture.path(".ai/local/state/mcp/server.json")).unwrap();
    let successor = node.serve();
    wait("old server noticing its lease was replaced", || {
        old.get("/").1["leaseholder"] == false
    });
    wait("retired mesh and cooperation", || {
        old.get("/api/v1/mesh").1["active"] == false && cooperation(&old)["active"] == false
    });
    assert_eq!(old.request("POST", "/mcp", Some(&init)).0, 409);
    let ping = json!({"jsonrpc":"2.0","id":2,"method":"ping"}).to_string();
    let (status, _, body) = old.request_with(
        "POST",
        "/mcp",
        Some(&ping),
        &[("mcp-session-id", &session.id)],
    );
    assert_eq!(status, 200, "the existing client still works: {body}");
    assert_eq!(serde_json::from_str::<Value>(&body).unwrap()["id"], 2);

    wait("successor links", || {
        connected(&successor) && connected(&remote)
    });
    let current = cooperation(&successor);
    assert_eq!(current["runtime"], original["runtime"]);
    assert_ne!(current["stream"], original["stream"]);
    let before = current["counters"].clone();
    wait("three successful successor heartbeats", || {
        cooperation(&successor)["counters"]["syncs_out"]
            .as_u64()
            .unwrap()
            >= before["syncs_out"].as_u64().unwrap() + 3
    });
    let after = cooperation(&successor);
    assert_eq!(after["counters"]["syncs_failed"], before["syncs_failed"]);
    assert_eq!(after["counters"]["reconnects"], before["reconnects"]);
    assert_eq!(old.get("/").1["leaseholder"], false);
    assert_eq!(successor.get("/").1["leaseholder"], true);
    drop(session);
}
