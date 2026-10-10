//! An expired claim never returns to beat a claim admitted in its absence (I2138).
//!
//! A claim's expiry is a verdict the fold reaches from liveness alone, so when the stream that
//! holds it beats again — a laptop waking, a process resumed — the claim was Held again, and
//! being the earlier acquisition it won the scope back from a claim another runtime had been
//! admitted to while it was gone. Here two real runtimes are linked; one is frozen past the
//! expiry, the other is admitted to the same scope, and the first is resumed: both must report
//! the revived claim as the one in conflict, naming the claim admitted in its absence.

mod common;

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use common::{Fixture, Served};
use serde_json::{json, Value};

use majordomus_cli::mesh::identity::NodeIdentity;

const HEARTBEAT: u64 = 1;
const EXPIRY: u64 = 4;

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
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntrust:\n  policy: deny_unknown\n  allow:\n",
        );
        for key in allow {
            yaml.push_str(&format!("    - {key}\n"));
        }
        yaml.push_str(&format!(
            "cooperation:\n  heartbeat_seconds: {HEARTBEAT}\n  expiry_seconds: {EXPIRY}\n  repository: one-repository\n"
        ));
        if !seeds.is_empty() {
            yaml.push_str("  seeds:\n");
            for s in seeds {
                yaml.push_str(&format!("    - http://{}\n", s.address));
            }
        }
        self.fixture.write(".ai/repo/mesh/majordomus.yaml", &yaml);
    }

    fn serve(&self) -> Served {
        Served::start_with_env(
            &self.fixture.root(),
            &["--discovery", "filesystem"],
            &[("XDG_STATE_HOME", self.state.to_str().unwrap())],
        )
    }
}

fn post(s: &Served, path: &str, body: Value) -> (u16, Value) {
    let (status, _, text) = s.request("POST", path, Some(&body.to_string()));
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn wait_until<F: FnMut() -> bool>(what: &str, timeout: Duration, mut check: F) {
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
        .map(|p| {
            p.iter()
                .filter(|p| p["state"] == json!("connected"))
                .count()
        })
        .unwrap_or(0)
}

fn claim(s: &Served, key: &str) -> Value {
    s.get("/api/v1/mesh/state").1["state"]["claims"]
        .as_array()
        .and_then(|c| c.iter().find(|c| c["key"] == json!(key)).cloned())
        .unwrap_or(Value::Null)
}

fn signal(s: &Served, which: &str) {
    let status = Command::new("kill")
        .args([which, &s.child.id().to_string()])
        .status()
        .expect("kill");
    assert!(status.success(), "kill {which}");
}

#[test]
fn revived_claim_conflicts_test() {
    let a = Node::new();
    let b = Node::new();
    a.declare(&[&a.key, &b.key], &[]);
    let sa = a.serve();
    b.declare(&[&a.key, &b.key], &[&sa]);
    let sb = b.serve();
    wait_until("the two runtimes link", Duration::from_secs(20), || {
        connected(&sa) == 1 && connected(&sb) == 1
    });

    // A holds the scope, and B sees it held
    let (status, held) = post(
        &sa,
        "/api/v1/mesh/claims",
        json!({"session": "a1", "scope": ["apps"]}),
    );
    assert_eq!(status, 200, "{held}");
    let first = held["key"].as_str().unwrap().to_string();
    wait_until("B sees A's claim held", Duration::from_secs(10), || {
        claim(&sb, &first)["state"]["state"] == json!("held")
    });

    // A stops beating — a laptop asleep — until B expires it, and B is admitted meanwhile
    signal(&sa, "-STOP");
    wait_until(
        "B expires A's claim",
        Duration::from_secs(EXPIRY * 4),
        || claim(&sb, &first)["state"]["state"] == json!("expired"),
    );
    let (status, admitted) = post(
        &sb,
        "/api/v1/mesh/claims",
        json!({"session": "b1", "scope": ["apps/majordomus-cli"]}),
    );
    signal(&sa, "-CONT");
    assert_eq!(status, 200, "B is admitted while A is gone: {admitted}");
    let second = admitted["key"].as_str().unwrap().to_string();

    // A beats again: its claim is live again, and on both runtimes it is the one in conflict
    for (name, s) in [("A", &sa), ("B", &sb)] {
        wait_until(
            &format!("{name} folds the revived claim as conflicted"),
            Duration::from_secs(EXPIRY * 5),
            || {
                let revived = claim(s, &first);
                revived["state"]["state"] == json!("conflicted")
                    && revived["state"]["detail"] == json!(second)
                    && claim(s, &second)["state"]["state"] == json!("held")
            },
        );
        assert_eq!(
            claim(s, &second)["supersedes"],
            json!([first]),
            "{name}: the admitted claim names the one it fenced"
        );
    }
}
