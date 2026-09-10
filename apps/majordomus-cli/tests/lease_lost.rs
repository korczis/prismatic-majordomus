//! A server that has lost its lease: what it still does, what it stops doing, and how a
//! client with a remembered address can tell it from the one that holds the lease.
//!
//! On 2026-09-10 this checkout had two servers. One held the lease; the other had been
//! superseded by a rebuild, kept its socket, kept its peer board and kept answering `GET /`
//! with the same name and the same `repository_id` as the current server — from a different
//! generation of the layer. A session whose environment carried the older address read that
//! board and saw none of the peers anybody else could see, and nothing in any answer said
//! so. Every liveness test passed, because none of them asked the only question that
//! separates the two.
//!
//! The whole file is one test on purpose: [`majordomus_cli::lease::lost`] records a fact
//! about the process that is never unmade — a lease taken over is not given back — so the
//! before and the after cannot be two tests sharing a binary.

mod common;

use std::sync::Arc;

use common::Fixture;
use majordomus_cli::http::mcp::McpEndpoint;
use majordomus_cli::http::server;
use majordomus_cli::http::Router;
use majordomus_cli::lease::{self, LEASEHOLDER_KEY};
use majordomus_cli::Repository;
use serde_json::{json, Value};

fn init() -> Value {
    json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "unit", "version": "0", "title": "Unit" }
        }
    })
}

/// `GET /` of a running server, parsed.
fn index(url: &str) -> Value {
    let reply = majordomus_cli::mcp::bridge::request(
        url,
        "GET",
        "/",
        &[],
        None,
        std::time::Duration::from_secs(5),
    )
    .expect("the server answers its index");
    assert_eq!(reply.status, 200);
    serde_json::from_str(&reply.body).expect("the index is JSON")
}

/// POST an `initialize` with no session header, and hand back the status and the body.
fn open_session(url: &str) -> (u16, Value) {
    let body = init().to_string();
    let reply = majordomus_cli::mcp::bridge::request(
        url,
        "POST",
        "/mcp",
        &[("content-type", "application/json")],
        Some(&body),
        std::time::Duration::from_secs(5),
    )
    .expect("the server answers /mcp");
    let value = serde_json::from_str(&reply.body).unwrap_or(Value::Null);
    (reply.status, value)
}

#[test]
fn a_server_that_lost_its_lease_says_so_and_takes_on_nobody_new() {
    let f = Fixture::new();
    let repo = Repository::discover(&f.root()).expect("the fixture is a repository");
    let app = common::load_app(&f);
    let bound = server::bind("127.0.0.1", 0).expect("a free loopback port");
    let url = bound.url();
    let endpoint = Arc::new(McpEndpoint::new(app.context.clone(), "test", url.clone()));
    let router = Router::new(app.context.clone(), "test").with_mcp(Arc::clone(&endpoint));
    let running = bound.start(router);

    // --- while it is the checkout's server

    let before = index(&url);
    assert_eq!(before["name"], "majordomus");
    assert_eq!(
        before[LEASEHOLDER_KEY], json!(true),
        "a server nothing has taken a lease from claims to be the one: {before}"
    );
    assert!(
        lease::probe(&url, repo.root()),
        "the probe accepts the server it is looking for"
    );

    // a client that arrives now is served, and its session is the board's
    let (status, reply) = open_session(&url);
    assert_eq!(status, 200, "a new client is taken on: {reply}");
    let kept = endpoint.active();
    assert_eq!(kept, 1, "one session open");

    // --- the lease is taken over by another process

    lease::lost();
    assert!(lease::was_lost());
    assert!(
        lease::held().is_none(),
        "the process stops claiming a lease it no longer has"
    );

    // --- from here on it is not the checkout's server, and says so

    let after = index(&url);
    assert_eq!(
        after[LEASEHOLDER_KEY], json!(false),
        "the one field that separates it from the current server: {after}"
    );
    assert_eq!(
        after["repository_id"], before["repository_id"],
        "everything else is as true of it as before — which is the whole difficulty"
    );
    assert!(
        !lease::probe(&url, repo.root()),
        "a client with a remembered address is not answered as though this were the one"
    );

    // it takes on nobody new, and the refusal names what to do instead
    let (status, reply) = open_session(&url);
    assert_eq!(status, 409, "a new client is refused: {reply}");
    let text = reply.to_string();
    assert!(
        text.contains("lease_lost"),
        "the refusal carries a stable code: {text}"
    );
    assert!(
        text.contains("launcher"),
        "and says how to reach the current server: {text}"
    );

    // and the session it already had is still its own: it serves the peers it has
    assert_eq!(
        endpoint.active(),
        kept,
        "an open session is not closed by the takeover"
    );

    running.stop();
}
