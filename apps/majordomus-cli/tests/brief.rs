//! The session brief (ADR 0136), end to end: the command line, the HTTP route, the MCP tool
//! and the Cockpit are one answer, and the command line reads the board of the server that
//! holds the lease rather than the empty board of its own process.

mod common;

use serde_json::{json, Value};

use common::{run_in, Fixture, Served};

/// One MCP session over HTTP, holding the session header the server minted for it.
struct Session<'a> {
    served: &'a Served,
    id: String,
    next: u64,
}

impl<'a> Session<'a> {
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

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let id = self.next;
        self.next += 1;
        let body = self.send(json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": tool, "arguments": arguments },
        }));
        let v: Value = serde_json::from_str(&body).unwrap_or_else(|e| panic!("{e}: {body}"));
        assert_eq!(v["result"]["isError"], false, "{tool} was refused: {body}");
        v["result"]["structuredContent"].clone()
    }
}

fn brief_json(f: &Fixture, extra: &[&str]) -> Value {
    let mut args = vec!["brief", "--format", "json", "--this-checkout"];
    args.extend_from_slice(extra);
    let (code, out, err) = run_in(&f.root(), &args, "");
    assert_eq!(code, 0, "brief failed: {err}");
    serde_json::from_str(&out).unwrap_or_else(|e| panic!("brief is not JSON ({e}): {out}"))
}

#[test]
fn without_a_server_the_command_line_composes_the_brief_and_says_so() {
    let f = Fixture::new();
    let b = brief_json(&f, &[]);
    assert_eq!(b["answered_by"]["process"], "this_process", "{b}");
    assert!(
        b["answered_by"]["note"]
            .as_str()
            .is_some_and(|n| n.contains("no server answers")),
        "{b}"
    );
    for section in ["session", "peers", "mesh", "integration", "derive"] {
        assert!(b[section]["state"].is_string(), "{section} is missing: {b}");
        assert!(
            b[section]["summary"].is_string(),
            "{section} says nothing: {b}"
        );
    }
    assert_eq!(
        b["peers"]["attached"], 0,
        "nobody is attached without a server"
    );
    let line = b["line"].as_str().expect("a line");
    assert!(line.contains("| peers 0/1b |"), "{line}");
    assert!(b["keep_awake"]
        .as_str()
        .is_some_and(|k| k.contains("majordomus brief")));
    // the other shapes of the same answer
    let (code, out, _) = run_in(&f.root(), &["brief", "--line", "--this-checkout"], "");
    assert_eq!(code, 0);
    assert_eq!(out.lines().count(), 1, "{out}");
    let (code, out, _) = run_in(&f.root(), &["brief", "--keep-awake", "--this-checkout"], "");
    assert_eq!(code, 0);
    assert!(out.starts_with("Keep yourself awake"), "{out}");
    assert!(out.contains("\nNow: "), "{out}");
    let (code, out, _) = run_in(&f.root(), &["brief", "--this-checkout"], "");
    assert_eq!(code, 0);
    assert!(out.starts_with("session      "), "{out}");
    assert!(out.contains("\nanswered by  this_process"), "{out}");
}

#[test]
fn beside_a_server_every_surface_carries_the_leaseholders_board() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let mut session = Session::open(&served, "brief-test");
    session.call(
        "majordomus_announce",
        json!({ "intent": "building the brief", "scope": ["docs/BRIEF.md"] }),
    );

    // MCP: the caller is named, and its own announcement is on the board
    let over_mcp = session.call("majordomus_brief", json!({ "checkouts": "this" }));
    assert_eq!(
        over_mcp["answered_by"]["process"], "leaseholder",
        "{over_mcp}"
    );
    assert_eq!(over_mcp["session"]["caller"], "p1", "{over_mcp}");
    assert_eq!(over_mcp["peers"]["attached"], 1, "{over_mcp}");

    // HTTP: the same board
    let (status, over_http) = served.get("/api/v1/brief?checkouts=this");
    assert_eq!(status, 200, "{over_http}");
    assert_eq!(over_http["peers"]["attached"], 1, "{over_http}");

    // the command line: asked of the server, not composed from this process's empty board
    let b = brief_json(&f, &["--session", "no-such-provider-session"]);
    assert_eq!(b["answered_by"]["process"], "leaseholder", "{b}");
    assert_eq!(b["peers"]["attached"], 1, "{b}");
    assert!(
        b["peers"]["named"]
            .as_array()
            .is_some_and(|n| n.iter().any(|l| l
                .as_str()
                .is_some_and(|l| l.contains("brief-test") && l.contains("building the brief")))),
        "{b}"
    );
    assert!(
        b["session"]["summary"].as_str().is_some_and(
            |s| s.contains("no open episode for provider session no-such-provider-session")
        ),
        "{b}"
    );

    // the Cockpit overview lays it out
    let (status, _, page) =
        served.request_with("GET", "/cockpit", None, &[("Accept", "text/html")]);
    assert_eq!(status, 200);
    assert!(
        page.contains("Session brief"),
        "the overview shows no brief"
    );
    assert!(
        page.contains("building the brief"),
        "the overview names no peer"
    );
}
