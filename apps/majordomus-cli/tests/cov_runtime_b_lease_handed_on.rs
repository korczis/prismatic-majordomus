//! A server that lost the checkout's lease hands each session on once the busy grace has
//! passed, and its log says which session it handed on and why.
//!
//! One test in this binary: [`majordomus_cli::lease::lost`] records a fact about the process
//! that is never unmade.

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::Fixture;
use majordomus_cli::http::mcp::McpEndpoint;
use majordomus_cli::http::{server, Router};
use majordomus_cli::lease;
use serde_json::json;

#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn post(url: &str, session: Option<&str>, body: &serde_json::Value) -> (u16, Option<String>) {
    let mut headers = vec![("content-type", "application/json")];
    if let Some(s) = session {
        headers.push(("mcp-session-id", s));
    }
    let reply = majordomus_cli::mcp::bridge::request(
        url,
        "POST",
        "/mcp",
        &headers,
        Some(&body.to_string()),
        Duration::from_secs(30),
    )
    .expect("the server answers /mcp");
    let session = reply.header("mcp-session-id").map(str::to_string);
    (reply.status, session)
}

#[test]
fn a_session_handed_on_after_a_lost_lease_is_named_in_the_log() {
    let captured = Captured::default();
    let writer = captured.clone();
    tracing::subscriber::set_global_default(
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || writer.clone())
            .finish(),
    )
    .expect("the only subscriber of this binary");
    lease::declare_timings(&majordomus_cli::policy::ServerPolicy {
        busy_grace_seconds: Some(1),
        ..Default::default()
    });
    let f = Fixture::new();
    let app = common::load_app(&f);
    let bound = server::bind("127.0.0.1", 0).expect("a free loopback port");
    let url = bound.url();
    let endpoint = Arc::new(McpEndpoint::new(app.context.clone(), "test", url.clone()));
    let router = Router::new(app.context.clone(), "test").with_mcp(Arc::clone(&endpoint));
    let running = bound.start(router);

    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": { "name": "unit", "version": "0" } } });
    let (status, session) = post(&url, None, &init);
    assert_eq!(status, 200);
    let session = session.expect("a session id");
    assert_eq!(endpoint.active(), 1);

    lease::lost();
    // within the grace the session is served; past it, handed on
    let ping = json!({ "jsonrpc": "2.0", "id": 9, "method": "ping" });
    let waited = Instant::now();
    loop {
        let (status, _) = post(&url, Some(&session), &ping);
        if status == 404 {
            break;
        }
        assert_eq!(status, 200, "served while the grace lasts");
        assert!(
            waited.elapsed() < Duration::from_secs(60),
            "the session was never handed on"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        endpoint.active(),
        0,
        "this server holds no session any more"
    );
    let log = String::from_utf8_lossy(&captured.0.lock().unwrap()).into_owned();
    let line = log
        .lines()
        .find(|l| l.contains("session handed on: this server lost the lease"))
        .unwrap_or_else(|| panic!("no hand-on in the log:\n{log}"));
    assert!(line.contains(&session), "the session is named: {line}");
    assert!(line.contains("peer="), "and its peer: {line}");
    running.stop();
}
