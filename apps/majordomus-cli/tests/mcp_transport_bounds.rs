//! Every MCP transport has a bound, and a session outlives its longest request (I2147).

mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use common::Fixture;
use majordomus_cli::http::mcp::McpEndpoint;
use majordomus_cli::http::Request;
use majordomus_cli::mcp::{bridge, stdio};
use serde_json::{json, Value};

#[test]
fn a_stdin_line_over_the_bound_is_refused_and_the_session_goes_on() {
    let mut input = vec![b'x'; stdio::MAX_LINE_BYTES + 10];
    input.push(b'\n');
    input.extend_from_slice(br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#);
    input.push(b'\n');
    let mut out = Vec::new();
    let answered = stdio::serve(&input[..], &mut out, |m| {
        Some(json!({ "jsonrpc": "2.0", "id": m["id"], "result": {} }))
    })
    .unwrap();
    let lines: Vec<&str> = std::str::from_utf8(&out).unwrap().lines().collect();
    assert_eq!(answered, 2, "{lines:?}");
    assert!(
        lines[0].contains("-32700") && lines[0].contains("bound"),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].contains("\"result\""),
        "the next message is answered"
    );
}

#[test]
fn a_reply_over_the_bound_is_refused() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let sender = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut head = [0u8; 1024];
        let _ = stream.read(&mut head);
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n");
        let chunk = vec![b'a'; 1024 * 1024];
        for _ in 0..=(bridge::MAX_REPLY_BYTES / chunk.len()) {
            if stream.write_all(&chunk).is_err() {
                break;
            }
        }
    });
    let reply = bridge::request(&url, "GET", "/", &[], None, Duration::from_secs(30));
    let err = reply.expect_err("a reply over the bound is not read");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData, "{err}");
    let _ = sender.join();
}

fn endpoint(f: &Fixture) -> Arc<McpEndpoint> {
    let app = common::load_app(f);
    Arc::new(McpEndpoint::new(
        app.context.clone(),
        "test",
        "http://127.0.0.1:0".into(),
    ))
}

fn post(
    endpoint: &McpEndpoint,
    headers: Vec<(String, String)>,
    message: Value,
) -> (u16, Value, Option<String>) {
    let req = Request::parse_target("POST", "/mcp", message.to_string().into_bytes())
        .with_headers(headers);
    let response = endpoint.handle(&req);
    let id = response
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("mcp-session-id"))
        .map(|(_, v)| v.clone());
    (
        response.status,
        serde_json::from_str(&response.body.text()).unwrap_or(Value::Null),
        id,
    )
}

fn initialize() -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                        "clientInfo": { "name": "unit", "version": "0" } } })
}

#[test]
fn a_session_id_is_random_and_an_unknown_protocol_version_is_refused() {
    let f = Fixture::new();
    let e = endpoint(&f);
    let (_, _, a) = post(&e, vec![], initialize());
    let (_, _, b) = post(&e, vec![], initialize());
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a, b);
    for id in [&a, &b] {
        assert_eq!(id.len(), 32, "{id}");
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()), "{id}");
        assert!(
            !id.contains(&std::process::id().to_string()),
            "no pid in {id}"
        );
    }
    let (status, body, _) = post(
        &e,
        vec![
            ("Mcp-Session-Id".into(), a.clone()),
            ("MCP-Protocol-Version".into(), "1999-01-01".into()),
        ],
        json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }),
    );
    assert_eq!(status, 400, "{body}");
    let (status, _, _) = post(
        &e,
        vec![
            ("Mcp-Session-Id".into(), a),
            ("MCP-Protocol-Version".into(), "2025-06-18".into()),
        ],
        json!({ "jsonrpc": "2.0", "id": 3, "method": "ping" }),
    );
    assert_eq!(status, 200, "a version this server speaks is served");
}

#[test]
fn a_session_is_not_reaped_behind_a_request_that_ran_long() {
    let f = Fixture::new();
    let e = endpoint(&f);
    let (_, _, id) = post(&e, vec![], initialize());
    let id = id.unwrap();
    // a call that runs 300 ms
    let (status, _, _) = post(
        &e,
        vec![("Mcp-Session-Id".into(), id)],
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                "params": { "name": "majordomus_demonstrate_execution",
                            "arguments": { "steps": 1, "delay_ms": 300 } } }),
    );
    assert_eq!(status, 200);
    // a reaper that forgets sessions silent for 200 ms, run the moment the answer is out
    let gone = e.reap_idle(Duration::from_millis(200), Duration::from_secs(900));
    assert!(
        gone.is_empty(),
        "the session was reaped behind its own request"
    );
    assert_eq!(e.active(), 1);
}
