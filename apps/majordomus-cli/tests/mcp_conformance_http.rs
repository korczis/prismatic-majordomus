//! MCP over HTTP held to its boundaries, on a real `majordomus serve`: who may speak to
//! `/mcp`, and whether sessions that share one server stay apart.
//!
//! `mcp_shared.rs` proves the session mechanics (an id is issued, required and forgotten)
//! and that a storm of stdio clients converges on one server. What it does not ask:
//!
//! - whether a page in a browser, on another origin, can open or use a session — the
//!   attack a loopback server is exposed to, and the reason the router refuses a foreign
//!   `Origin` on anything that is not a read;
//! - whether sessions used at the same time, request by request interleaved, each keep
//!   their own identity and all see one listing.

mod common;

use std::sync::Arc;

use common::{Fixture, Served};
use serde_json::{json, Value};

/// A server whose per-user state (the mesh's node identity) lives beside the fixture.
fn serve(f: &Fixture) -> Served {
    let state = f.parent().join("state");
    Served::start_with_env(
        &f.root(),
        &[],
        &[("XDG_STATE_HOME", state.to_str().unwrap())],
    )
}

fn initialize(name: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": { "name": name, "version": "0" } } })
    .to_string()
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// A browser attaches `Origin` to every request a page makes, and a page on any site can
/// address `http://127.0.0.1:<port>/mcp`. So the origin is the whole boundary between "a
/// client on this machine" and "whatever tab the developer has open": a foreign one is
/// refused before a session exists, gets no session id to reuse, and cannot ride a session
/// that a legitimate client opened. The server's own origin, and a client that sends none
/// (every MCP client that is not a browser), are served.
#[test]
fn a_foreign_origin_can_neither_open_nor_use_an_mcp_session() {
    let f = Fixture::new();
    let served = serve(&f);
    let own = format!("http://{}", served.address);
    let foreign = [
        "https://evil.example",
        "http://evil.example",
        "null",
        // the same host on another port is another origin
        "http://127.0.0.1:1",
        // ... and so is a name that merely starts like the server's
        &format!("{own}.evil.example"),
    ];

    for origin in foreign {
        let (status, headers, body) = served.request_with(
            "POST",
            "/mcp",
            Some(&initialize("foreign")),
            &[("Origin", origin)],
        );
        assert!(
            (400..500).contains(&status),
            "initialize from Origin {origin} was answered {status}: {body}"
        );
        assert!(
            header(&headers, "mcp-session-id").is_none(),
            "a refused origin ({origin}) was handed a session id"
        );
        assert!(
            serde_json::from_str::<Value>(&body)
                .map(|v| v.get("result").is_none())
                .unwrap_or(true),
            "a refused origin ({origin}) was answered with a result: {body}"
        );
        assert!(
            header(&headers, "access-control-allow-origin").is_none(),
            "the refusal of {origin} carries a CORS grant"
        );
    }

    // a client that is not a browser, and a page the server itself served, are let in
    let (status, headers, body) = served.request("POST", "/mcp", Some(&initialize("plain")));
    assert_eq!(status, 200, "a client with no Origin is served: {body}");
    let session = header(&headers, "mcp-session-id")
        .expect("a session id")
        .to_string();
    let (status, _, body) = served.request_with(
        "POST",
        "/mcp",
        Some(&initialize("own-page")),
        &[("Origin", &own)],
    );
    assert_eq!(status, 200, "the server's own origin is served: {body}");

    // a foreign page that has learned a live session id still cannot use it
    let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }).to_string();
    let announce = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {
        "name": "majordomus_announce",
        "arguments": { "intent": "a page on another origin was here", "paths": "README.md" } } })
    .to_string();
    for body_text in [&list, &announce] {
        let (status, _, body) = served.request_with(
            "POST",
            "/mcp",
            Some(body_text),
            &[
                ("Mcp-Session-Id", &session),
                ("Origin", "https://evil.example"),
            ],
        );
        assert!(
            (400..500).contains(&status),
            "a foreign origin used a live session ({status}): {body}"
        );
    }
    // ... and left nothing behind: the session's owner sees a board no one announced on
    let peers = json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {
        "name": "majordomus_peers", "arguments": {} } })
    .to_string();
    let (status, _, body) = served.request_with(
        "POST",
        "/mcp",
        Some(&peers),
        &[("Mcp-Session-Id", &session)],
    );
    assert_eq!(status, 200, "{body}");
    assert!(
        !body.contains("a page on another origin was here"),
        "the refused announcement reached the board: {body}"
    );
}

/// Six sessions on one server, each sending forty requests while the others send theirs.
/// Every answer carries the id it was asked with, every session is told it is the same
/// peer from its first request to its last, no two sessions are the same peer, and all of
/// them list the same tools: sessions share a server and nothing else.
#[test]
fn interleaved_http_sessions_keep_their_own_identity_and_one_listing() {
    let f = Fixture::new();
    let served = Arc::new(serve(&f));
    let workers: Vec<_> = (0..6)
        .map(|n| {
            let served = Arc::clone(&served);
            std::thread::spawn(move || {
                let (status, headers, body) =
                    served.request("POST", "/mcp", Some(&initialize(&format!("client-{n}"))));
                assert_eq!(status, 200, "{body}");
                let session = header(&headers, "mcp-session-id")
                    .expect("a session id")
                    .to_string();
                let mut callers = Vec::new();
                let mut listings = Vec::new();
                for i in 0..40u64 {
                    let id = 1000 * (n as u64 + 1) + i;
                    let request = if i % 2 == 0 {
                        json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {
                            "name": "majordomus_peers", "arguments": {} } })
                    } else {
                        json!({ "jsonrpc": "2.0", "id": id, "method": "tools/list" })
                    };
                    let (status, _, body) = served.request_with(
                        "POST",
                        "/mcp",
                        Some(&request.to_string()),
                        &[("Mcp-Session-Id", &session)],
                    );
                    assert_eq!(status, 200, "client-{n} request {i}: {body}");
                    let answer: Value = serde_json::from_str(&body).expect("a JSON answer");
                    assert_eq!(
                        answer["id"], id,
                        "client-{n} was answered with another's id"
                    );
                    if i % 2 == 0 {
                        let peers = &answer["result"]["structuredContent"];
                        let caller = peers["caller"].as_str().expect("a caller").to_string();
                        let me = peers["peers"]
                            .as_array()
                            .expect("peers")
                            .iter()
                            .find(|p| p["id"] == caller.as_str())
                            .expect("the caller is on the board");
                        assert_eq!(
                            me["client"]["name"],
                            format!("client-{n}"),
                            "client-{n} is told it is a peer that another client identified as"
                        );
                        callers.push(caller);
                    } else {
                        listings.push(answer["result"].to_string());
                    }
                }
                (session, callers, listings)
            })
        })
        .collect();
    let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();

    let mut identities = std::collections::BTreeSet::new();
    let mut sessions = std::collections::BTreeSet::new();
    for (n, (session, callers, listings)) in results.iter().enumerate() {
        assert!(
            callers.iter().all(|c| c == &callers[0]),
            "client-{n} changed peer mid-session: {callers:?}"
        );
        assert!(
            identities.insert(callers[0].clone()),
            "client-{n} shares the peer id {} with another session",
            callers[0]
        );
        assert!(
            sessions.insert(session.clone()),
            "two clients were handed one session id"
        );
        assert!(
            listings.iter().all(|l| l == &results[0].2[0]),
            "client-{n} listed tools differently from client-0, or from itself"
        );
    }
}
