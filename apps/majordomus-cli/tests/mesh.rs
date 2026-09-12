//! The mesh as another node sees it: a server with an enabled declaration activates
//! discovery, creates the node identity under the state directory it was given, and
//! serves registration — a signed envelope becomes one canonical record with its trust
//! and provenance, a replay is refused, a tampered envelope is refused, a restart is
//! the same node and never a duplicate, and every projection (`/api/v1/mesh*`) reads
//! the one registry. A disabled declaration keeps every socket closed and says why.
//!
//! The in-process half proves the rendezvous handshake end to end without a network:
//! two runtimes exchange envelopes through `register` exactly as two servers would,
//! and each converges on one record of the other.
//!
//! docs/CLAIMS.yaml marks `mesh-off-by-default`, `mesh-observation-not-authority` and
//! `mesh-declared-is-held` as guaranteed and names this file as a test that proves
//! them; the ids are written here so the link reads from both ends. `a_disabled_declaration_opens_nothing_and_says_why`
//! is the first: no socket opens until an enabled declaration is committed, and the
//! disabled declaration is reported as the reason rather than as an error. The trust
//! assertions of `a_server_activates_the_mesh_and_registration_converges_to_one_record`
//! are the second: a discovered node is labelled under the declared policy, which
//! defaults to deny_unknown, and gains no execution, no authorization and no write
//! surface by being seen.

mod common;

use common::{Fixture, Served};
use serde_json::{json, Value};

use majordomus_cli::mesh::config::{MeshConfig, MulticastConfig};
use majordomus_cli::mesh::identity::NodeIdentity;
use majordomus_cli::mesh::protocol::advertise;
use majordomus_cli::mesh::MeshRuntime;

/// An enabled declaration with every socket-opening transport off: activation, identity
/// and registration under test, and not one datagram anywhere.
const ENABLED_QUIET: &str = "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntrust:\n  policy: tofu\n";

const DISABLED: &str = "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: false\n";

fn post(s: &Served, path: &str, body: &Value) -> (u16, Value) {
    let (status, _, text) = s.request("POST", path, Some(&body.to_string()));
    let value = serde_json::from_str(&text).unwrap_or(Value::Null);
    (status, value)
}

#[test]
fn a_server_activates_the_mesh_and_registration_converges_to_one_record() {
    let f = Fixture::new();
    f.write(".ai/repo/mesh/majordomus.yaml", ENABLED_QUIET);
    let state = f.root().join("xdg-state");
    let mut s = Served::start_with_env(
        &f.root(),
        &["--discovery", "filesystem"],
        &[("XDG_STATE_HOME", state.to_str().unwrap())],
    );

    // Activation: the mesh is on, this node has an identity, and the identity file
    // landed under the state directory the server was given — nowhere else.
    let (status, mesh) = s.get("/api/v1/mesh");
    assert_eq!(status, 200);
    assert_eq!(mesh["active"], json!(true), "{mesh}");
    // The doctor of a server that activated its mesh says so on the `runtime` check —
    // the server's own verdict, which is what `majordomus mesh doctor` asks it for.
    let (status, report) = s.get("/api/v1/mesh/doctor");
    assert_eq!(status, 200);
    assert_eq!(report["ok"], json!(true), "{report}");
    let runtime = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check"] == json!("runtime"))
        .expect("a runtime check");
    assert_eq!(runtime["ok"], json!(true), "{runtime}");
    assert!(
        runtime["detail"]
            .as_str()
            .unwrap()
            .starts_with("active as "),
        "{runtime}"
    );
    let own = mesh["identity"]["node_id"]
        .as_str()
        .expect("a node id")
        .to_string();
    assert!(
        state.join("majordomus").join("node.json").is_file(),
        "the identity lives under the given XDG_STATE_HOME"
    );
    assert_eq!(mesh["trust_policy"], json!("tofu"));
    assert_eq!(
        mesh["providers"].as_array().map(Vec::len),
        Some(0),
        "every transport is off in this declaration: {mesh}"
    );

    // A signed envelope registers. The answer leads with the server's own envelope, so
    // the caller learns the server exactly as the server learned the caller.
    let caller = NodeIdentity::ephemeral().expect("an ephemeral identity");
    let envelope = advertise(
        &caller,
        1,
        &["127.0.0.1:9".into()],
        &["http".into()],
        &[],
        "test",
    );
    let (status, answer) = post(
        &s,
        "/api/v1/mesh/register",
        &json!({ "envelope": envelope }),
    );
    assert_eq!(status, 200);
    assert_eq!(answer["accepted"], json!(true), "{answer}");
    assert_eq!(
        answer["candidates"][0]["pk"], mesh["identity"]["public_key"],
        "the answering node's own envelope leads the candidates"
    );

    // One canonical record: identity, trust by the declared policy, rendezvous provenance.
    let (status, nodes) = s.get("/api/v1/mesh/nodes");
    assert_eq!(status, 200);
    assert_eq!(nodes["count"], json!(1), "{nodes}");
    let record = &nodes["nodes"][0];
    assert_eq!(record["node_id"], json!(caller.public.node_id.as_str()));
    assert_eq!(
        record["trust"]["state"],
        json!("trusted"),
        "tofu trusts first use"
    );
    assert_eq!(record["presence"], json!("present"));
    assert_eq!(record["sources"][0]["source"], json!("rendezvous"));
    assert_ne!(
        record["node_id"],
        json!(own),
        "the server never lists itself"
    );

    // A replay of the same envelope is refused and changes nothing.
    let (_, again) = post(
        &s,
        "/api/v1/mesh/register",
        &json!({ "envelope": envelope }),
    );
    assert_eq!(again["accepted"], json!(false));
    assert_eq!(again["refusal"], json!("replayed"));

    // A tampered envelope fails its signature; hostile input is a refusal, not a peer.
    let mut forged = serde_json::to_value(&envelope).unwrap();
    forged["name"] = json!("impostor");
    forged["seq"] = json!(99);
    let (_, refused) = post(&s, "/api/v1/mesh/register", &json!({ "envelope": forged }));
    assert_eq!(refused["accepted"], json!(false));
    assert_eq!(refused["refusal"], json!("signature"));

    // A restart: the same key, a new instance, sequence starting over. The record is
    // updated — never duplicated — and says it saw a restart.
    let restarted = advertise_with_instance(&caller, 1);
    let (_, after) = post(
        &s,
        "/api/v1/mesh/register",
        &json!({ "envelope": restarted }),
    );
    assert_eq!(after["accepted"], json!(true), "{after}");
    let (_, nodes) = s.get("/api/v1/mesh/nodes");
    assert_eq!(
        nodes["count"],
        json!(1),
        "a restart is not a second node: {nodes}"
    );
    assert_eq!(nodes["nodes"][0]["restarts"], json!(1));

    // The status tallies agree with the registry every surface reads.
    let (_, mesh) = s.get("/api/v1/mesh");
    assert_eq!(mesh["tallies"]["nodes"], json!(1));
    assert!(mesh["tallies"]["replayed"].as_u64().unwrap() >= 1);
    assert!(mesh["refusals"]["signature"].as_u64().unwrap() >= 1);

    assert_eq!(s.stop(), 0);
}

/// The same identity announcing from a fresh instance: what a process restart looks
/// like on the wire.
fn advertise_with_instance(identity: &NodeIdentity, seq: u64) -> majordomus_cli::mesh::Envelope {
    // A second ephemeral identity donates a fresh instance id; the signing key and
    // therefore the node id stay the caller's.
    let donor = NodeIdentity::ephemeral().expect("a donor identity");
    let mut adv = advertise(identity, seq, &[], &[], &[], "test").adv;
    adv.inst = donor.public.instance_id;
    let sig = identity.sign(&adv.signing_bytes());
    majordomus_cli::mesh::Envelope { adv, sig }
}

#[test]
fn a_disabled_declaration_opens_nothing_and_says_why() {
    let f = Fixture::new();
    f.write(".ai/repo/mesh/majordomus.yaml", DISABLED);
    let state = f.root().join("xdg-state");
    let mut s = Served::start_with_env(
        &f.root(),
        &["--discovery", "filesystem"],
        &[("XDG_STATE_HOME", state.to_str().unwrap())],
    );
    let (status, mesh) = s.get("/api/v1/mesh");
    assert_eq!(status, 200);
    assert_eq!(mesh["active"], json!(false));
    assert!(
        mesh["reason"]
            .as_str()
            .unwrap_or_default()
            .contains("disabled"),
        "{mesh}"
    );
    assert!(
        !state.join("majordomus").join("node.json").exists(),
        "a disabled mesh creates no identity"
    );
    let caller = NodeIdentity::ephemeral().expect("an identity");
    let envelope = advertise(&caller, 1, &[], &[], &[], "test");
    let (status, answer) = post(
        &s,
        "/api/v1/mesh/register",
        &json!({ "envelope": envelope }),
    );
    assert_eq!(status, 200);
    assert_eq!(answer["accepted"], json!(false));
    assert!(
        answer["refusal"]
            .as_str()
            .unwrap_or_default()
            .contains("not active"),
        "{answer}"
    );
    assert_eq!(s.stop(), 0);
}

#[test]
fn two_runtimes_discover_each_other_through_the_rendezvous_handshake() {
    let dir = tempfile::tempdir().unwrap();
    let quiet = |id: &str| MeshConfig {
        schema: "mesh/v1".into(),
        kind: "mesh-declaration".into(),
        id: id.into(),
        enabled: true,
        multicast: MulticastConfig {
            enabled: false,
            ..MulticastConfig::default()
        },
        broadcast: Default::default(),
        rendezvous: Default::default(),
        trust: Default::default(),
    };
    let a_id = NodeIdentity::load_or_create(&dir.path().join("a.json")).unwrap();
    let b_id = NodeIdentity::load_or_create(&dir.path().join("b.json")).unwrap();
    let a_node = a_id.public.node_id.clone();
    let b_node = b_id.public.node_id.clone();
    let b_envelope = advertise(
        &b_id,
        1,
        &["127.0.0.1:2".into()],
        &["http".into()],
        &[],
        "b",
    );

    let a = MeshRuntime::new();
    a.activate(
        &quiet("a"),
        a_id,
        vec!["127.0.0.1:1".into()],
        vec![],
        "test",
    )
    .unwrap();
    let b = MeshRuntime::new();
    b.activate(
        &quiet("b"),
        b_id,
        vec!["127.0.0.1:2".into()],
        vec![],
        "test",
    )
    .unwrap();

    // B registers with A, exactly as its rendezvous provider would over HTTP; A now
    // knows B, and A's answer opens with A's own envelope for B to ingest.
    let answer = a.register(&serde_json::to_value(&b_envelope).unwrap(), "test");
    assert!(answer.accepted);
    let a_envelope = answer.candidates.first().expect("A's own envelope").clone();
    let answer = b.register(&serde_json::to_value(&a_envelope).unwrap(), "test");
    assert!(answer.accepted);

    let a_sees: Vec<String> = a.nodes().iter().map(|n| n.node_id.to_string()).collect();
    let b_sees: Vec<String> = b.nodes().iter().map(|n| n.node_id.to_string()).collect();
    assert_eq!(a_sees, vec![b_node.to_string()], "A holds one record: B");
    assert_eq!(b_sees, vec![a_node.to_string()], "B holds one record: A");

    a.stop();
    b.stop();
}

/// `mesh-declared-is-held`: an enabled declaration the server could not activate is a
/// failed `runtime` check carrying the server's reason — never a quiet off. The identity
/// cannot be created when a directory sits where `node.json` must be written, which is
/// one of the ways activation fails; the server serves regardless, and the doctor says
/// why the mesh does not.
#[test]
fn an_enabled_declaration_the_server_could_not_activate_fails_the_doctor_and_names_why() {
    let f = Fixture::new();
    f.write(".ai/repo/mesh/majordomus.yaml", ENABLED_QUIET);
    let state = f.root().join("xdg-state");
    std::fs::create_dir_all(state.join("majordomus").join("node.json")).unwrap();
    let mut s = Served::start_with_env(
        &f.root(),
        &["--discovery", "filesystem"],
        &[("XDG_STATE_HOME", state.to_str().unwrap())],
    );
    let (status, mesh) = s.get("/api/v1/mesh");
    assert_eq!(
        status, 200,
        "a mesh that cannot start is a reason, not a failed server"
    );
    assert_eq!(mesh["active"], json!(false), "{mesh}");
    let (status, report) = s.get("/api/v1/mesh/doctor");
    assert_eq!(status, 200);
    assert_eq!(report["ok"], json!(false), "{report}");
    let runtime = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["check"] == json!("runtime"))
        .expect("a runtime check");
    assert_eq!(runtime["ok"], json!(false), "{runtime}");
    let detail = runtime["detail"].as_str().unwrap();
    assert!(
        detail.contains("the declaration is enabled and this server's mesh is not active"),
        "{detail}"
    );
    assert!(detail.contains("serve ensure"), "{detail}");
    // Every other check still holds: the failure is the runtime's alone.
    for c in report["checks"].as_array().unwrap() {
        if c["check"] != json!("runtime") && c["check"] != json!("identity") {
            assert_eq!(c["ok"], json!(true), "{c}");
        }
    }
    assert_eq!(s.stop(), 0);
}
