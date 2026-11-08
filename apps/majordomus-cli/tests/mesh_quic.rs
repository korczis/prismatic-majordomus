//! Mesh links over QUIC between real processes (ADR 0233, I4564): two `majordomus serve`
//! processes, each with its own checkout and node key, find each other through a declared
//! seed and link. Each server answers QUIC on its own port number over UDP beside HTTP, so
//! the link takes QUIC; a server with `MAJORDOMUS_MESH_QUIC=off` answers HTTP alone, and its
//! peer falls back to HTTP and says why. Nothing is mocked: loopback is the wire.

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::{Fixture, Served};
use serde_json::{json, Value};

use majordomus_cli::mesh::identity::NodeIdentity;

struct Node {
    fixture: Fixture,
    state: PathBuf,
    key: String,
}

impl Node {
    fn new() -> Self {
        let fixture = Fixture::new();
        let state = fixture.parent().join("xdg-state");
        let key = NodeIdentity::load_or_create(&state.join("majordomus").join("node.json"))
            .expect("a node identity")
            .public
            .public_key;
        Node {
            fixture,
            state,
            key,
        }
    }

    fn declare(&self, allow: &[&str], seeds: &[&Served]) {
        let mut yaml = String::from(
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntailscale:\n  enabled: false\ntrust:\n  policy: deny_unknown\n  allow:\n",
        );
        for key in allow {
            yaml.push_str(&format!("    - {key}\n"));
        }
        yaml.push_str(
            "cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: one-repository\n",
        );
        if !seeds.is_empty() {
            yaml.push_str("  seeds:\n");
            for s in seeds {
                yaml.push_str(&format!("    - http://{}\n", s.address));
            }
        }
        self.fixture.write(".ai/repo/mesh/majordomus.yaml", &yaml);
    }

    fn serve(&self, quic: bool) -> Served {
        let state = self.state.to_str().unwrap();
        let mut env = vec![("XDG_STATE_HOME", state)];
        if !quic {
            env.push(("MAJORDOMUS_MESH_QUIC", "off"));
        }
        Served::start_with_env(&self.fixture.root(), &["--discovery", "filesystem"], &env)
    }
}

fn patience() -> u32 {
    std::env::var("MAJORDOMUS_TEST_PATIENCE")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|f| (1..=20).contains(f))
        .unwrap_or(1)
}

fn wait_until<F: FnMut() -> bool>(what: &str, timeout: Duration, mut check: F) {
    let timeout = timeout * patience();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if check() {
            return;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    panic!("timed out after {timeout:?} waiting for: {what}");
}

fn connected(s: &Served) -> usize {
    s.get("/api/v1/mesh/cooperation").1["peers"]
        .as_array()
        .map(|p| p.iter().filter(|p| p["state"] == json!("connected")).count())
        .unwrap_or(0)
}

fn links(s: &Served) -> Value {
    s.get("/api/v1/mesh").1["links"].clone()
}

fn paths(s: &Served) -> Vec<Value> {
    links(s)["paths"].as_array().cloned().unwrap_or_default()
}

fn claim_held_on(s: &Served, key: &str) -> bool {
    s.get("/api/v1/mesh/state?view=full").1["state"]["claims"]
        .as_array()
        .is_some_and(|claims| {
            claims
                .iter()
                .any(|c| c["key"] == json!(key) && c["state"]["state"] == json!("held"))
        })
}

fn claim(s: &Served, session: &str, scope: &str) -> String {
    let (status, _, text) = s.request(
        "POST",
        "/api/v1/mesh/claims",
        Some(&json!({"session": session, "scope": [scope]}).to_string()),
    );
    assert_eq!(status, 200, "{text}");
    let claimed: Value = serde_json::from_str(&text).unwrap();
    claimed["key"].as_str().unwrap().to_string()
}

#[test]
fn two_runtimes_link_over_quic_and_carry_a_claim_both_ways() {
    let a = Node::new();
    let b = Node::new();
    a.declare(&[&a.key, &b.key], &[]);
    let sa = a.serve(true);
    b.declare(&[&a.key, &b.key], &[&sa]);
    let sb = b.serve(true);

    for s in [&sa, &sb] {
        let listening = links(s)["listening"].as_str().unwrap_or_default().to_string();
        assert!(
            listening.starts_with("quic://"),
            "each server answers QUIC: {}",
            links(s)
        );
    }
    wait_until("both runtimes hold one connected link", Duration::from_secs(20), || {
        connected(&sa) == 1 && connected(&sb) == 1
    });
    wait_until("B's link to A took QUIC", Duration::from_secs(10), || {
        paths(&sb).iter().any(|p| p["transport"] == json!("quic"))
    });
    assert!(
        paths(&sb).iter().all(|p| p["transport"] == json!("quic")),
        "no round fell back: {:?}",
        paths(&sb)
    );

    let from_a = claim(&sa, "a1", "apps/a");
    let from_b = claim(&sb, "b1", "apps/b");
    wait_until("A's claim is held on B", Duration::from_secs(10), || {
        claim_held_on(&sb, &from_a)
    });
    wait_until("B's claim is held on A", Duration::from_secs(10), || {
        claim_held_on(&sa, &from_b)
    });
}

#[test]
fn a_runtime_without_quic_is_reached_over_http_and_the_path_says_why() {
    let a = Node::new();
    let b = Node::new();
    a.declare(&[&a.key, &b.key], &[]);
    let sa = a.serve(false);
    assert_eq!(
        links(&sa)["reason"],
        json!("MAJORDOMUS_MESH_QUIC=off"),
        "{}",
        links(&sa)
    );
    b.declare(&[&a.key, &b.key], &[&sa]);
    let sb = b.serve(true);

    wait_until("both runtimes hold one connected link", Duration::from_secs(25), || {
        connected(&sa) == 1 && connected(&sb) == 1
    });
    wait_until("B records an HTTP path to A", Duration::from_secs(10), || {
        paths(&sb)
            .iter()
            .any(|p| p["transport"] == json!("http") && p["fallback"].is_string())
    });
    let from_b = claim(&sb, "b1", "apps/b");
    wait_until("B's claim is held on A", Duration::from_secs(10), || {
        claim_held_on(&sa, &from_b)
    });
}
