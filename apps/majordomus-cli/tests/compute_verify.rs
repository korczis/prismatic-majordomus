//! The server recomputes a browser's claim with the portable core (I2785, ADR 0170).
//!
//! A browser that runs the core as wasm32 returns a claim: the function, the input it
//! computed on, the result it got. `compute.verify` recomputes it with the same core this
//! executable links and answers `match`, `mismatch` or `invalid`. These tests replay every
//! row of the core's fixture — the digests Python computed, outside the code under test —
//! over a real socket, show that a caller on another host may ask (a verification is a
//! read, ADR 0126), and that asking writes nothing anywhere.
//!
//! They prove the criterion `a-browser-computes-what-the-server-verifies#the-server-recomputes-a-claim`.

mod common;

use std::net::IpAddr;
use std::sync::Arc;

use common::{Fixture, Served};
use majordomus_cli::http::mcp::McpEndpoint;
use majordomus_cli::http::{Request, Router};
use serde_json::{json, Value};

/// The fixture every runtime of the core is held to.
fn fixture() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../majordomus-core/fixtures/canonical-digest.v1.json");
    serde_json::from_str(&std::fs::read_to_string(&path).expect("the core's fixture"))
        .expect("the fixture is JSON")
}

fn verify(s: &Served, input: &str, claimed: &str) -> Value {
    let (status, _, body) = s.request(
        "POST",
        "/api/v1/compute/verify",
        Some(
            &json!({ "function": "canonical_digest", "input": input, "claimed": claimed })
                .to_string(),
        ),
    );
    assert_eq!(status, 200, "{body}");
    serde_json::from_str(&body).expect("the answer is JSON")
}

#[test]
fn every_row_of_the_core_fixture_is_judged_over_http() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let fixture = fixture();
    let cases = fixture["cases"].as_array().expect("cases");
    assert!(cases.len() >= 10, "the fixture lost rows: {}", cases.len());

    for case in cases {
        let (id, input) = (&case["id"], case["input"].as_str().expect("input"));
        let digest = case["digest"].as_str().expect("digest");

        let right = verify(&s, input, digest);
        assert_eq!(right["verdict"], "match", "{id}: {right}");
        assert_eq!(right["recomputed"], digest, "{id}: {right}");

        // one hex digit changed: a wrong claim, whoever made it
        let last = if digest.ends_with('0') { "1" } else { "0" };
        let wrong = format!("{}{last}", &digest[..63]);
        let answer = verify(&s, input, &wrong);
        assert_eq!(answer["verdict"], "mismatch", "{id}: {answer}");
        assert_eq!(answer["recomputed"], digest, "{id}: {answer}");
    }

    for case in fixture["invalid"].as_array().expect("invalid rows") {
        let answer = verify(&s, case["input"].as_str().expect("input"), &"0".repeat(64));
        assert_eq!(answer["verdict"], "invalid", "{}: {answer}", case["id"]);
        assert!(
            answer["reason"]
                .as_str()
                .is_some_and(|r| r.contains("not JSON")),
            "{answer}"
        );
    }
}

#[test]
fn a_caller_on_another_host_may_ask_and_asking_writes_nothing() {
    let f = Fixture::new();
    let before = f.snapshot();
    let app = common::load_app(&f);
    let router = Router::new(app.context.clone(), "test");
    let input = r#"{"b":2,"a":1}"#;
    let digest = majordomus_core::digest_of_json_text(input).expect("JSON");
    let request = Request::bind(
        majordomus_cli::capability::HttpMethod::Post,
        "/api/v1/compute/verify",
        &json!({ "input": input, "claimed": digest }),
    )
    .with_remote(Some("192.168.100.30".parse::<IpAddr>().unwrap()));
    let response = router.handle(&request);
    assert_eq!(response.status, 200, "{}", response.body.text());
    let answer: Value = serde_json::from_str(&response.body.text()).expect("JSON");
    assert_eq!(answer["verdict"], "match", "{answer}");
    assert_eq!(
        answer["function"], "canonical_digest",
        "the function defaults: {answer}"
    );
    assert_eq!(
        f.snapshot(),
        before,
        "a verification wrote to the repository"
    );

    let s = Served::start(&f.root(), &[]);
    verify(&s, input, &digest);
    let (status, executions) = s.get("/api/v1/executions");
    assert_eq!(status, 200, "{executions}");
    assert_eq!(
        executions["remembered"], 0,
        "a verification is not an execution: {executions}"
    );
    assert_eq!(
        f.snapshot(),
        before,
        "a verification wrote to the repository"
    );
}

/// An MCP message from `address`, in the session `session` names, through the endpoint.
fn mcp(endpoint: &McpEndpoint, session: Option<&str>, message: Value) -> (Value, Option<String>) {
    let mut req = Request::parse_target("POST", "/mcp", message.to_string().into_bytes())
        .with_remote(Some("192.168.100.30".parse::<IpAddr>().unwrap()));
    if let Some(id) = session {
        req = req.with_headers(vec![("Mcp-Session-Id".into(), id.into())]);
    }
    let response = endpoint.handle(&req);
    let id = response
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("mcp-session-id"))
        .map(|(_, v)| v.clone());
    (
        serde_json::from_str(&response.body.text()).unwrap_or(Value::Null),
        id,
    )
}

#[test]
fn an_agent_on_another_host_verifies_a_claim_over_mcp() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let endpoint = Arc::new(McpEndpoint::new(
        app.context.clone(),
        "test",
        "http://127.0.0.1:0".into(),
    ));
    let init = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "compute-verify", "version": "0" } }
    });
    let (_, session) = mcp(&endpoint, None, init);
    let session = session.expect("a session id");
    let fixture = fixture();
    let case = &fixture["cases"][0];
    let (input, digest) = (
        case["input"].as_str().expect("input"),
        case["digest"].as_str().expect("digest"),
    );

    let call = |id: u64, claimed: &str| {
        json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {
            "name": "majordomus_compute_verify",
            "arguments": { "function": "canonical_digest", "input": input, "claimed": claimed } } })
    };
    let (right, _) = mcp(&endpoint, Some(&session), call(2, digest));
    assert_eq!(right["result"]["isError"], json!(false), "{right}");
    let answer = &right["result"]["structuredContent"];
    assert_eq!(answer["verdict"], "match", "{right}");
    assert_eq!(answer["recomputed"], digest, "{right}");

    let (wrong, _) = mcp(&endpoint, Some(&session), call(3, &"0".repeat(64)));
    assert_eq!(
        wrong["result"]["structuredContent"]["verdict"], "mismatch",
        "{wrong}"
    );
}
