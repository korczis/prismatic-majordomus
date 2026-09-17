//! Mesh cooperation between real processes over real TCP: every test here starts separate
//! `majordomus serve` processes, each with its own repository checkout and its own node
//! identity (its own `XDG_STATE_HOME`, which is what a separate machine is to the mesh),
//! and lets them find each other through a declared seed, link through the signed
//! handshake, and replicate through the signed sync — the same code path a LAN or a
//! tailnet takes, with loopback as the wire. Nothing here is mocked; the in-process
//! transport of the unit tests is not used.
//!
//! What is proven, one test each:
//!
//! - two runtimes link both ways and share sessions, claims, conflicts, reviews and a
//!   handover that lands as a local record (acceptance scenarios A and B);
//! - runtimes of different repositories never form a link, and say why (scenario D);
//! - an untrusted key is refused although it was reachable (discovery is not trust);
//! - a runtime killed without shutdown expires on its peer, its claim stops excluding,
//!   and its restart reconnects as the same runtime without a duplicate (scenario C);
//! - three runtimes in a line converge through the middle one, each event once (E);
//! - malformed, mis-signed and future-protocol messages are typed refusals over HTTP;
//! - two worktrees of one machine are two runtimes under one key, and link;
//! - the command line answers the same state the HTTP surface does.
//!
//! docs/CLAIMS.yaml names this file as the proof of `mesh-cooperation-links`,
//! `mesh-claims-cross-runtime`, `mesh-repository-isolation`, `mesh-liveness-expiry` and
//! `mesh-three-runtime-convergence`; the ids are written here so the link reads from
//! both ends.

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::{run_in, Fixture, Served};
use serde_json::{json, Value};

use majordomus_cli::mesh::identity::NodeIdentity;
use majordomus_cli::mesh::link::{sign, Domain, Hello, RuntimeCard, HELLO_PATH, SYNC_PATH};

const HEARTBEAT: u64 = 1;
const EXPIRY: u64 = 4;

/// One node: a checkout, a state directory holding its key, and its public key.
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

    /// A second checkout on the same machine: a new fixture, the same key.
    fn beside(other: &Node) -> Self {
        Node {
            fixture: Fixture::new(),
            state: other.state.clone(),
            key: other.key.clone(),
        }
    }

    fn declare(&self, repository: &str, allow: &[&str], seeds: &[&Served]) {
        let mut yaml = String::from(
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntrust:\n  policy: deny_unknown\n  allow:\n",
        );
        for key in allow {
            yaml.push_str(&format!("    - {key}\n"));
        }
        yaml.push_str(&format!(
            "cooperation:\n  heartbeat_seconds: {HEARTBEAT}\n  expiry_seconds: {EXPIRY}\n  repository: {repository}\n"
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

/// How much longer than the written bound to wait. These tests run real servers over real
/// sockets, so every bound here is wall-clock; under coverage instrumentation the same work
/// takes several times as long and a bound that is generous on a developer's machine expires
/// on the runner. `MAJORDOMUS_TEST_PATIENCE` lets the slow environment say so, rather than
/// every bound being written for the slowest one — which would turn a real hang into a
/// ten-minute wait for everybody.
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

fn peers(s: &Served) -> Vec<Value> {
    s.get("/api/v1/mesh/cooperation").1["peers"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn connected(s: &Served) -> usize {
    peers(s)
        .iter()
        .filter(|p| p["state"] == json!("connected"))
        .count()
}

fn state(s: &Served) -> Value {
    s.get("/api/v1/mesh/state").1["state"].clone()
}

fn claims_in(s: &Served, st: &str) -> Vec<Value> {
    state(s)["claims"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c["state"]["state"] == json!(st))
        .collect()
}

fn runtime_of(s: &Served) -> String {
    s.get("/api/v1/mesh/cooperation").1["runtime"]
        .as_str()
        .expect("an active runtime")
        .to_string()
}

/// Kill a server without shutdown, as a crash or a power cut would.
fn crash(mut s: Served) {
    s.child.kill().expect("kill");
    let _ = s.child.wait();
    drop(s.child.stdin.take());
}

const HANDOVER: &str = "---\nschema_version: 1\ncreated_at: 2026-09-15T12:00:00Z\ntask_id: t-20260915120000-abcd\nprofile: implementation\nowner: \"a\"\nrepository_id: /machine-a/repo/.git\nworktree: /machine-a/repo\nbranch: feature/mesh\nhead: 0123456789abcdef0123456789abcdef01234567\nworking_tree: clean\nchanged_files:\n---\n\n# Objective\nMake the mesh real.\n\n# Current State\nLinks work; the Cockpit does not show them.\n\n# Next Action\nRender the machine tree.\n";

#[test]
fn two_runtimes_link_and_share_sessions_claims_reviews_and_a_handover() {
    // mesh-cooperation-links, mesh-claims-cross-runtime
    let a = Node::new();
    let b = Node::new();
    a.declare("one-repository", &[&a.key, &b.key], &[]);
    let sa = a.serve();
    b.declare("one-repository", &[&a.key, &b.key], &[&sa]);
    let sb = b.serve();

    wait_until(
        "both runtimes hold one connected link",
        Duration::from_secs(15),
        || connected(&sa) == 1 && connected(&sb) == 1,
    );
    let (ra, rb) = (runtime_of(&sa), runtime_of(&sb));
    assert_ne!(ra, rb);
    assert_eq!(
        peers(&sa)[0]["runtime"],
        json!(rb),
        "A holds B, by runtime key"
    );
    assert_eq!(
        peers(&sb)[0]["runtime"],
        json!(ra),
        "B holds A, by runtime key"
    );
    assert_eq!(peers(&sb)[0]["trust"]["state"], json!("trusted"));

    // A session and an exclusive claim on A become visible on B.
    let (status, opened) = post(
        &sa,
        "/api/v1/mesh/sessions",
        json!({"session": "a1", "client": "claude-code", "intent": "implement #184", "issue": "#184", "branch": "feature/mesh"}),
    );
    assert_eq!(status, 200, "{opened}");
    let (status, claimed) = post(
        &sa,
        "/api/v1/mesh/claims",
        json!({"session": "a1", "scope": ["apps/majordomus-cli"], "issue": "#184", "intent": "the link layer"}),
    );
    assert_eq!(status, 200, "{claimed}");
    let claim_key = claimed["key"].as_str().unwrap().to_string();
    wait_until("B sees A's claim held", Duration::from_secs(10), || {
        claims_in(&sb, "held")
            .iter()
            .any(|c| c["key"] == json!(claim_key))
    });
    let sessions = state(&sb)["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        sessions
            .iter()
            .any(|s| s["info"]["issue"] == json!("#184") && s["runtime"] == json!(ra)),
        "B sees A's session and the issue it works on: {sessions:?}"
    );

    // B's conflicting exclusive claim is refused, naming A's claim.
    let (status, refused) = post(
        &sb,
        "/api/v1/mesh/claims",
        json!({"session": "b1", "scope": ["apps"]}),
    );
    assert_eq!(status, 422, "{refused}");
    let message = refused["error"]["message"].as_str().unwrap_or_default();
    assert!(message.starts_with("claim_conflict"), "{message}");
    assert!(
        message.contains(&claim_key),
        "the refusal names the claim it meets: {message}"
    );
    // A non-overlapping claim is fine.
    let (status, _) = post(
        &sb,
        "/api/v1/mesh/claims",
        json!({"session": "b1", "scope": ["docs"]}),
    );
    assert_eq!(status, 200);

    // B asks A for a review; A answers; B sees the answer.
    let (status, requested) = post(
        &sb,
        "/api/v1/mesh/reviews",
        json!({"session": "b1", "subject": "feature/mesh", "issue": "#184", "reviewer": ra}),
    );
    assert_eq!(status, 200, "{requested}");
    let review = requested["key"].as_str().unwrap().to_string();
    wait_until("A sees B's review request", Duration::from_secs(10), || {
        state(&sa)["reviews"]
            .as_array()
            .is_some_and(|r| r.iter().any(|x| x["key"] == json!(review)))
    });
    let (status, _) = post(
        &sa,
        "/api/v1/mesh/reviews/answer",
        json!({"request": review, "session": "a1", "verdict": "approved", "note": "ship it"}),
    );
    assert_eq!(status, 200);
    wait_until("B sees the answer", Duration::from_secs(10), || {
        state(&sb)["reviews"].as_array().is_some_and(|r| {
            r.iter()
                .any(|x| x["key"] == json!(review) && x["state"] == json!("answered"))
        })
    });

    // A publishes its handover; B consumes it into its own handovers directory.
    a.fixture.write(
        ".ai/local/state/handovers/20260915T120000Z--feature-mesh--0123456--00000000000000aa.md",
        HANDOVER,
    );
    let (status, published) = post(&sa, "/api/v1/mesh/handovers", json!({"issue": "#184"}));
    assert_eq!(status, 200, "{published}");
    let handover = published["key"].as_str().unwrap().to_string();
    wait_until("B holds A's handover", Duration::from_secs(10), || {
        state(&sb)["handovers"]
            .as_array()
            .is_some_and(|h| h.iter().any(|x| x["id"] == json!(handover)))
    });
    let (status, consumed) = post(
        &sb,
        "/api/v1/mesh/handovers/consume",
        json!({"handover": handover, "session": "b1"}),
    );
    assert_eq!(status, 200, "{consumed}");
    let record = b.fixture.path(consumed["path"].as_str().unwrap());
    let text = std::fs::read_to_string(&record).expect("the consumed handover is a local record");
    assert!(text.contains("branch: feature/mesh"));
    assert!(text.contains("issue: \"#184\""));
    assert!(text.contains("# Next Action\nRender the machine tree."));
    assert!(
        !text.contains("/machine-a/repo\n"),
        "no path of A's disk is written on B"
    );
    wait_until("A sees that B consumed it", Duration::from_secs(10), || {
        state(&sa)["handovers"].as_array().is_some_and(|h| {
            h.iter().any(|x| {
                x["id"] == json!(handover)
                    && x["consumed_by"].as_array().is_some_and(|c| !c.is_empty())
            })
        })
    });

    // One state, two runtimes.
    wait_until("the digests agree", Duration::from_secs(10), || {
        state(&sa)["digest"] == state(&sb)["digest"]
    });

    // The command line answers from the same server the HTTP surface is.
    let (code, out, err) = run_in(
        &b.fixture.root(),
        &["mesh", "peers", "--format", "json"],
        "",
    );
    assert_eq!(code, 0, "{err}");
    let cli: Value = serde_json::from_str(&out).expect("mesh peers --format json is JSON");
    let http = sb.get("/api/v1/mesh/peers").1;
    assert_eq!(cli["runtime"], http["runtime"]);
    assert_eq!(cli["repository"], http["repository"]);
    let (code, _, _) = run_in(
        &b.fixture.root(),
        &[
            "mesh",
            "claim",
            "apps/majordomus-cli/src",
            "--session",
            "b2",
            "--format",
            "json",
        ],
        "",
    );
    assert_eq!(code, 10, "a refused claim exits 10 on the command line");
    let (code, out, err) = run_in(
        &b.fixture.root(),
        &["mesh", "verify", "--format", "json"],
        "",
    );
    let verified: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
    assert_eq!(code, 0, "mesh verify: {out}\n{err}");
    assert_eq!(verified["ok"], json!(true), "{verified}");
}

#[test]
fn runtimes_of_different_repositories_never_link_and_say_why() {
    // mesh-repository-isolation
    let x = Node::new();
    let y = Node::new();
    x.declare("repository-x", &[&x.key, &y.key], &[]);
    let sx = x.serve();
    y.declare("repository-y", &[&x.key, &y.key], &[&sx]);
    let sy = y.serve();

    wait_until("Y records X's refusal", Duration::from_secs(15), || {
        sy.get("/api/v1/mesh/cooperation").1["refused"]
            .as_array()
            .is_some_and(|r| {
                r.iter()
                    .any(|x| x["refusal"]["code"] == json!("repository_mismatch"))
            })
    });
    let (_, xs) = sx.get("/api/v1/mesh/cooperation");
    assert!(
        xs["refused"]
            .as_array()
            .is_some_and(|r| r.iter().any(|v| v["direction"] == json!("inbound")
                && v["refusal"]["code"] == json!("repository_mismatch"))),
        "X refused Y's hello: {xs}"
    );
    post(
        &sx,
        "/api/v1/mesh/claims",
        json!({"session": "x1", "scope": ["apps"]}),
    );
    std::thread::sleep(Duration::from_secs(HEARTBEAT * 3));
    assert!(
        peers(&sx).is_empty() && peers(&sy).is_empty(),
        "no cooperative relationship formed"
    );
    assert!(
        state(&sy)["claims"]
            .as_array()
            .is_some_and(|c| c.is_empty()),
        "nothing of X's repository reached Y"
    );
    // The same two machines serving the same repository do link.
    let z = Node::beside(&y);
    z.declare("repository-x", &[&x.key, &y.key], &[&sx]);
    let sz = z.serve();
    wait_until(
        "a runtime of the same repository links",
        Duration::from_secs(15),
        || connected(&sz) == 1,
    );
}

#[test]
fn a_reachable_but_untrusted_key_is_refused() {
    let a = Node::new();
    let stranger = Node::new();
    a.declare("one-repository", &[&a.key], &[]);
    let sa = a.serve();
    stranger.declare("one-repository", &[&a.key, &stranger.key], &[&sa]);
    let ss = stranger.serve();
    wait_until(
        "the stranger is refused as untrusted",
        Duration::from_secs(15),
        || {
            ss.get("/api/v1/mesh/cooperation").1["refused"]
                .as_array()
                .is_some_and(|r| r.iter().any(|x| x["refusal"]["code"] == json!("untrusted")))
        },
    );
    assert!(peers(&sa).is_empty(), "A linked nobody");
}

#[test]
fn a_killed_runtime_expires_and_its_restart_reconnects_as_the_same_runtime() {
    // mesh-liveness-expiry
    let a = Node::new();
    let b = Node::new();
    a.declare("one-repository", &[&a.key, &b.key], &[]);
    let sa = a.serve();
    b.declare("one-repository", &[&a.key, &b.key], &[&sa]);
    let sb = b.serve();
    wait_until("linked", Duration::from_secs(15), || {
        connected(&sa) == 1 && connected(&sb) == 1
    });
    let rb = runtime_of(&sb);

    let (status, claimed) = post(
        &sb,
        "/api/v1/mesh/claims",
        json!({"session": "b1", "scope": ["apps"]}),
    );
    assert_eq!(status, 200, "{claimed}");
    wait_until("A sees B's claim", Duration::from_secs(10), || {
        claims_in(&sa, "held").len() == 1
    });
    let (status, _) = post(
        &sa,
        "/api/v1/mesh/claims",
        json!({"session": "a1", "scope": ["apps/x"]}),
    );
    assert_eq!(status, 422, "B's live claim excludes A");

    // Crash B: no shutdown, no release.
    let killed_at = Instant::now();
    crash(sb);
    wait_until(
        "A expires B's link",
        Duration::from_secs(EXPIRY + 8),
        || {
            peers(&sa)
                .iter()
                .any(|p| p["runtime"] == json!(rb) && p["state"] == json!("expired"))
        },
    );
    wait_until(
        "B's claim expires at A",
        Duration::from_secs(EXPIRY + 8),
        || claims_in(&sa, "expired").len() == 1,
    );
    assert!(
        killed_at.elapsed() >= Duration::from_secs(EXPIRY - 1),
        "expiry is the declared expiry, not an instant guess"
    );
    let (status, reclaimed) = post(
        &sa,
        "/api/v1/mesh/claims",
        json!({"session": "a1", "scope": ["apps/x"]}),
    );
    assert_eq!(
        status, 200,
        "an expired lease no longer excludes: {reclaimed}"
    );

    // Restart B in the same checkout with the same key: the same runtime, a new instance.
    let sb = b.serve();
    assert_eq!(runtime_of(&sb), rb, "a restart is the same runtime");
    wait_until("B reconnects", Duration::from_secs(15), || {
        connected(&sa) == 1 && connected(&sb) == 1
    });
    let listed = peers(&sa);
    assert_eq!(listed.len(), 1, "no duplicate peer: {listed:?}");
    assert!(
        listed[0]["restarts"].as_u64().unwrap_or(0) >= 1,
        "{listed:?}"
    );
    // B's previous claim stays expired: a restart resurrects no ownership.
    wait_until(
        "the state converges after the restart",
        Duration::from_secs(10),
        || state(&sa)["digest"] == state(&sb)["digest"],
    );
    let at_b = state(&sb);
    let held: Vec<&Value> = at_b["claims"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["state"]["state"] == json!("held"))
        .collect();
    assert_eq!(held.len(), 1, "only A's new claim holds: {at_b}");
    assert_eq!(held[0]["key"], reclaimed["key"]);
}

#[test]
fn three_runtimes_in_a_line_converge_through_the_middle_without_loops() {
    // mesh-three-runtime-convergence
    let a = Node::new();
    let b = Node::new();
    let c = Node::new();
    let keys = [a.key.as_str(), b.key.as_str(), c.key.as_str()];
    a.declare("one-repository", &keys, &[]);
    c.declare("one-repository", &keys, &[]);
    let sa = a.serve();
    let sc = c.serve();
    b.declare("one-repository", &keys, &[&sa, &sc]);
    let sb = b.serve();
    wait_until("B links to both ends", Duration::from_secs(15), || {
        connected(&sb) == 2
    });
    assert_eq!(peers(&sa).len(), 1, "A links to B only");
    assert_eq!(peers(&sc).len(), 1, "C links to B only");

    post(
        &sa,
        "/api/v1/mesh/claims",
        json!({"session": "a1", "scope": ["apps"]}),
    );
    post(
        &sc,
        "/api/v1/mesh/claims",
        json!({"session": "c1", "scope": ["docs"]}),
    );
    wait_until(
        "C sees A's claim through B, and A sees C's",
        Duration::from_secs(15),
        || claims_in(&sc, "held").len() == 2 && claims_in(&sa, "held").len() == 2,
    );
    wait_until("three digests agree", Duration::from_secs(10), || {
        let d = state(&sa)["digest"].clone();
        d == state(&sb)["digest"] && d == state(&sc)["digest"]
    });
    // Each event once everywhere: two sessions opened and two claims, four events.
    for s in [&sa, &sb, &sc] {
        let events = s.get("/api/v1/mesh/events?limit=1000").1;
        assert_eq!(events["count"], json!(4), "{events}");
    }
    // Quiet rounds carry nothing further: replication stopped rather than looping.
    let served = |s: &Served| {
        s.get("/api/v1/mesh/cooperation").1["counters"]["events_served"]
            .as_u64()
            .unwrap_or(0)
    };
    let sent = |s: &Served| {
        s.get("/api/v1/mesh/cooperation").1["counters"]["events_sent"]
            .as_u64()
            .unwrap_or(0)
    };
    let before = (served(&sa) + served(&sc), sent(&sb));
    std::thread::sleep(Duration::from_secs(HEARTBEAT * 4));
    assert_eq!(
        (served(&sa) + served(&sc), sent(&sb)),
        before,
        "no event travels twice"
    );
    // A runtime of the line can still be refused an overlapping claim held at the far end.
    let (status, _) = post(
        &sa,
        "/api/v1/mesh/claims",
        json!({"session": "a2", "scope": ["docs/MESH.md"]}),
    );
    assert_eq!(status, 422, "C's claim excludes A through B");
}

#[test]
fn malformed_forged_and_future_link_messages_are_typed_refusals_over_http() {
    let a = Node::new();
    a.declare("one-repository", &[&a.key], &[]);
    let sa = a.serve();
    let code = |v: &Value| v["refusal"]["code"].as_str().unwrap_or("").to_string();

    let (status, reply) = post(&sa, HELLO_PATH, json!({"body": {"nope": 1}, "sig": ""}));
    assert_eq!(status, 200);
    assert_eq!(code(&reply), "malformed");

    let stranger = NodeIdentity::ephemeral().unwrap();
    let card = RuntimeCard {
        pk: stranger.public.public_key.clone(),
        runtime: "00000000000000ff".into(),
        instance: stranger.public.instance_id.as_str().into(),
        repo: "0".repeat(32),
        name: "stranger".into(),
        version: "9.9.9".into(),
        features: vec!["claims".into()],
        endpoints: vec![],
    };
    let hello = |min: u32, max: u32| Hello {
        proto_min: min,
        proto_max: max,
        card: card.clone(),
        nonce: majordomus_cli::mesh::link::fresh_token(),
        ts: majordomus_cli::mesh::protocol::now(),
    };
    let future = sign(
        &stranger,
        Domain::Hello,
        serde_json::to_value(hello(7, 9)).unwrap(),
    );
    let (_, reply) = post(&sa, HELLO_PATH, serde_json::to_value(&future).unwrap());
    assert_eq!(code(&reply), "protocol_unsupported");
    assert_eq!(reply["refusal"]["proto_max"], json!(1));

    let mut forged = sign(
        &stranger,
        Domain::Hello,
        serde_json::to_value(hello(1, 1)).unwrap(),
    );
    forged.body["card"]["name"] = json!("impostor");
    let (_, reply) = post(&sa, HELLO_PATH, serde_json::to_value(&forged).unwrap());
    assert_eq!(code(&reply), "signature");

    let wrong_repo = sign(
        &stranger,
        Domain::Hello,
        serde_json::to_value(hello(1, 1)).unwrap(),
    );
    let (_, reply) = post(&sa, HELLO_PATH, serde_json::to_value(&wrong_repo).unwrap());
    assert_eq!(code(&reply), "repository_mismatch");

    let (_, reply) = post(
        &sa,
        SYNC_PATH,
        serde_json::to_value(sign(
            &stranger,
            Domain::SyncRequest,
            json!({"link": "0".repeat(32), "counter": 1, "instance": "0".repeat(16), "marks": {}, "events": []}),
        ))
        .unwrap(),
    );
    assert_eq!(code(&reply), "unknown_link");

    let huge = format!(
        "{{\"body\":{{\"x\":\"{}\"}},\"sig\":\"\"}}",
        "y".repeat(1_100_000)
    );
    let (status, _, _) = sa.request("POST", HELLO_PATH, Some(&huge));
    assert_eq!(
        status, 413,
        "the transport bound refuses before the protocol reads"
    );

    // The runtime is still healthy after all of it.
    let (_, cooperation) = sa.get("/api/v1/mesh/cooperation");
    assert_eq!(cooperation["active"], json!(true));
    assert!(
        cooperation["counters"]["refused_in"]["malformed"]
            .as_u64()
            .unwrap_or(0)
            >= 1
    );
}

#[test]
fn two_worktrees_of_one_machine_are_two_runtimes_under_one_key() {
    let a = Node::new();
    let b = Node::beside(&a);
    a.declare("one-repository", &[&a.key], &[]);
    let sa = a.serve();
    b.declare("one-repository", &[&a.key], &[&sa]);
    let sb = b.serve();
    wait_until("the two checkouts link", Duration::from_secs(15), || {
        connected(&sa) == 1 && connected(&sb) == 1
    });
    let (ra, rb) = (runtime_of(&sa), runtime_of(&sb));
    assert_eq!(ra[..32], rb[..32], "one machine, one node key");
    assert_ne!(ra, rb, "two checkouts, two runtimes");
    assert_eq!(peers(&sa)[0]["local"], json!(true));
    let (status, _) = post(
        &sa,
        "/api/v1/mesh/claims",
        json!({"session": "wt1", "scope": ["site"]}),
    );
    assert_eq!(status, 200);
    wait_until(
        "the sibling checkout sees the claim",
        Duration::from_secs(10),
        || claims_in(&sb, "held").len() == 1,
    );
    let (status, _) = post(
        &sb,
        "/api/v1/mesh/claims",
        json!({"session": "wt2", "scope": ["site/content"]}),
    );
    assert_eq!(
        status, 422,
        "a claim in one worktree excludes the other worktree's session"
    );
}
