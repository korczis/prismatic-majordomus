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
//! docs/CLAIMS.yaml marks `mesh-off-by-default`, `mesh-one-registry` and
//! `mesh-observation-not-authority` as guaranteed and names this file as the test that
//! proves them; the ids are written here so the link reads from both ends. The server half
//! of `mesh-declared-is-held` is here too — the `runtime` check of `/api/v1/mesh/doctor` is
//! the server's own verdict on its declaration, active, off as declared, or failed with the
//! reason — and test/cases/494_the_mesh_is_declared_and_held.sh, the test that claim names,
//! carries it through the command line and the session-start briefing.
//! `mesh-one-registry` is the registry half of
//! `a_server_activates_the_mesh_and_registration_converges_to_one_record`: one canonical
//! record per node and runtime however many envelopes arrive, a replay refused per
//! instance, a restart that updates rather than duplicates, and every projection —
//! `/api/v1/mesh`, `/api/v1/mesh/nodes` — reading that one registry and holding no peers
//! of its own. (The claim named test/cases/130_mesh.sh until 2026-09-20; that case says in
//! its own header that it runs without a network and covers the operator's path, so it
//! carried no part of this claim.) `a_disabled_declaration_opens_nothing_and_says_why`
//! is the first: no socket opens until an enabled declaration is committed, and the
//! disabled declaration is reported as the reason rather than as an error. The trust
//! assertions of `a_server_activates_the_mesh_and_registration_converges_to_one_record`
//! are the second: a discovered node is labelled under the declared policy, which
//! defaults to deny_unknown, and gains no execution, no authorization and no write
//! surface by being seen.

mod common;

use common::{run_in, Fixture, Served};
use serde_json::{json, Value};

use majordomus_cli::mesh::config::{MeshConfig, MulticastConfig};
use majordomus_cli::mesh::identity::NodeIdentity;
use majordomus_cli::mesh::protocol::advertise;
use majordomus_cli::mesh::MeshRuntime;

/// An enabled declaration with every socket-opening transport off: activation, identity
/// and registration under test, and not one datagram anywhere.
const ENABLED_QUIET: &str = "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntrust:\n  policy: tofu\n";

const DISABLED: &str = "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: false\n";

/// The `runtime` check of the server's own `mesh doctor` report.
fn runtime_check(s: &Served) -> Value {
    let (status, report) = s.get("/api/v1/mesh/doctor");
    assert_eq!(status, 200, "{report}");
    report["checks"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["check"] == json!("runtime"))
        .cloned()
        .unwrap_or_else(|| panic!("the server's doctor has no runtime check: {report}"))
}

fn post(s: &Served, path: &str, body: &Value) -> (u16, Value) {
    let (status, _, text) = s.request("POST", path, Some(&body.to_string()));
    let value = serde_json::from_str(&text).unwrap_or(Value::Null);
    (status, value)
}

#[test]
fn a_server_activates_the_mesh_and_registration_converges_to_one_record() {
    let f = Fixture::new();
    f.write(".ai/repo/mesh/majordomus.yaml", ENABLED_QUIET);
    // committed: trust on first use is honoured only from the trunk's copy (I2135)
    f.commit("the mesh declaration");
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
    // The doctor of a server that activated its mesh says so on its `runtime` check: the
    // server's own verdict, which is what `majordomus mesh doctor` asks it for.
    let runtime = runtime_check(&s);
    assert_eq!(runtime["ok"], json!(true), "{runtime}");
    assert!(
        runtime["detail"]
            .as_str()
            .unwrap_or_default()
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

    // Replay protection is per instance, so the pre-restart envelope is a fresh sequence
    // of a past instance rather than a replay of the current one — and the registry is
    // one record per node either way: whatever arrives, from whichever instance, in
    // whichever order, this node is one row and never two.
    post(
        &s,
        "/api/v1/mesh/register",
        &json!({ "envelope": envelope }),
    );
    let (_, nodes) = s.get("/api/v1/mesh/nodes");
    assert_eq!(
        nodes["count"],
        json!(1),
        "one record per node, across instances and sources: {nodes}"
    );
    assert_eq!(
        nodes["nodes"][0]["node_id"],
        json!(caller.public.node_id.as_str())
    );

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
    // Off is the declaration's answer, not a failure: the doctor holds and says why.
    let runtime = runtime_check(&s);
    assert_eq!(runtime["ok"], json!(true), "{runtime}");
    assert!(
        runtime["detail"]
            .as_str()
            .unwrap_or_default()
            .starts_with("off, as declared"),
        "{runtime}"
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
    // each runtime lists only its own key: an unlisted machine stays closed, and the
    // other's key stays unknown, so its verdict is observed
    let quiet = |id: &str, own: &str| MeshConfig {
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
        trust: majordomus_cli::mesh::config::TrustConfig {
            allow: vec![own.to_string()],
            ..Default::default()
        },
        cooperation: Default::default(),
    };
    let a_id = NodeIdentity::load_or_create(&dir.path().join("a.json")).unwrap();
    let b_id = NodeIdentity::load_or_create(&dir.path().join("b.json")).unwrap();
    let a_node = a_id.public.node_id.clone();
    let (a_key, b_key) = (
        a_id.public.public_key.clone(),
        b_id.public.public_key.clone(),
    );
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
        &quiet("a", &a_key),
        a_id,
        vec!["127.0.0.1:1".into()],
        vec![],
        "test",
    )
    .unwrap();
    let b = MeshRuntime::new();
    b.activate(
        &quiet("b", &b_key),
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
/// failed `runtime` check carrying the server's reason and the remedy — never a quiet off.
/// The identity cannot be created when a directory sits where `node.json` must be written,
/// which is one of the ways activation fails; the server serves regardless (a mesh that
/// cannot start is a reason, never a failed server), and its doctor says why the mesh does
/// not run.
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
    let runtime = runtime_check(&s);
    assert_eq!(runtime["ok"], json!(false), "{runtime}");
    let detail = runtime["detail"].as_str().unwrap_or_default();
    assert!(
        detail.starts_with("the declaration is enabled and this server's mesh is not active"),
        "{detail}"
    );
    // The server's own reason, without the "not active" the verdict has already said.
    let reason = mesh["reason"].as_str().unwrap_or_default();
    let why = reason.strip_prefix("not active: ").unwrap_or(reason);
    assert!(
        !why.is_empty() && detail.ends_with(why),
        "the verdict carries the server's own reason '{why}': {detail}"
    );
    assert!(
        runtime["remediation"]
            .as_str()
            .unwrap_or_default()
            .contains("serve ensure"),
        "{runtime}"
    );
    assert_eq!(s.stop(), 0);
}

/// An enabled declaration whose one transport is the multicast group: the plan it implies
/// on any machine is that group's port and nothing else — no hub, no broadcast, no link.
const ENABLED_MULTICAST: &str =
    "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n";

/// `mesh firewall --format json` in the repository at `root`: the exit code and the report.
fn firewall(root: &std::path::Path, extra: &[&str]) -> (i32, Value) {
    let mut args = vec!["mesh", "firewall"];
    args.extend_from_slice(extra);
    args.extend_from_slice(&["--format", "json"]);
    let (code, out, err) = run_in(root, &args, "");
    let report = serde_json::from_str(&out).unwrap_or_else(|e| {
        panic!(
            "`majordomus {}` is not JSON ({e}): {out}\n{err}",
            args.join(" ")
        )
    });
    (code, report)
}

/// `mesh-firewall-is-derived`, on the command line: a mesh that is off needs nothing
/// admitted, and `mesh firewall apply` over a plan that needs nothing is refused before
/// any privilege is asked about — so this test runs no firewall tool on any machine, as
/// any user, root included, and can never change the firewall of the machine it runs on.
#[test]
fn a_mesh_that_is_off_needs_no_firewall_rule_and_apply_refuses_without_running_anything() {
    let f = Fixture::new();
    let root = f.root();
    // No declaration at all, then a disabled one: the same answer for the same reason.
    for declaration in [None, Some(DISABLED)] {
        if let Some(text) = declaration {
            f.write(".ai/repo/mesh/majordomus.yaml", text);
            f.commit("declare the mesh, disabled");
        }
        let (code, report) = firewall(&root, &["--port", "8741"]);
        assert_eq!(code, 0, "nothing needed is not a failure: {report}");
        assert_eq!(report["ok"], json!(true), "{report}");
        assert_eq!(report["plan"]["rules"], json!([]), "{report}");
        assert_eq!(report["commands"], json!([]), "{report}");
        assert_eq!(report["observation"]["state"], json!("present"));
        assert_eq!(report["platform"], json!(std::env::consts::OS));
        assert!(
            report.get("blocked_recently").is_none(),
            "an empty plan reads no kernel log: {report}"
        );

        let (code, out, err) = run_in(&root, &["mesh", "firewall"], "");
        assert_eq!(code, 0, "{err}");
        assert!(out.starts_with("backend     "), "{out}");
        assert!(out.contains("\nplan        0 rule(s)\n"), "{out}");
        assert!(!out.contains("commands    "), "{out}");
        assert!(
            out.trim_end()
                .lines()
                .last()
                .is_some_and(|l| l.starts_with("verdict     ") && l.contains("needs no inbound")),
            "{out}"
        );

        let (code, applied) = firewall(&root, &["apply"]);
        assert_eq!(code, 10, "a refusal exits 10: {applied}");
        assert_eq!(applied["ok"], json!(false));
        assert_eq!(applied["applied"], json!([]), "nothing ran: {applied}");
        assert!(
            applied["refused"]
                .as_str()
                .unwrap_or_default()
                .ends_with("nothing to apply"),
            "{applied}"
        );
        assert_eq!(applied["observation"], applied["before"]["observation"]);

        let (code, out, err) = run_in(&root, &["mesh", "firewall", "apply", "--port", "8741"], "");
        assert_eq!(code, 10, "{err}");
        assert!(out.starts_with("refused     "), "{out}");
        assert!(
            out.trim_end()
                .ends_with("verdict     the host firewall does not admit the mesh (see above)"),
            "{out}"
        );

        // The command line renders a capability's answer and adds nothing to it: the
        // capability, asked in this process over the same repository, refuses the same way.
        let app = common::load_app(&f);
        let answer = |id: &str| {
            app.context
                .execute(id, json!({ "port": 8741 }))
                .unwrap_or_else(|e| panic!("{id} refused: {e:?}"))
        };
        let needed = answer("mesh.firewall");
        assert_eq!(needed["ok"], json!(true), "{needed}");
        assert_eq!(needed["plan"]["rules"], json!([]), "{needed}");
        let refused = answer("mesh.firewall.apply");
        assert_eq!(refused["ok"], json!(false), "{refused}");
        assert_eq!(refused["applied"], json!([]), "{refused}");
        assert_eq!(refused["refused"], applied["refused"], "{refused}");
    }
}

/// The report over an enabled declaration, on whatever host runs the suite. `mesh firewall`
/// only reads: it asks the firewall for its status and the kernel log for its last five
/// minutes, as the invoking user, and what it cannot read it reports as unobservable. So
/// the plan and the shape of the answer are held here on every machine, and the verdict —
/// which is a fact of this machine's firewall — is only required to agree with itself.
#[test]
fn an_enabled_declaration_derives_the_group_rule_and_the_report_agrees_with_its_exit_code() {
    let f = Fixture::new();
    f.write(".ai/repo/mesh/majordomus.yaml", ENABLED_MULTICAST);
    f.commit("declare the mesh");
    let root = f.root();

    let (code, report) = firewall(&root, &[]);
    let rules = report["plan"]["rules"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(rules.len(), 1, "{report}");
    assert_eq!(rules[0]["role"], json!("multicast"));
    assert_eq!(rules[0]["protocol"], json!("udp"));
    assert_eq!(rules[0]["port"], json!(7741));
    assert_eq!(rules[0]["destination"], json!("239.255.77.77"));
    assert_eq!(rules[0]["sources"], json!([]));
    assert_eq!(report["plan"]["hub_ports"], json!([]));
    assert_eq!(report["window_seconds"], json!(300));
    let state = report["observation"]["state"].as_str().unwrap_or_default();
    assert!(
        [
            "no_backend",
            "inactive",
            "present",
            "missing",
            "unobservable"
        ]
        .contains(&state),
        "{report}"
    );
    let ok = report["ok"] == json!(true);
    assert_eq!(code, if ok { 0 } else { 10 }, "{report}");
    if state == "missing" {
        assert!(!ok, "a rule observed missing fails the report: {report}");
    }

    // A server beyond loopback is dialed on its port, so the port is admitted too.
    let (_, report) = firewall(&root, &["--port", "8741"]);
    let roles: Vec<&str> = report["plan"]["rules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["role"].as_str())
        .collect();
    assert_eq!(roles, ["multicast", "link"], "{report}");
}

/// I2133: a rendezvous answers a refused caller with the refusal alone, and an admitted one
/// with this node and the trusted nodes of the caller's own repositories — never the
/// strangers it merely observed, nor another repository's topology.
#[test]
fn a_registration_learns_only_the_trusted_nodes_of_its_own_repository() {
    let dir = tempfile::tempdir().unwrap();
    let key = |name: &str| {
        NodeIdentity::load_or_create(&dir.path().join(format!("{name}.json"))).unwrap()
    };
    let (hub_id, trusted_id, stranger_id, caller_id, other_id, untrusted_id) = (
        key("hub"),
        key("trusted"),
        key("stranger"),
        key("caller"),
        key("other"),
        key("untrusted"),
    );
    let mut config = MeshConfig {
        schema: "mesh/v1".into(),
        kind: "mesh-declaration".into(),
        id: "hub".into(),
        enabled: true,
        multicast: MulticastConfig {
            enabled: false,
            ..MulticastConfig::default()
        },
        broadcast: Default::default(),
        rendezvous: Default::default(),
        trust: Default::default(),
        cooperation: Default::default(),
    };
    // the hub lists its own key beside the trusted nodes': a machine its trust list does not
    // name keeps its mesh closed and admits nobody. The two callers are trusted too; what an
    // untrusted caller learns is asked of `untrusted` below (ADR 0220 decision 5, I3826)
    config.trust.allow = vec![
        hub_id.public.public_key.clone(),
        trusted_id.public.public_key.clone(),
        caller_id.public.public_key.clone(),
        other_id.public.public_key.clone(),
    ];
    let hub_id_pk = hub_id.public.public_key.clone();
    let hub = MeshRuntime::new();
    hub.activate(&config, hub_id, vec!["127.0.0.1:1".into()], vec![], "test")
        .unwrap();
    let envelope = |id: &NodeIdentity, repos: &[&str], port: u16| {
        let repos: Vec<String> = repos.iter().map(|r| r.to_string()).collect();
        serde_json::to_value(advertise(
            id,
            1,
            &[format!("127.0.0.1:{port}")],
            &["http".into()],
            &repos,
            "n",
        ))
        .unwrap()
    };
    // the hub knows a trusted node and a stranger of repository r1
    assert!(
        hub.register(&envelope(&trusted_id, &["r1"], 2), "test")
            .accepted
    );
    assert!(
        hub.register(&envelope(&stranger_id, &["r1"], 3), "test")
            .accepted
    );

    // a caller whose envelope is refused learns nothing
    let refused = hub.register(&json!({ "v": 0 }), "test");
    assert!(!refused.accepted);
    assert!(refused.candidates.is_empty(), "{:?}", refused.candidates);

    // the keys an answer names, in a stable order
    let learned = |candidates: &[majordomus_cli::mesh::protocol::Envelope]| {
        let mut keys: Vec<String> = candidates.iter().map(|c| c.adv.pk.clone()).collect();
        keys.sort();
        keys
    };
    let sorted = |keys: &[&String]| {
        let mut keys: Vec<String> = keys.iter().map(|k| (*k).clone()).collect();
        keys.sort();
        keys
    };

    // a trusted caller of r1 learns the hub and the trusted nodes of r1, itself among them,
    // and never the stranger
    let same = hub.register(&envelope(&caller_id, &["r1"], 4), "test");
    assert!(same.accepted);
    assert_eq!(
        learned(&same.candidates),
        sorted(&[
            &hub_id_pk,
            &trusted_id.public.public_key,
            &caller_id.public.public_key
        ]),
    );

    // a trusted caller of another repository learns the hub and itself, no node of r1
    let other = hub.register(&envelope(&other_id, &["r2"], 5), "test");
    assert!(other.accepted);
    assert_eq!(
        learned(&other.candidates),
        sorted(&[&hub_id_pk, &other_id.public.public_key]),
    );

    // a caller of r1 whose key nobody trusts is answered, and learns the hub alone: who
    // trusts whom is told only to those trusted (ADR 0220 decision 5, I3826)
    let unknown = hub.register(&envelope(&untrusted_id, &["r1"], 6), "test");
    assert!(unknown.accepted);
    let keys: Vec<String> = unknown
        .candidates
        .iter()
        .map(|c| c.adv.pk.clone())
        .collect();
    assert_eq!(
        keys,
        vec![hub_id_pk.clone()],
        "an untrusted caller learned {keys:?}"
    );
    hub.stop();
}

/// A discovery provider that floods the manager: it hands over `total` copies of this node's
/// own signed advertisement as fast as the queue takes them, counting what the queue took and
/// what it dropped. Each copy the manager reads costs it a full parse and signature check
/// before it is recognised as this node's echo, so the flood outruns the manager by orders of
/// magnitude — what a segment shouting datagrams does to a real one.
struct Flood {
    total: usize,
    accepted: std::sync::Arc<std::sync::atomic::AtomicU64>,
    dropped: std::sync::Arc<std::sync::atomic::AtomicU64>,
    done: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl majordomus_cli::mesh::MeshProvider for Flood {
    fn id(&self) -> &'static str {
        "flood"
    }

    fn start(
        &mut self,
        ctx: &majordomus_cli::mesh::ProviderContext,
    ) -> Result<(), majordomus_cli::mesh::MeshError> {
        use std::sync::atomic::Ordering;
        let bytes = majordomus_cli::mesh::protocol::encode(&ctx.beacon.next_envelope())?;
        let tx = ctx.tx.clone();
        let (total, accepted, dropped, done) = (
            self.total,
            std::sync::Arc::clone(&self.accepted),
            std::sync::Arc::clone(&self.dropped),
            std::sync::Arc::clone(&self.done),
        );
        std::thread::spawn(move || {
            for _ in 0..total {
                let sent = tx.send(majordomus_cli::mesh::Observation {
                    source: majordomus_cli::mesh::MeshSource::UdpMulticast,
                    path: "flood".into(),
                    bytes: bytes.clone(),
                });
                // counted after the queue answered, so a reader never sees more accepted
                // than the queue took
                if sent {
                    accepted.fetch_add(1, Ordering::SeqCst);
                } else {
                    dropped.fetch_add(1, Ordering::SeqCst);
                }
            }
            done.store(true, Ordering::SeqCst);
        });
        Ok(())
    }

    fn status(&self) -> majordomus_cli::mesh::ProviderStatus {
        majordomus_cli::mesh::ProviderStatus {
            id: "flood".into(),
            state: majordomus_cli::mesh::provider::MeshProviderState::Running,
            detail: None,
            sent: 0,
            received: 0,
        }
    }
}

/// A discovery flood costs the process a bounded queue and a counter, and the counter is what
/// the mesh status answers (I2133): every observation beyond `OBSERVATION_QUEUE` is dropped at
/// the provider, never more than the bound waits for the manager at any moment, and each drop
/// is the `overflow` refusal `mesh.status` reports — through the capability and through its
/// HTTP route, the same number the provider counted.
#[test]
fn discovery_flood_bounded_test() {
    use majordomus_cli::mesh::provider::OBSERVATION_QUEUE;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    let f = Fixture::new();
    let app = common::load_app(&f);
    let mesh = &app.context.mesh;
    let config: MeshConfig = serde_json::from_value(json!({
        "schema": "mesh/v1", "kind": "mesh-declaration", "id": "majordomus", "enabled": true,
        "multicast": { "enabled": false }, "trust": { "policy": "tofu" },
    }))
    .unwrap();
    mesh.activate_as(
        &config,
        Arc::new(NodeIdentity::ephemeral().unwrap()),
        "0000000000000001",
        vec![],
        vec![],
        "test",
    )
    .expect("the mesh activates");
    assert_eq!(mesh.status().refusals.overflow, 0, "nothing dropped yet");

    let total = 50 * OBSERVATION_QUEUE;
    let accepted = Arc::new(AtomicU64::new(0));
    let dropped = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicBool::new(false));
    mesh.attach(Box::new(Flood {
        total,
        accepted: Arc::clone(&accepted),
        dropped: Arc::clone(&dropped),
        done: Arc::clone(&done),
    }))
    .expect("the flood starts");

    // while the flood runs and after it: what the queue took, less what the manager has
    // finished reading, is at most the bound — plus the one observation being read. Read in
    // this order (the queue's count first) the difference can only understate the queue.
    let bound = OBSERVATION_QUEUE as u64 + 1;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let finished = done.load(Ordering::SeqCst);
        let took = accepted.load(Ordering::SeqCst);
        let read = mesh.status().refusals.self_heard;
        assert!(
            took.saturating_sub(read) <= bound,
            "{took} observations taken and {read} read: the queue held more than its bound of {OBSERVATION_QUEUE}"
        );
        if finished {
            break;
        }
        assert!(Instant::now() < deadline, "the flood did not end");
        std::thread::sleep(Duration::from_millis(1));
    }
    let took = accepted.load(Ordering::SeqCst);
    let lost = dropped.load(Ordering::SeqCst);
    assert_eq!(
        took + lost,
        total as u64,
        "every observation was taken or dropped"
    );
    assert!(
        lost > 0,
        "{total} observations at once overflowed nothing: the flood never filled the queue"
    );

    // the manager drains what the queue took, and nothing more
    let deadline = Instant::now() + Duration::from_secs(60);
    while mesh.status().refusals.self_heard < took {
        assert!(
            Instant::now() < deadline,
            "the manager did not drain the queue: {} of {took} read",
            mesh.status().refusals.self_heard
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let status = mesh.status();
    assert_eq!(status.refusals.self_heard, took, "{:?}", status.refusals);
    assert_eq!(
        status.refusals.overflow, lost,
        "the status counts every drop the provider saw"
    );

    // and the answer a client reads says the same: the capability and its HTTP route
    let answer = app
        .context
        .execute("mesh.status", json!({}))
        .expect("mesh.status answers");
    assert_eq!(answer["refusals"]["overflow"], json!(lost), "{answer}");
    let router = majordomus_cli::http::Router::new(app.context.clone(), "test");
    let response = router.handle(&majordomus_cli::http::Request::parse_target(
        "GET",
        "/api/v1/mesh",
        vec![],
    ));
    assert_eq!(response.status, 200);
    let routed: Value = serde_json::from_str(&response.body.text()).expect("a JSON status");
    assert_eq!(routed["refusals"]["overflow"], json!(lost), "{routed}");
    mesh.stop();
}
