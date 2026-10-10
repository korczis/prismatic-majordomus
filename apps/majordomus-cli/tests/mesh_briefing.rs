//! The mesh, as work, for a session that starts (I2285, ADR 0128).
//!
//! Two runtimes of one repository on two machines are linked. What B is doing — its session,
//! the scope it claims, the handover it publishes and the review it asks for — must reach A's
//! briefing: the answer a session starting on A reads before it touches anything.

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::{Fixture, Served};
use serde_json::{json, Value};

use majordomus_cli::mesh::identity::NodeIdentity;

const HANDOVER: &str = "---\nschema_version: 1\ncreated_at: 2026-10-10T12:00:00Z\ntask_id: t-20261010120000-abcd\nprofile: implementation\nowner: \"b\"\nrepository_id: /machine-b/repo/.git\nworktree: /machine-b/repo\nbranch: feature/release\nhead: 0123456789abcdef0123456789abcdef01234567\nworking_tree: clean\nchanged_files:\n---\n\n# Objective\nLand the release record.\n\n# Current State\nDrafted.\n\n# Next Action\nPush it.\n";

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

fn briefing(s: &Served) -> Value {
    s.get("/api/v1/mesh/briefing").1
}

#[test]
fn what_another_machine_does_reaches_the_briefing_of_this_one() {
    let a = Node::new();
    let b = Node::new();
    a.declare(&[&a.key, &b.key], &[]);
    let sa = a.serve();
    b.declare(&[&a.key, &b.key], &[&sa]);
    let sb = b.serve();

    // before anybody works, A's briefing says so in one line
    wait_until("A's mesh runs", Duration::from_secs(15), || {
        briefing(&sa)["active"] == json!(true)
    });

    // B works: a session with an intent, an exclusive claim, a handover and a review
    let (status, _) = post(
        &sb,
        "/api/v1/mesh/sessions",
        json!({"session": "b1", "client": "codex", "intent": "the release record", "branch": "feature/release"}),
    );
    assert_eq!(status, 200);
    let (status, _) = post(
        &sb,
        "/api/v1/mesh/claims",
        json!({"session": "b1", "scope": ["docs/RELEASE.md"]}),
    );
    assert_eq!(status, 200);
    b.fixture.write(
        ".ai/local/state/handovers/20261010T120000Z--feature-release--0123456--00000000000000bb.md",
        HANDOVER,
    );
    let (status, published) = post(&sb, "/api/v1/mesh/handovers", json!({"issue": "I9"}));
    assert_eq!(status, 200, "{published}");
    let handover = published["key"].as_str().unwrap().to_string();
    let (status, requested) = post(
        &sb,
        "/api/v1/mesh/reviews",
        json!({"session": "b1", "subject": "PR #9", "issue": "I9"}),
    );
    assert_eq!(status, 200, "{requested}");

    // all of it reaches A's briefing, as data and as the lines a session reads
    wait_until("A is briefed on B's work", Duration::from_secs(20), || {
        let b = briefing(&sa);
        let text = b["text"].as_str().unwrap_or_default().to_string();
        text.contains("the release record")
            && text.contains("!docs/RELEASE.md")
            && text.contains("Land the release record.")
            && text.contains("PR #9")
    });
    let answer = briefing(&sa);
    let machines = answer["briefing"]["machines"].as_array().unwrap().clone();
    let remote = machines
        .iter()
        .find(|m| m["this_machine"] == json!(false))
        .expect("B's machine is on A's briefing");
    assert!(remote["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["client"] == json!("codex") && s["branch"] == json!("feature/release")));
    assert_eq!(answer["briefing"]["handovers"][0]["id"], json!(handover));
    assert!(answer["text"]
        .as_str()
        .unwrap()
        .contains(&format!("majordomus_mesh_handover_consume {handover}")));

    // B's own briefing does not tell it to take the handover it published
    let own = briefing(&sb);
    assert!(own["briefing"]["handovers"].as_array().unwrap().is_empty());
}
