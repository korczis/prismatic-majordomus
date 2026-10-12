//! What a bridged `majordomus mcp` does once the shared server it was bridged to stops
//! answering, branch by branch: it re-attaches to the server the election names, takes the
//! lease over when that server is gone, or serves its client alone when no election can be
//! held; and in each, a message that may have run is reported as an outcome nobody knows
//! instead of being sent again, while a message that cannot run twice is sent again.
//!
//! The server is a few lines of HTTP in this test, named by a lease planted in the fixture,
//! so that it can lose an answer, refuse, or stop being this repository's server on purpose.
//! Every `majordomus mcp` here is ended the way a client ends it, by closing its input, and
//! must exit cleanly: a session that ends is part of what is proven.

mod common;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::Fixture;
use serde_json::{json, Value};

const WAIT: Duration = Duration::from_secs(60);

/// A tool that writes the repository: never idempotent by its declared hints.
const WRITER: &str = "majordomus_plan_transition";

/// A tool that reads: idempotent by its declared hints.
const READER: &str = "majordomus_repository";

/// What the fake server does with the `n`th request of one kind (counted from 1).
#[derive(Clone, Copy, Debug)]
enum Then {
    /// Answer it.
    Answer,
    /// Read the whole request, then close the connection without a word.
    Drop,
    /// As [`Then::Drop`], and from now on answer the lease probe as another repository's
    /// server: the election finds this server gone.
    DropAndLeave,
    /// Answer with this HTTP status.
    Status(u16),
}

/// The script: the method, the tool for a `tools/call` (empty otherwise), and how many
/// requests of that method and tool this one is.
type Script = Arc<dyn Fn(&str, &str, usize) -> Then + Send + Sync>;

struct FakeServer {
    url: String,
    seen: Arc<Mutex<HashMap<String, usize>>>,
}

impl FakeServer {
    fn start(root: &Path, script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}", listener.local_addr().unwrap());
        let identity = majordomus_cli::repository::identity(root);
        let seen = Arc::new(Mutex::new(HashMap::new()));
        let counts = Arc::clone(&seen);
        let gone = Arc::new(AtomicBool::new(false));
        let sessions = Arc::new(AtomicUsize::new(0));
        std::thread::spawn(move || {
            for stream in listener.incoming().map_while(Result::ok) {
                let identity = identity.clone();
                let counts = Arc::clone(&counts);
                let script = Arc::clone(&script);
                let gone = Arc::clone(&gone);
                let sessions = Arc::clone(&sessions);
                std::thread::spawn(move || {
                    serve_one(stream, &identity, &counts, &script, &gone, &sessions);
                });
            }
        });
        FakeServer { url, seen }
    }

    /// How many requests of `method` (and, for a call, `tool`) reached this server.
    fn received(&self, method: &str, tool: &str) -> usize {
        self.seen
            .lock()
            .unwrap()
            .get(&format!("{method} {tool}"))
            .copied()
            .unwrap_or(0)
    }

    /// Name this server in the fixture's lease, as the process that holds it.
    fn plant_lease(&self, f: &Fixture) {
        let lease = json!({
            "schema": "majordomus-mcp-lease/v1",
            "pid": std::process::id(),
            "token": "fake-server",
            "root": f.root(),
            "url": self.url,
            "started_at": "2026-10-09T00:00:00Z",
        });
        f.write(".ai/local/state/mcp/server.json", &lease.to_string());
    }
}

fn serve_one(
    mut stream: TcpStream,
    identity: &str,
    counts: &Mutex<HashMap<String, usize>>,
    script: &Script,
    gone: &AtomicBool,
    sessions: &AtomicUsize,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let Some((method, path, body)) = read_request(&mut stream) else {
        return;
    };
    if method == "GET" && path == "/" {
        let repository = if gone.load(Ordering::SeqCst) {
            "another-repository".to_string()
        } else {
            identity.to_string()
        };
        let index =
            json!({ "name": "majordomus", "repository_id": repository, "leaseholder": true });
        return respond(&mut stream, 200, &index.to_string(), None);
    }
    if method == "DELETE" {
        return respond(&mut stream, 204, "", None);
    }
    let message: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let id = message["id"].clone();
    let rpc = message["method"].as_str().unwrap_or_default().to_string();
    let tool = if rpc == "tools/call" {
        message["params"]["name"].as_str().unwrap_or_default()
    } else {
        ""
    };
    let n = {
        let mut counts = counts.lock().unwrap();
        let n = counts.entry(format!("{rpc} {tool}")).or_insert(0);
        *n += 1;
        *n
    };
    match script(&rpc, tool, n) {
        Then::Drop => return,
        Then::DropAndLeave => {
            gone.store(true, Ordering::SeqCst);
            return;
        }
        Then::Status(s) => return respond(&mut stream, s, r#"{"error":"refused"}"#, None),
        Then::Answer => {}
    }
    match rpc.as_str() {
        "initialize" => {
            let n = sessions.fetch_add(1, Ordering::SeqCst) + 1;
            let result = json!({ "protocolVersion": "2025-06-18", "capabilities": { "tools": {} }, "serverInfo": { "name": "fake", "version": "0" } });
            let answer = json!({ "jsonrpc": "2.0", "id": id, "result": result });
            respond(
                &mut stream,
                200,
                &answer.to_string(),
                Some(&format!("fake-{n}")),
            );
        }
        "tools/call" => {
            let result = json!({ "content": [{ "type": "text", "text": "done" }], "structuredContent": { "answered_by": "fake", "attempt": n }, "isError": false });
            let answer = json!({ "jsonrpc": "2.0", "id": id, "result": result });
            respond(&mut stream, 200, &answer.to_string(), None);
        }
        m if m.starts_with("notifications/") => respond(&mut stream, 202, "", None),
        _ => {
            let answer = json!({ "jsonrpc": "2.0", "id": id, "result": {} });
            respond(&mut stream, 200, &answer.to_string(), None);
        }
    }
}

/// One request: method, path and body, read whole (`Content-Length`).
fn read_request(stream: &mut TcpStream) -> Option<(String, String, String)> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((k, v)) = header.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                length = v.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).ok()?;
    Some((method, path, String::from_utf8_lossy(&body).into_owned()))
}

fn respond(stream: &mut TcpStream, status: u16, body: &str, session: Option<&str>) {
    let mut head = format!(
        "HTTP/1.1 {status} X\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        body.len()
    );
    if let Some(s) = session {
        head.push_str(&format!("Mcp-Session-Id: {s}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
}

/// A `majordomus mcp` child: stdout frames and stderr lines, each readable with a timeout.
struct Mcp {
    child: Child,
    stdin: Option<ChildStdin>,
    out: Receiver<String>,
    err: Receiver<String>,
    seen_err: Vec<String>,
}

impl Mcp {
    fn spawn(f: &Fixture) -> Self {
        let mut child = common::majordomus()
            .args(["mcp", "--http-port", "0"])
            .current_dir(f.root())
            .env("MAJORDOMUS_LOG", "info")
            .env("MAJORDOMUS_SHARE", common::dist_share())
            .env("XDG_STATE_HOME", f.parent().join("state"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn majordomus mcp");
        let stdin = child.stdin.take();
        let (out_tx, out) = mpsc::channel();
        let stdout = child.stdout.take().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if out_tx.send(line).is_err() {
                    break;
                }
            }
        });
        let (err_tx, err) = mpsc::channel();
        let stderr = child.stderr.take().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if err_tx.send(line).is_err() {
                    break;
                }
            }
        });
        Mcp {
            child,
            stdin,
            out,
            err,
            seen_err: Vec::new(),
        }
    }

    fn wait_log(&mut self, needle: &str) -> String {
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.err.recv_timeout(left) {
                Ok(line) => {
                    self.seen_err.push(line.clone());
                    if line.contains(needle) {
                        return line;
                    }
                }
                Err(_) => panic!(
                    "no stderr line containing {needle:?} within {WAIT:?}; stderr so far:\n{}",
                    self.seen_err.join("\n")
                ),
            }
        }
    }

    fn drain_log(&mut self) -> String {
        while let Ok(line) = self.err.try_recv() {
            self.seen_err.push(line);
        }
        self.seen_err.join("\n")
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin still open");
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let line = self.out.recv_timeout(WAIT).unwrap_or_else(|_| {
            panic!(
                "no stdout frame for {method} within {WAIT:?}; stderr:\n{}",
                self.drain_log()
            )
        });
        let v: Value = serde_json::from_str(&line).expect("a JSON frame");
        assert_eq!(v["id"], id, "the frame answers the request: {line}");
        v
    }

    fn initialize(&mut self) {
        self.request(
            1,
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "failover-test", "version": "0" } }),
        );
        self.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    }

    fn call(&mut self, id: u64, tool: &str, arguments: Value) -> Value {
        self.request(
            id,
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        )
    }

    /// End the session as a client does, by closing the input, and wait for a clean exit.
    fn finish(mut self) -> String {
        drop(self.stdin.take());
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(status) = self.child.try_wait().expect("the child can be waited on") {
                let log = self.drain_log();
                assert!(status.success(), "exit {status}; stderr:\n{log}");
                return log;
            }
            assert!(
                Instant::now() < deadline,
                "the session did not end within {WAIT:?} of its input closing; stderr:\n{}",
                self.drain_log()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// A bridge attached to a fake server that runs `script`, its client initialized.
fn bridged(script: Script) -> (Fixture, FakeServer, Mcp) {
    let f = Fixture::new();
    let server = FakeServer::start(&f.root(), script);
    server.plant_lease(&f);
    let mut bridge = Mcp::spawn(&f);
    bridge.wait_log("bridging this stdio session");
    bridge.initialize();
    (f, server, bridge)
}

fn assert_outcome_unknown(answer: &Value, id: u64) {
    assert_eq!(answer["error"]["code"], -32603, "{answer}");
    assert_eq!(answer["error"]["data"]["outcome"], "unknown", "{answer}");
    assert_eq!(answer["error"]["data"]["request_id"], id, "{answer}");
    let text = answer["error"]["message"].as_str().unwrap_or_default();
    assert!(text.starts_with("outcome unknown"), "{answer}");
}

/// Make the election impossible: the directory the lease lives in is replaced by a file.
fn break_the_election(f: &Fixture) {
    let dir = f.path(".ai/local/state/mcp");
    std::fs::remove_dir_all(&dir).unwrap();
    std::fs::write(&dir, "not a directory").unwrap();
}

/// A write whose answer was lost, on a server the election names again: the bridge
/// re-attaches, opens its session there anew, and reports the write as an outcome nobody
/// knows rather than sending it a second time.
#[test]
fn a_lost_write_reattaches_to_the_named_server_and_is_not_sent_again() {
    let (_f, server, mut bridge) = bridged(Arc::new(|_, tool, n| {
        if tool == WRITER && n == 1 {
            Then::Drop
        } else {
            Then::Answer
        }
    }));
    let answer = bridge.call(7, WRITER, json!({ "id": "I0001", "status": "done" }));
    assert_outcome_unknown(&answer, 7);
    assert_eq!(server.received("tools/call", WRITER), 1, "sent once");
    assert_eq!(
        server.received("initialize", ""),
        2,
        "the session was opened again on the server the election named"
    );
    // the session goes on, on the server it re-attached to
    let next = bridge.call(8, READER, json!({}));
    assert_eq!(next["result"]["structuredContent"]["answered_by"], "fake");
    let log = bridge.finish();
    assert!(log.contains("electing again"), "{log}");
    assert!(log.contains("re-attached to the shared server"), "{log}");
}

/// A write whose answer was lost, on a server that then will not open a session: the write is
/// still reported as an outcome nobody knows, and it is not sent again anywhere.
#[test]
fn a_lost_write_whose_session_cannot_be_reopened_is_still_reported_unknown() {
    let (_f, server, mut bridge) = bridged(Arc::new(|method, tool, n| match (method, tool, n) {
        ("tools/call", WRITER, 1) => Then::Drop,
        ("initialize", _, n) if n >= 2 => Then::Status(500),
        _ => Then::Answer,
    }));
    let answer = bridge.call(7, WRITER, json!({ "id": "I0001", "status": "done" }));
    assert_outcome_unknown(&answer, 7);
    assert_eq!(server.received("tools/call", WRITER), 1, "sent once");
    assert_eq!(
        server.received("initialize", ""),
        2,
        "the reopening was tried"
    );
    bridge.finish();
}

/// A read whose answer was lost is sent again on the server the election names; when that
/// server refuses it with a status that calls for no election, the client gets that
/// refusal, named, rather than "unavailable".
#[test]
fn a_resent_read_refused_by_the_named_server_is_answered_as_that_refusal() {
    let (_f, server, mut bridge) = bridged(Arc::new(|_, tool, n| match (tool, n) {
        (READER, 1) => Then::Drop,
        (READER, _) => Then::Status(400),
        _ => Then::Answer,
    }));
    let answer = bridge.call(5, READER, json!({}));
    assert_eq!(answer["error"]["code"], -32600, "{answer}");
    assert_eq!(answer["error"]["data"]["status"], 400, "{answer}");
    let text = answer["error"]["message"].as_str().unwrap_or_default();
    assert!(text.starts_with("request 5 was refused"), "{answer}");
    assert!(!text.contains("unavailable"), "{answer}");
    assert_eq!(server.received("tools/call", READER), 2, "sent again once");
    bridge.finish();
}

/// A write whose answer was lost, from a server that is then no longer this repository's:
/// the bridge takes the lease over and serves itself, the write is reported as an outcome
/// nobody knows, and the session goes on locally.
#[test]
fn a_lost_write_on_a_server_that_left_is_taken_over_and_not_sent_again() {
    let (f, server, mut bridge) = bridged(Arc::new(|_, tool, n| {
        if tool == WRITER && n == 1 {
            Then::DropAndLeave
        } else {
            Then::Answer
        }
    }));
    let answer = bridge.call(7, WRITER, json!({ "id": "I0001", "status": "done" }));
    assert_outcome_unknown(&answer, 7);
    assert_eq!(server.received("tools/call", WRITER), 1, "sent once");
    bridge.wait_log("took over as the shared server");
    let lease: Value = serde_json::from_str(
        &std::fs::read_to_string(f.path(".ai/local/state/mcp/server.json")).unwrap(),
    )
    .unwrap();
    assert_ne!(
        lease["token"], "fake-server",
        "the lease is the bridge's own now"
    );
    // the session goes on, answered by this process itself
    let next = bridge.call(8, READER, json!({}));
    assert!(next.get("result").is_some(), "{next}");
    assert_ne!(next["result"]["structuredContent"]["answered_by"], "fake");
    assert_eq!(server.received("tools/call", READER), 0);
    bridge.finish();
}

/// A write whose answer was lost when no election can be held: the client is served alone,
/// and the write is reported as an outcome nobody knows rather than run here a second time.
#[test]
fn a_lost_write_with_no_election_possible_is_served_alone_and_not_sent_again() {
    let (f, server, mut bridge) = bridged(Arc::new(|_, tool, n| {
        if tool == WRITER && n == 1 {
            Then::Drop
        } else {
            Then::Answer
        }
    }));
    break_the_election(&f);
    let answer = bridge.call(7, WRITER, json!({ "id": "I0001", "status": "done" }));
    assert_outcome_unknown(&answer, 7);
    assert_eq!(server.received("tools/call", WRITER), 1, "sent once");
    let reason = bridge.wait_log("serving this client alone");
    assert!(reason.contains("electing again failed"), "{reason}");
    // the session goes on, answered alone
    let next = bridge.call(8, READER, json!({}));
    assert!(next.get("result").is_some(), "{next}");
    assert_ne!(next["result"]["structuredContent"]["answered_by"], "fake");
    assert_eq!(server.received("tools/call", READER), 0);
    bridge.finish();
}

/// A read whose answer was lost when no election can be held is run again by the process
/// that now serves the client alone, and the client gets that answer.
#[test]
fn a_lost_read_with_no_election_possible_is_answered_alone() {
    let (f, server, mut bridge) = bridged(Arc::new(|_, tool, n| {
        if tool == READER && n == 1 {
            Then::Drop
        } else {
            Then::Answer
        }
    }));
    break_the_election(&f);
    let answer = bridge.call(5, READER, json!({}));
    assert!(answer.get("result").is_some(), "{answer}");
    assert_ne!(answer["result"]["structuredContent"]["answered_by"], "fake");
    assert_eq!(server.received("tools/call", READER), 1);
    bridge.wait_log("serving this client alone");
    bridge.finish();
}

/// A write whose answer was lost when nothing can serve the client — no election, and a
/// layer that no longer loads — is still reported as an outcome nobody knows, never as a
/// failure that suggests it did not run.
#[test]
fn a_lost_write_that_nothing_can_serve_afterwards_is_still_reported_unknown() {
    let (f, server, mut bridge) = bridged(Arc::new(|_, tool, n| {
        if tool == WRITER && n == 1 {
            Then::Drop
        } else {
            Then::Answer
        }
    }));
    break_the_election(&f);
    f.write(".ai/manifest.yaml", "schema: [this is not a manifest\n");
    let answer = bridge.call(7, WRITER, json!({ "id": "I0001", "status": "done" }));
    assert_outcome_unknown(&answer, 7);
    assert_eq!(server.received("tools/call", WRITER), 1, "sent once");
    bridge.wait_log("cannot serve this client alone either");
    bridge.finish();
}
