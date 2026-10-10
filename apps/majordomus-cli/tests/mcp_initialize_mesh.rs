//! Every MCP session is told the mesh at `initialize` (I2287, ADR 0128).
//!
//! A client of any provider — Claude Code, Codex, Gemini, anything that speaks MCP — reads the
//! `initialize` instructions before its first call. On a server whose mesh runs they must say
//! who works where and what waits here, and name the calls of the protocol; where no mesh is
//! declared they say nothing about one.

mod common;

use std::time::{Duration, Instant};

use common::{Fixture, Served};
use serde_json::{json, Value};

use majordomus_cli::mesh::identity::NodeIdentity;

fn initialize(s: &Served) -> String {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": { "name": "codex", "version": "0" } } })
    .to_string();
    let (status, headers, text) = s.request("POST", "/mcp", Some(&body));
    assert_eq!(status, 200, "{text}");
    let v: Value = serde_json::from_str(&text).expect("an initialize answer");
    // the session is ended, as a client leaving does, so the server does not wait on it
    if let Some((_, id)) = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("mcp-session-id"))
    {
        s.request_with("DELETE", "/mcp", None, &[("Mcp-Session-Id", id)]);
    }
    v["result"]["instructions"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn a_client_is_told_the_mesh_at_initialize_and_only_where_one_runs() {
    let f = Fixture::new();
    let state = f.parent().join("xdg-state");
    let key = NodeIdentity::load_or_create(&state.join("majordomus").join("node.json"))
        .expect("a node identity")
        .public
        .public_key;
    f.write(
        ".ai/repo/mesh/majordomus.yaml",
        &format!("schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntrust:\n  policy: deny_unknown\n  allow:\n    - {key}\ncooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: one-repository\n"),
    );
    let s = Served::start_with_env(
        &f.root(),
        &["--discovery", "filesystem"],
        &[("XDG_STATE_HOME", state.to_str().unwrap())],
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while s.get("/api/v1/mesh/briefing").1["active"] != json!(true) {
        assert!(Instant::now() < deadline, "the mesh did not start");
        std::thread::sleep(Duration::from_millis(150));
    }
    let told = initialize(&s);
    for word in [
        "This repository's mesh links its runtimes on every machine",
        "majordomus_mesh_claim",
        "majordomus_mesh_handover_consume",
        "majordomus_mesh_review_answer",
        "majordomus_mesh_briefing",
    ] {
        assert!(told.contains(word), "{word} missing from: {told}");
    }

    // a repository that declares no mesh is told nothing about one
    let plain = Fixture::new();
    let p = Served::start(&plain.root(), &[]);
    assert!(!initialize(&p).contains("mesh links"));
}
