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

// claims: mcp-lease-lost

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

/// A ping on an open session, and its status.
fn ping(url: &str, session: &str) -> u16 {
    let body = json!({ "jsonrpc": "2.0", "id": 9, "method": "ping" }).to_string();
    majordomus_cli::mcp::bridge::request(
        url,
        "POST",
        "/mcp",
        &[
            ("content-type", "application/json"),
            ("mcp-session-id", session),
        ],
        Some(&body),
        std::time::Duration::from_secs(5),
    )
    .expect("the server answers /mcp")
    .status
}

/// The id of a session opened now.
fn session_id(url: &str) -> String {
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
    reply
        .header("mcp-session-id")
        .expect("a session id")
        .to_string()
}

#[test]
fn a_server_that_lost_its_lease_says_so_and_takes_on_nobody_new() {
    // the grace a superseded server keeps its sessions for, declared before anything reads it
    lease::declare_timings(&majordomus_cli::policy::ServerPolicy {
        busy_grace_seconds: Some(1),
        ..Default::default()
    });
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
        before[LEASEHOLDER_KEY],
        json!(true),
        "a server nothing has taken a lease from claims to be the one: {before}"
    );
    assert!(
        lease::probe(&url, repo.root()),
        "the probe accepts the server it is looking for"
    );

    // a client that arrives now is served, and its session is the board's
    let session = session_id(&url);
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
        after[LEASEHOLDER_KEY],
        json!(false),
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

    // its health says that what the server check describes is another process
    let report = app
        .context
        .execute("health.report", json!({}))
        .expect("health.report answers");
    let server_check = report["checks"]
        .as_array()
        .expect("the report carries checks")
        .iter()
        .find(|c| c["id"] == "server")
        .expect("the report carries the server check")
        .clone();
    assert_eq!(server_check["status"], "warn", "{server_check}");
    assert!(
        server_check
            .to_string()
            .contains("what is described above is another process"),
        "the check names the process it describes as another one: {server_check}"
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

    // and the session it already had is still its own for the grace: a request in flight
    // and the ones right behind it are answered where they started
    assert_eq!(
        endpoint.active(),
        kept,
        "an open session is not closed by the takeover"
    );
    assert_eq!(ping(&url, &session), 200, "served within the grace");

    // after the grace it is handed on: answered 404, which the bridge answers by opening a
    // session, being refused 409 and electing the current server (I2127)
    std::thread::sleep(std::time::Duration::from_millis(1200));
    assert_eq!(
        ping(&url, &session),
        404,
        "the session is handed on after the grace"
    );
    assert_eq!(
        endpoint.active(),
        0,
        "and this server holds no session any more"
    );

    running.stop();
}
