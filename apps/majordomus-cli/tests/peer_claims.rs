//! A peer holds more than one claim.
//!
//! One MCP session is not always one piece of work. A client that fans work out to
//! subagents shares its session with all of them, so they arrive at the board as a single
//! peer; before named claims each announcement replaced the last, and the board ended up
//! describing whichever worker spoke most recently while the rest of the scope silently
//! stopped being claimed by anyone. These are the end-to-end proofs, over the transport a
//! client actually uses: two MCP sessions on one server, announcing under names.

mod common;

use serde_json::{json, Value};

use common::{Fixture, Served};

/// One MCP session over HTTP, holding the session header the server minted for it.
struct Session<'a> {
    served: &'a Served,
    id: String,
    next: u64,
}

impl<'a> Session<'a> {
    /// `initialize`, then the notification that completes it: what any MCP client does
    /// before its first tool call.
    fn open(served: &'a Served, client: &str) -> Self {
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": client, "version": "0.0.0" },
            },
        });
        let (status, headers, _) =
            served.request("POST", "/mcp", Some(&serde_json::to_string(&body).unwrap()));
        assert_eq!(status, 200, "initialize was not accepted");
        let id = headers
            .iter()
            .find(|(k, _)| k == "mcp-session-id")
            .map(|(_, v)| v.clone())
            .expect("the server mints a session id on initialize");
        let mut s = Session {
            served,
            id,
            next: 2,
        };
        s.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        s
    }

    fn send(&mut self, message: Value) -> String {
        let (_, _, body) = self.served.request_with(
            "POST",
            "/mcp",
            Some(&serde_json::to_string(&message).unwrap()),
            &[("Mcp-Session-Id", &self.id)],
        );
        body
    }

    /// Call a tool and return its structured content.
    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let id = self.next;
        self.next += 1;
        let body = self.send(json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": tool, "arguments": arguments },
        }));
        let v: Value = serde_json::from_str(&body).unwrap_or_else(|e| panic!("{e}: {body}"));
        assert_eq!(
            v["result"]["isError"], false,
            "{tool} was refused: {}",
            body
        );
        v["result"]["structuredContent"].clone()
    }
}

/// The whole defect and its absence, over the transport a client uses: one session
/// announcing three times under three names holds three claims, every one of them is
/// ground another peer is warned off, and re-announcing a name updates it in place.
#[test]
fn one_session_holds_every_claim_it_names() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let mut fleet = Session::open(&served, "claude-code");

    fleet.call(
        "majordomus_announce",
        json!({ "intent": "the whole mandate", "scope": ["docs"] }),
    );
    fleet.call(
        "majordomus_announce",
        json!({ "intent": "the guard", "scope": [".claude"], "claim": "worker-a" }),
    );
    let third = fleet.call(
        "majordomus_announce",
        json!({ "intent": "the cockpit", "scope": ["site"], "claim": "worker-b" }),
    );

    let claims = third["claims"].as_array().expect("claims are listed");
    assert_eq!(claims.len(), 3, "three claims, not the newest one: {third}");
    assert_eq!(claims[0]["intent"], "the whole mandate");
    assert!(
        claims[0].get("name").is_none(),
        "the unnamed claim carries no name: {third}"
    );
    assert_eq!(claims[1]["name"], "worker-a");
    assert_eq!(claims[2]["name"], "worker-b");
    assert_eq!(
        third["announcement"]["intent"], "the cockpit",
        "the single-line projection is the most recent claim"
    );

    // a second client sees all three, and collides with two of them
    let mut other = Session::open(&served, "codex");
    let answer = other.call(
        "majordomus_announce",
        json!({ "intent": "unrelated", "scope": [".claude/hooks", "site"] }),
    );
    let met: Vec<&str> = answer["overlaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["intent"].as_str().unwrap())
        .collect();
    assert_eq!(
        met,
        vec!["the guard", "the cockpit"],
        "every claim of a peer is ground, whichever of its claims it is: {answer}"
    );

    // and the board serves the same thing to a reader that is not a peer at all
    let (status, board) = served.get("/api/v1/peers");
    assert_eq!(status, 200);
    let p1 = board["peers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "p1")
        .expect("the first session is on the board");
    assert_eq!(p1["claims"].as_array().unwrap().len(), 3, "{board}");
}

/// Announcing under a name already used replaces that claim rather than adding one, and
/// the answer names the ground the peer has just stopped holding — which is the half that
/// makes a narrowing visible to the worker that did it.
#[test]
fn re_announcing_a_name_updates_it_and_reports_what_it_released() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let mut peer = Session::open(&served, "claude-code");

    peer.call(
        "majordomus_announce",
        json!({ "intent": "the guard", "scope": [".claude", "scripts/ci"], "claim": "worker-a" }),
    );
    let again = peer.call(
        "majordomus_announce",
        json!({ "intent": "the guard, narrowed", "scope": [".claude"], "claim": "worker-a" }),
    );

    assert_eq!(
        again["claims"].as_array().unwrap().len(),
        1,
        "the same name is the same claim: {again}"
    );
    assert_eq!(again["claims"][0]["intent"], "the guard, narrowed");
    assert_eq!(
        again["released"],
        json!(["scripts/ci"]),
        "the ground it stopped claiming is named at the moment it stops: {again}"
    );
}

/// An unnamed announcement is what a single session has always made, and it still behaves
/// exactly as it did: one claim, replaced each time, reported under `announcement`.
#[test]
fn an_unnamed_announcement_is_what_it_always_was() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let mut peer = Session::open(&served, "claude-code");

    peer.call(
        "majordomus_announce",
        json!({ "intent": "first", "scope": ["docs"] }),
    );
    let second = peer.call(
        "majordomus_announce",
        json!({ "intent": "second", "scope": ["site"] }),
    );

    assert_eq!(second["claims"].as_array().unwrap().len(), 1);
    assert_eq!(second["announcement"]["intent"], "second");
    assert_eq!(second["announcement"]["scope"], json!(["site"]));
    assert_eq!(
        second["released"],
        json!(["docs"]),
        "even the unnamed claim says what it stopped holding: {second}"
    );
}
