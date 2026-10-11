//! No answer, log line or mesh frame carries a credential the redactor knows (I2161).
//!
//! An error echoes what it was asked — a path, a tool name, an argument — and a handover is
//! replicated in clear to every linked runtime. Each of those now passes the one redaction rule
//! set (`redaction::redact_secrets`, the port of the capture table) before it leaves the process.

mod common;

use std::sync::Arc;

use common::Fixture;
use majordomus_cli::http::mcp::McpEndpoint;
use majordomus_cli::http::{Request, Router};
use serde_json::{json, Value};

/// A credential's shape, assembled at run time so that no committed file carries one.
fn token() -> String {
    format!("{}{}", "ghp_", "a".repeat(36))
}

#[test]
fn an_http_error_does_not_echo_a_credential() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let router = Router::new(app.context.clone(), "test");
    let path = format!("/api/v1/{}", token());
    let response = router.handle(&Request::parse_target("GET", &path, vec![]));
    assert_eq!(response.status, 404);
    let text = response.body.text();
    assert!(!text.contains(&token()), "{text}");
    assert!(text.contains("[redacted:github-token]"), "{text}");
}

#[test]
fn an_mcp_error_does_not_echo_a_credential() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let endpoint = Arc::new(McpEndpoint::new(
        app.context.clone(),
        "test",
        "http://127.0.0.1:0".into(),
    ));
    let post = |session: Option<&str>, message: Value| {
        let mut req = Request::parse_target("POST", "/mcp", message.to_string().into_bytes());
        if let Some(id) = session {
            req = req.with_headers(vec![("Mcp-Session-Id".into(), id.into())]);
        }
        endpoint.handle(&req)
    };
    let opened = post(
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "unit", "version": "0" } } }),
    );
    let session = opened
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("mcp-session-id"))
        .map(|(_, v)| v.clone())
        .expect("a session");
    let reply = post(
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": token(), "arguments": {} } }),
    );
    let text = reply.body.text();
    assert!(!text.contains(&token()), "{text}");
}

#[test]
fn a_handover_body_is_redacted_before_it_is_signed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("20261009T120000Z--x.md");
    std::fs::write(
        &path,
        format!(
            "---\ntask_id: t-1\n---\n\n# Objective\nship it\n# Current State\npushed with {}\n# Next Action\nnone\n",
            token()
        ),
    )
    .unwrap();
    let body = majordomus_cli::mesh::handover::to_body(&path, None, None).unwrap();
    assert!(!body.body.contains(&token()), "{}", body.body);
    assert!(body.body.contains("[redacted:github-token]"));
    assert_eq!(
        body.id,
        majordomus_cli::mesh::journal::HandoverBody::digest_of(&body.body),
        "the id names the body that travels"
    );
}
