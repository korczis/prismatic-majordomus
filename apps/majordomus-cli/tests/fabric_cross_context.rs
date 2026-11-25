//! Majordomus Fabric, the legacy baseline of gate G4 (scoped authority): today a caller on
//! this machine is authorised by its address, and by nothing that names the checkout it
//! works in.
//!
//! Every checkout runs its own server on a loopback port (ADR 0003, ADR 0044). The router
//! refuses a state-changing request from another host (ADR 0126, `remote_callers.rs`) and
//! from a browser page on another origin (`mcp_conformance_http.rs`). A plain client on
//! loopback (a CLI, an MCP client, a script) gets every capability of every checkout whose
//! port it can reach: nothing in its request says which checkout it belongs to, and so
//! nothing can refuse it for belonging to another.
//!
//! This file pins that, so that the change which ends it is seen. It is a characterization,
//! not the rule: the owner decided on 2026-11-24 (knowledge record
//! `fabric-starts-with-p00-s00-s01`, answer 4) that local trust is one OS user over a socket
//! with mode 0600, and that every request is authorised against the context and capability it
//! names (ADR 0199, amendment of 2026-11-24). When the host binds a session to its context,
//! these assertions invert: the write into the other checkout is refused, and this file is
//! rewritten to prove the refusal on every projection (Fabric S04, matrix case 1).

mod common;

use common::{Fixture, Served};
use serde_json::{json, Value};

const ISSUE: &str = ".ai/repo/project/issues/I0001.yaml";

/// A server whose per-user state (the mesh's node identity) lives beside its fixture.
fn serve(f: &Fixture) -> Served {
    let state = f.parent().join("state");
    Served::start_with_env(
        &f.root(),
        &[],
        &[("XDG_STATE_HOME", state.to_str().unwrap())],
    )
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn status_of(served: &Served, issue: &str) -> String {
    let (status, answer) = served.get(&format!("/api/v1/plan/record?id={issue}"));
    assert_eq!(status, 200, "{answer}");
    answer["issue"]["status"]
        .as_str()
        .unwrap_or_else(|| panic!("plan.record carries no status: {answer}"))
        .to_string()
}

const INVERTS: &str = "this is the legacy baseline of Fabric gate G4: when a session is bound \
                       to its context, the write into another checkout must be refused, and \
                       this test is rewritten to prove that refusal";

#[test]
fn today_a_loopback_client_of_one_checkout_changes_another_checkouts_plan() {
    // two unrelated checkouts on one machine, one OS user, each with its own server
    let a = Fixture::new();
    let b = Fixture::new();
    let a_before = std::fs::read_to_string(a.root().join(ISSUE)).expect("A's issue record");
    let b_before = std::fs::read_to_string(b.root().join(ISSUE)).expect("B's issue record");
    let served_a = serve(&a);
    let served_b = serve(&b);
    assert_ne!(
        served_a.address, served_b.address,
        "one server per checkout"
    );
    assert_eq!(status_of(&served_b, "I0001"), "READY");

    // A client of checkout A addresses B's port. Its request names an issue and a move,
    // and nothing about the checkout it works in: the request has no field for it.
    let (status, _, answer) = served_b.request(
        "POST",
        "/api/v1/plan/transition",
        Some(r#"{"issue":"I0001","transition":"start"}"#),
    );
    assert_eq!(status, 200, "{INVERTS}: {answer}");
    let moved: Value = serde_json::from_str(&answer).expect("a JSON answer");
    assert_eq!(moved["from"], "READY", "{moved}");

    // B's repository carries the move; A's is untouched
    let b_after = std::fs::read_to_string(b.root().join(ISSUE)).expect("B's issue record");
    assert_ne!(b_after, b_before, "{INVERTS}: B's record did not change");
    assert!(b_after.contains("started_at"), "{b_after}");
    let a_after = std::fs::read_to_string(a.root().join(ISSUE)).expect("A's issue record");
    assert_eq!(
        a_after, a_before,
        "the write landed in the caller's own checkout"
    );
}

#[test]
fn today_an_mcp_session_opened_on_another_checkouts_server_writes_there() {
    let a = Fixture::new();
    let b = Fixture::new();
    let b_before = std::fs::read_to_string(b.root().join(ISSUE)).expect("B's issue record");
    let a_before = std::fs::read_to_string(a.root().join(ISSUE)).expect("A's issue record");
    let served_b = serve(&b);

    // an MCP client that says, in its own words, that it works in checkout A
    let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": { "name": format!("client-of {}", a.root().display()), "version": "0" } } });
    let (status, headers, body) = served_b.request("POST", "/mcp", Some(&initialize.to_string()));
    assert_eq!(status, 200, "{body}");
    let session = header(&headers, "mcp-session-id")
        .expect("a session id: the server opened one for a client of another checkout")
        .to_string();

    // a tool that declares a repository mutation runs in B for it
    let call = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
        "name": "majordomus_plan_transition",
        "arguments": { "issue": "I0001", "transition": "start" } } });
    let (status, _, body) = served_b.request_with(
        "POST",
        "/mcp",
        Some(&call.to_string()),
        &[("Mcp-Session-Id", &session)],
    );
    assert_eq!(status, 200, "{body}");
    let answer: Value = serde_json::from_str(&body).expect("a JSON answer");
    assert!(
        answer["error"].is_null() && answer["result"]["isError"] != json!(true),
        "{INVERTS}: {answer}"
    );
    let b_after = std::fs::read_to_string(b.root().join(ISSUE)).expect("B's issue record");
    assert_ne!(b_after, b_before, "{INVERTS}: B's record did not change");
    let a_after = std::fs::read_to_string(a.root().join(ISSUE)).expect("A's issue record");
    assert_eq!(
        a_after, a_before,
        "the write landed in the caller's own checkout"
    );

    // the client leaves as a client should, so that the server's stop does not wait for it
    let (status, _, body) =
        served_b.request_with("DELETE", "/mcp", None, &[("Mcp-Session-Id", &session)]);
    assert!(status < 300, "the session was not closed: {status} {body}");
}
