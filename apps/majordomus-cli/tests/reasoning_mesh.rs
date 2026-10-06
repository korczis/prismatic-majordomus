//! A linked mesh runtime is an optional advisor, end to end (ADR 0098 with ADR 0067).
//!
//! Two runtimes of one repository link. Every advisor of the shipped catalogue is disabled
//! in both, so the peer is the only reviewer there could be, whatever this machine has
//! installed. Then:
//!
//! 1. runtime B appears on A's `reasoning.advisors` as `peer:<B>`, available, with the
//!    capabilities the catalogue's `peers` declaration gives a reviewing runtime;
//! 2. a code-review plan on A selects it;
//! 3. a review addressed to B through A's server — the exchange the `mesh-review` adapter
//!    performs — is answered on B and read back on A;
//! 4. with B gone, the advisor disappears and the same plan decides locally: a mesh peer
//!    enriches review and is never required.

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::{Fixture, Served};
use serde_json::{json, Value};

use majordomus_cli::mesh::identity::NodeIdentity;

const HEARTBEAT: u64 = 1;
const EXPIRY: u64 = 4;

/// Every catalogue advisor, by id: disabled in the served runtimes so the outcome does
/// not depend on what this machine has installed. Peers are not in the list, so they stay
/// eligible.
const CATALOGUE_DISABLED: &str = "chatgpt,gemini,codex,ollama,lmstudio";

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
            "cooperation:\n  heartbeat_seconds: {HEARTBEAT}\n  expiry_seconds: {EXPIRY}\n  repository: reasoning-mesh\n"
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
            &[
                ("XDG_STATE_HOME", self.state.to_str().unwrap()),
                ("MAJORDOMUS_ADVISORS_DISABLE", CATALOGUE_DISABLED),
                ("MAJORDOMUS_REASONING_MODE", "standard"),
            ],
        )
    }
}

/// The slow-environment factor `tests/mesh_cooperation.rs` uses: these are real servers on
/// real sockets, and under coverage instrumentation the same work takes several times longer.
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

fn cooperation(s: &Served) -> Value {
    s.get("/api/v1/mesh/cooperation").1
}

fn connected(s: &Served) -> usize {
    cooperation(s)["peers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["state"] == json!("connected"))
        .count()
}

fn runtime_of(s: &Served) -> String {
    cooperation(s)["runtime"]
        .as_str()
        .expect("an active runtime")
        .to_string()
}

fn post(s: &Served, path: &str, body: Value) -> (u16, Value) {
    let (status, _, text) = s.request("POST", path, Some(&body.to_string()));
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

fn reviews(s: &Served) -> Vec<Value> {
    s.get("/api/v1/mesh/state").1["state"]["reviews"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// Kill a server without shutdown, as a crash or a power cut would.
fn crash(mut s: Served) {
    s.child.kill().expect("kill");
    let _ = s.child.wait();
    drop(s.child.stdin.take());
}

fn advisors(s: &Served) -> Value {
    let (status, report) = s.get("/api/v1/reasoning/advisors");
    assert_eq!(status, 200, "{report}");
    report
}

fn peer_advisor(report: &Value, runtime: &str) -> Option<Value> {
    report["advisors"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|a| a["id"] == json!(format!("peer:{runtime}")))
        .cloned()
}

fn code_review_plan(s: &Served) -> Value {
    let (status, plan) =
        s.get("/api/v1/reasoning/plan?materiality=material&capabilities=code_review");
    assert_eq!(status, 200, "{plan}");
    plan
}

#[test]
fn a_linked_peer_reviews_and_its_absence_leaves_reasoning_intact() {
    let a = Node::new();
    let b = Node::new();
    a.declare(&[&a.key, &b.key], &[]);
    let sa = a.serve();
    b.declare(&[&a.key, &b.key], &[&sa]);
    let sb = b.serve();
    wait_until(
        "both runtimes hold one connected link",
        Duration::from_secs(20),
        || connected(&sa) == 1 && connected(&sb) == 1,
    );
    let rb = runtime_of(&sb);

    // 1. the peer is an advisor, from the catalogue's declaration, and nothing else is
    let report = advisors(&sa);
    let peer =
        peer_advisor(&report, &rb).unwrap_or_else(|| panic!("B is not an advisor on A: {report}"));
    assert_eq!(peer["status"], json!("available"), "{peer}");
    assert_eq!(peer["transport"], json!("peer"));
    assert_eq!(peer["adapter"], json!("mesh-review"));
    assert!(peer["capabilities"]
        .as_array()
        .unwrap()
        .contains(&json!("code_review")));
    assert_eq!(report["available"], json!(1), "only the peer: {report}");
    assert_eq!(report["operational"], json!(true));

    // 2. the capability-driven plan selects it
    let plan = code_review_plan(&sa);
    assert_eq!(plan["outcome"], json!("consult"), "{plan}");
    assert_eq!(plan["selected"][0]["advisor"], json!(format!("peer:{rb}")));

    // 3. the review the mesh-review adapter would send: addressed to B through A, answered
    //    on B, read back on A
    let (status, written) = post(
        &sa,
        "/api/v1/mesh/reviews",
        json!({"session": "a1", "subject": "timeout semantics of the worker", "reviewer": rb}),
    );
    assert_eq!(status, 200, "{written}");
    let key = written["key"]
        .as_str()
        .expect("the review's key")
        .to_string();
    wait_until(
        "B holds the review request",
        Duration::from_secs(20),
        || reviews(&sb).iter().any(|r| r["key"] == json!(key)),
    );
    let (status, answered) = post(
        &sb,
        "/api/v1/mesh/reviews/answer",
        json!({"request": key, "session": "b1", "verdict": "changes_requested", "note": "exit and let the supervisor restart"}),
    );
    assert_eq!(status, 200, "{answered}");
    wait_until("A reads B's answer", Duration::from_secs(20), || {
        reviews(&sa)
            .iter()
            .filter(|r| r["key"] == json!(key))
            .any(|r| {
                r["answers"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|x| x["verdict"] == json!("changes_requested")))
            })
    });

    // 4. B gone: no peer advisor, and reasoning decides locally rather than failing
    crash(sb);
    wait_until(
        "A no longer counts B as a reviewing peer",
        Duration::from_secs(EXPIRY * 6),
        || peer_advisor(&advisors(&sa), &rb).is_none(),
    );
    let report = advisors(&sa);
    assert_eq!(report["available"], json!(0), "{report}");
    assert_eq!(report["operational"], json!(true));
    let plan = code_review_plan(&sa);
    assert_eq!(plan["outcome"], json!("decide_locally"), "{plan}");
    assert!(!plan["local_review"].as_array().unwrap().is_empty());
}
