//! The shared server's parts, in process and without a child: the peer board, the lease,
//! the `/mcp` endpoint and its sessions, the bridge's HTTP client against a real loopback
//! socket, the protocol's session resumption, and the stdio loop's edge cases.

mod common;

use std::io::Write;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use common::Fixture;
use majordomus_cli::http::mcp::{McpEndpoint, SESSION_HEADER};
use majordomus_cli::http::server;
use majordomus_cli::http::{Request, Router};
use majordomus_cli::lease::{self, Role};
use majordomus_cli::mcp::bridge::{self, Bridge, BridgeError};
use majordomus_cli::mcp::{stdio, Server, Surface};
use majordomus_cli::peers::{ClientInfo, PeerBoard, Transport};
use majordomus_cli::Repository;
use serde_json::{json, Value};

fn init() -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "unit", "version": "0", "title": "Unit" } } })
}

/// A shared server's router and endpoint over a fixture, bound on a free loopback port.
fn bound(f: &Fixture) -> (server::Running, Arc<McpEndpoint>) {
    let app = common::load_app(f);
    let b = server::bind("127.0.0.1", 0).unwrap();
    let endpoint = Arc::new(McpEndpoint::new(app.context.clone(), "test", b.url()));
    let router = Router::new(app.context.clone(), "test").with_mcp(endpoint.clone());
    (b.start(router), endpoint)
}

#[test]
fn the_peer_board_identifies_touches_announces_and_summarises() {
    let board = PeerBoard::new();
    assert!(board.is_empty());
    let a = board.attach(Transport::Stdio);
    let b = board.attach(Transport::Http);
    assert_eq!(board.get(&a).unwrap().client, ClientInfo::unknown());
    board.identify(&a, ClientInfo::from_initialize(&init()["params"]));
    let peer = board.get(&a).unwrap();
    assert_eq!(peer.client.name, "unit");
    assert_eq!(peer.client.title.as_deref(), Some("Unit"));
    assert!(peer.connected_at.ends_with('Z'));
    board.touch(&b);
    assert_eq!(board.get(&b).unwrap().last_seen_seconds_ago, 0);
    let announced = board
        .announce(&b, "writing docs", vec!["docs".into()])
        .unwrap();
    assert_eq!(
        announced.announcement.as_ref().unwrap().intent,
        "writing docs"
    );
    let summary = board.summary();
    assert!(summary.contains("p1 unit (Stdio)"), "{summary}");
    assert!(
        summary.contains("p2 (not initialized) (Http: writing docs)"),
        "{summary}"
    );
    let ghost = board.attach(Transport::Http);
    board.detach(&ghost);
    board.identify(&ghost, ClientInfo::unknown()); // a detached id: a no-op
    assert!(board.announce(&ghost, "x", vec![]).is_none());
    assert_eq!(board.len(), 2);
    board.detach(&a);
    board.detach(&b);
    assert_eq!(board.summary(), "none");
    assert_eq!(
        ClientInfo::from_initialize(&json!({ "clientInfo": { "name": "  " } })),
        ClientInfo::unknown()
    );
}

#[test]
fn the_lease_is_held_probed_published_and_released() {
    let f = Fixture::new();
    let repo = Repository::discover(&f.root()).unwrap();
    let path = lease::lease_path(&repo);
    assert!(path.ends_with(".ai/local/state/mcp/server.json"));
    let Role::Server(held) = lease::elect(&repo).unwrap() else {
        panic!("nobody holds the lease yet")
    };
    assert_eq!(held.root(), f.root());
    assert!(path.exists());
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(doc["schema"], "majordomus-mcp-lease/v1");
    assert!(doc["url"].is_null(), "no URL before the server is bound");

    let (running, _endpoint) = bound(&f);
    let url = running.url();
    assert!(
        !lease::probe(&url, std::path::Path::new("/somewhere/else")),
        "another root is not this server"
    );
    assert!(lease::probe(&url, &f.root()));
    held.publish(&url).unwrap();
    match lease::elect(&repo).unwrap() {
        Role::Peer { url: seen } => assert_eq!(seen, url),
        Role::Server(_) => panic!("a published, answering lease makes the next process a peer"),
    }
    // a file that is no longer this process's is left alone on release
    let mine = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, mine.replace("\"token\":\"", "\"token\":\"other-")).unwrap();
    held.release();
    assert!(
        path.exists(),
        "a lease another process wrote is not removed"
    );
    std::fs::remove_file(&path).unwrap();
    running.stop();

    // an abandoned lease: no URL, and older than the bind grace
    std::fs::write(
        &path,
        json!({ "schema": "majordomus-mcp-lease/v1", "token": "x", "root": f.root(), "url": null })
            .to_string(),
    )
    .unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(600))
        .unwrap();
    let role = lease::elect(&repo).unwrap();
    assert!(
        matches!(role, Role::Server(_)),
        "an abandoned lease is taken over"
    );
    drop(role);
    assert!(!path.exists(), "dropping the lease removes the file");
}

#[test]
fn the_endpoint_opens_reaps_and_closes_sessions() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let endpoint = McpEndpoint::new(app.context.clone(), "test", "http://127.0.0.1:1".into());
    assert_eq!(endpoint.url(), "http://127.0.0.1:1");
    let post = |body: &str, session: Option<&str>| {
        let mut req = Request::parse_target("POST", "/mcp", body.as_bytes().to_vec());
        if let Some(s) = session {
            req = req.with_headers(vec![(SESSION_HEADER.to_string(), s.to_string())]);
        }
        endpoint.handle(&req)
    };
    let opened = post(&init().to_string(), None);
    assert_eq!(opened.status, 200);
    let session = opened
        .headers
        .iter()
        .find(|(k, _)| k == SESSION_HEADER)
        .map(|(_, v)| v.clone())
        .unwrap();
    assert_eq!(app.context.peers.len(), 1);
    assert_eq!(app.context.peers.list()[0].client.name, "unit");
    let batch = post(
        &json!([{ "jsonrpc": "2.0", "method": "notifications/initialized" }, { "jsonrpc": "2.0", "id": 2, "method": "ping" }]).to_string(),
        Some(&session),
    );
    assert_eq!(batch.status, 200);
    let v: Value = serde_json::from_str(&batch.body).unwrap();
    assert_eq!(
        v.as_array().unwrap().len(),
        1,
        "a batch answers with a batch of its requests"
    );
    let missing = endpoint.handle(&Request::parse_target("DELETE", "/mcp", vec![]));
    assert_eq!(missing.status, 400);
    let unknown = endpoint.handle(
        &Request::parse_target("DELETE", "/mcp", vec![])
            .with_headers(vec![(SESSION_HEADER.into(), "nope".into())]),
    );
    assert_eq!(unknown.status, 404);
    let put = endpoint.handle(&Request::parse_target("PUT", "/mcp", vec![]));
    assert_eq!(put.status, 405);
    std::thread::sleep(Duration::from_millis(5));
    assert_eq!(endpoint.reap_idle(Duration::from_secs(60)).len(), 0);
    let gone = endpoint.reap_idle(Duration::ZERO);
    assert_eq!(gone.len(), 1, "an idle session expires");
    assert_eq!(endpoint.active(), 0);
    assert!(app.context.peers.is_empty(), "its peer is gone with it");
    let after = post(
        &json!({ "jsonrpc": "2.0", "id": 3, "method": "ping" }).to_string(),
        Some(&session),
    );
    assert_eq!(after.status, 404);
    let again = post(&init().to_string(), None);
    assert_eq!(again.status, 200);
    endpoint.close_all();
    assert_eq!(endpoint.active(), 0);
    assert!(format!("{endpoint:?}").contains("McpEndpoint"));
}

#[test]
fn the_bridge_client_speaks_to_a_real_socket_and_names_every_failure() {
    let f = Fixture::new();
    let (running, endpoint) = bound(&f);
    let url = running.url();
    assert_eq!(running.address(), url.strip_prefix("http://").unwrap());

    let mut b = Bridge::new(url.clone());
    assert_eq!(b.url(), url);
    assert!(b.client().is_none());
    assert!(
        b.reinitialize().is_ok(),
        "nothing to re-send yet is not an error"
    );
    // a request before initialize: the server wants a session, the bridge reports the refusal
    let err = b
        .handle(&json!({ "jsonrpc": "2.0", "id": 9, "method": "ping" }))
        .unwrap_err();
    assert!(
        matches!(err, BridgeError::Rejected { status: 400, .. }),
        "{err}"
    );
    assert!(err.to_string().contains("rejected"));

    let answer = b.handle(&init()).unwrap().unwrap();
    assert_eq!(answer["result"]["serverInfo"]["name"], "majordomus");
    assert_eq!(b.client().unwrap().name, "unit");
    assert!(b
        .handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
        .unwrap()
        .is_none());
    b.heartbeat().unwrap();
    assert_eq!(endpoint.active(), 1);

    // the server forgot the session: the bridge re-initialises and answers anyway
    endpoint.close_all();
    let answer = b
        .handle(&json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }))
        .unwrap()
        .unwrap();
    assert_eq!(answer["result"], json!({}));
    assert_eq!(endpoint.active(), 1, "a new session was opened silently");

    // the raw client
    let reply = bridge::request(&url, "GET", "/", &[], None, Duration::from_secs(5)).unwrap();
    assert_eq!(reply.status, 200);
    assert!(reply
        .header("content-type")
        .unwrap()
        .starts_with("application/json"));
    assert!(reply.header("X-None").is_none());
    let v: Value = serde_json::from_str(&reply.body).unwrap();
    assert_eq!(v["mcp"], "/mcp");
    assert!(bridge::request(
        "http://nohost.invalid:1",
        "GET",
        "/",
        &[],
        None,
        Duration::from_secs(1)
    )
    .is_err());

    b.close();
    assert_eq!(endpoint.active(), 0, "close ends the session on the server");
    b.close(); // idempotent
    running.stop();

    // the server is gone: unreachable, and a heartbeat without a session is a no-op
    let mut dead = Bridge::new(url.clone());
    dead.heartbeat().unwrap();
    let err = dead.handle(&init()).unwrap_err();
    assert!(matches!(err, BridgeError::Unreachable { .. }), "{err}");
    assert!(err.to_string().contains("unreachable"), "{err}");
    let mut moved = Bridge::new("http://127.0.0.1:1".into());
    moved.move_to(url);
    assert!(moved.handle(&init()).is_err());
}

#[test]
fn a_router_without_an_endpoint_has_no_mcp_route() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let router = Router::new(app.context.clone(), "test");
    let r = router.handle(&Request::parse_target("POST", "/mcp", b"{}".to_vec()));
    assert_eq!(r.status, 404);
    assert!(r.body.contains("no MCP over HTTP"));
    let index = router.handle(&Request::parse_target("GET", "/", vec![]));
    let v: Value = serde_json::from_str(&index.body).unwrap();
    assert!(v.get("mcp").is_none());
    assert_eq!(v["peers"], "/api/v1/peers");
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    let err = match server::bind("127.0.0.1", port) {
        Err(e) => e,
        Ok(_) => panic!("a taken port cannot be bound"),
    };
    assert!(
        err.to_string()
            .contains(&format!("cannot bind 127.0.0.1:{port}")),
        "{err}"
    );
    assert_eq!(err.exit_code(), 13);
    let fallback = server::bind_or_fallback("127.0.0.1", port).unwrap();
    assert_ne!(fallback.address(), format!("127.0.0.1:{port}"));
    assert!(fallback.url().starts_with("http://127.0.0.1:"));
}

#[test]
fn a_session_can_be_resumed_by_another_server() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let surface = Surface::new(app.context.clone());
    let peer = app.context.peers.attach(Transport::Stdio);
    let mut first = Server::new(surface.for_peer(peer.clone()), "test")
        .with_endpoint(Some("http://127.0.0.1:1".into()));
    assert!(!first.is_initialized());
    let answer = first.handle(init()).unwrap().into_value();
    let text = answer["result"]["instructions"].as_str().unwrap();
    assert!(
        text.contains("http://127.0.0.1:1/docs") && text.contains("You are peer p1"),
        "{text}"
    );
    first.handle(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    assert!(first.is_initialized());
    let client = first.client().unwrap().clone();
    assert_eq!(client.name, "unit");

    let mut second = Server::new(surface.for_peer(peer.clone()), "test");
    second.resume(client);
    assert!(second.is_initialized());
    assert_eq!(second.client().unwrap().name, "unit");
    assert_eq!(app.context.peers.get(&peer).unwrap().client.name, "unit");
    assert!(
        !second.instructions().contains("http://"),
        "no endpoint, no URL"
    );
    assert_eq!(surface.peer(), None);
    assert!(format!("{:?}", surface).contains("Surface"));
}

#[test]
fn the_stdio_loop_survives_bad_bytes_and_stops_on_a_broken_pipe() {
    let input: &[u8] = b"\xff\xfe not utf-8\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n";
    let mut out = Vec::new();
    let n = stdio::serve(input, &mut out, |m| {
        Some(json!({ "jsonrpc": "2.0", "id": m["id"], "result": {} }))
    })
    .unwrap();
    assert_eq!(
        n, 1,
        "one message answered; the parse error is written but not counted"
    );
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.lines().count(), 2, "{text}");
    assert!(text.lines().next().unwrap().contains("not UTF-8"), "{text}");

    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let input: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n";
    let n = stdio::serve(input, Broken, |m| {
        Some(json!({ "jsonrpc": "2.0", "id": m["id"], "result": {} }))
    })
    .unwrap();
    assert_eq!(n, 0, "a gone reader ends the loop cleanly");
}
