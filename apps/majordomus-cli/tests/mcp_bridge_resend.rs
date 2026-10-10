//! A bridged `majordomus mcp` sends a message a second time only when the server provably
//! did not run it: the connection was never made, the session was lost (404), the lease was
//! lost (409), or the message is idempotent (every method but `tools/call`, and a tool whose
//! declared execution hints say so). A call that was sent and never answered — a timeout, a
//! dropped connection, a 5xx — may have run, and the client is told that its outcome is
//! unknown instead of having it run twice. A 4xx other than 404 or 409 is answered as that
//! error, with no election: a 413 says "payload too large", not "shared server unavailable".
//!
//! The server a bridge attaches to here is a few lines of HTTP in this test, named by a lease
//! planted in the fixture, so that it can count what it receives and lose an answer on
//! purpose: what is counted is what reached the server, which is what would have run.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{Fixture, BIN};
use serde_json::{json, Value};

const WAIT: Duration = Duration::from_secs(30);

/// A tool that writes the repository: never idempotent by its declared hints.
const WRITER: &str = "majordomus_plan_transition";

/// A tool that reads: idempotent by its declared hints.
const READER: &str = "majordomus_repository";

/// What the fake server does with the `n`th `tools/call` (counted from 1) of one tool.
#[derive(Clone, Copy, Debug)]
enum Then {
    /// Answer with a result.
    Answer,
    /// Read the whole request, then close the connection without a word.
    Drop,
    /// Read the whole request, wait this long, then close without a word.
    Hang(Duration),
    /// Answer with this HTTP status.
    Status(u16),
}

type Script = Arc<dyn Fn(&str, usize) -> Then + Send + Sync>;

/// A stand-in for the shared server: answers the lease probe for one root, opens sessions,
/// and does with each `tools/call` what its script says, counting them per tool.
struct FakeServer {
    url: String,
    calls: Arc<Mutex<Vec<String>>>,
}

impl FakeServer {
    fn start(root: &Path, script: Script) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}", listener.local_addr().unwrap());
        let identity = majordomus_cli::repository::identity(root);
        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = Arc::clone(&calls);
        let sessions = Arc::new(AtomicUsize::new(0));
        std::thread::spawn(move || {
            for stream in listener.incoming().map_while(Result::ok) {
                let identity = identity.clone();
                let seen = Arc::clone(&seen);
                let script = Arc::clone(&script);
                let sessions = Arc::clone(&sessions);
                std::thread::spawn(move || {
                    serve_one(stream, &identity, &seen, &script, &sessions);
                });
            }
        });
        FakeServer { url, calls }
    }

    /// How many `tools/call` of `tool` reached this server.
    fn received(&self, tool: &str) -> usize {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|t| *t == tool)
            .count()
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
    seen: &Mutex<Vec<String>>,
    script: &Script,
    sessions: &AtomicUsize,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let Some((method, path, body)) = read_request(&mut stream) else {
        return;
    };
    if method == "GET" && path == "/" {
        let index = json!({ "name": "majordomus", "repository_id": identity, "leaseholder": true });
        return respond(&mut stream, 200, &index.to_string(), None);
    }
    if method == "DELETE" {
        return respond(&mut stream, 204, "", None);
    }
    let message: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let id = message["id"].clone();
    match message["method"].as_str().unwrap_or_default() {
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
            let tool = message["params"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let n = {
                let mut calls = seen.lock().unwrap();
                calls.push(tool.clone());
                calls.iter().filter(|t| **t == tool).count()
            };
            match script(&tool, n) {
                Then::Answer => {
                    let result = json!({ "content": [{ "type": "text", "text": "done" }], "structuredContent": { "state": "ok", "attempt": n }, "isError": false });
                    let answer = json!({ "jsonrpc": "2.0", "id": id, "result": result });
                    respond(&mut stream, 200, &answer.to_string(), None);
                }
                Then::Drop => {}
                Then::Hang(d) => std::thread::sleep(d),
                Then::Status(s) => respond(&mut stream, s, r#"{"error":"refused"}"#, None),
            }
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
    fn spawn(f: &Fixture, args: &[&str]) -> Self {
        let mut all = vec!["mcp"];
        all.extend_from_slice(args);
        let mut child = Command::new(BIN)
            .args(&all)
            .current_dir(f.root())
            .env_remove("MAJORDOMUS_ROOT")
            .env_remove("MAJORDOMUS_URL")
            .env_remove("MAJORDOMUS_HTTP_HOST")
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
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "resend-test", "version": "0" } }),
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
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A bridge attached to a fake server that runs `script`.
fn bridged(script: Script) -> (Fixture, FakeServer, Mcp) {
    let f = Fixture::new();
    let server = FakeServer::start(&f.root(), script);
    server.plant_lease(&f);
    let mut bridge = Mcp::spawn(&f, &["--http-port", "0"]);
    bridge.wait_log("bridging this stdio session");
    bridge.initialize();
    (f, server, bridge)
}

/// A writing call the server received and never answered is not sent again: it may have
/// run, and the client is told that its outcome is unknown, with the request's id. Before
/// this, the bridge elected, found the same server, re-opened its session and sent the call
/// a second time.
#[test]
fn a_mutation_whose_answer_was_lost_runs_once() {
    let (_f, server, mut bridge) = bridged(Arc::new(|_, _| Then::Drop));
    let answer = bridge.call(7, WRITER, json!({ "id": "I0001", "status": "done" }));
    assert_eq!(
        server.received(WRITER),
        1,
        "the writing call reached the server once; stderr:\n{}",
        bridge.drain_log()
    );
    assert_eq!(answer["error"]["code"], -32603, "{answer}");
    let text = answer["error"]["message"].as_str().unwrap_or_default();
    assert!(text.contains("outcome unknown"), "{answer}");
    assert!(
        text.contains('7'),
        "the message names the request id: {answer}"
    );
}

/// A read whose answer was lost is idempotent by its declared hints: it is sent again, and
/// the client gets the answer of the second attempt.
#[test]
fn a_read_whose_answer_was_lost_is_sent_again_and_answered() {
    let (_f, server, mut bridge) =
        bridged(Arc::new(
            |_, n| {
                if n == 1 {
                    Then::Drop
                } else {
                    Then::Answer
                }
            },
        ));
    let answer = bridge.call(5, READER, json!({}));
    assert_eq!(
        answer["result"]["structuredContent"]["attempt"],
        2,
        "{answer}; stderr:\n{}",
        bridge.drain_log()
    );
    assert_eq!(server.received(READER), 2);
}

/// A 413 is answered as "payload too large", the call reaches the server once, and no
/// election follows: the server answered, so it is neither gone nor broken.
#[test]
fn a_rejected_request_is_answered_as_that_error_without_an_election() {
    let (_f, server, mut bridge) = bridged(Arc::new(|_, _| Then::Status(413)));
    let answer = bridge.call(9, WRITER, json!({ "id": "I0001", "status": "done" }));
    let log = bridge.drain_log();
    assert_eq!(server.received(WRITER), 1, "{answer}; stderr:\n{log}");
    let text = answer["error"]["message"].as_str().unwrap_or_default();
    assert!(text.contains("payload too large"), "{answer}");
    assert!(!text.contains("shared server unavailable"), "{answer}");
    assert!(!log.contains("electing again"), "no election: {log}");
}

/// A message over the server's body limit, sent through a bridge to a real shared server,
/// reaches the client as "payload too large" and starts no election.
#[test]
fn an_oversized_message_is_payload_too_large() {
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f, &["--http-port", "0"]);
    a.wait_log("listening on http://");
    let mut b = Mcp::spawn(&f, &["--http-port", "0"]);
    b.wait_log("bridging this stdio session");
    b.initialize();
    let pad = "x".repeat(majordomus_cli::http::server::MAX_BODY_BYTES + 1);
    let answer = b.call(
        11,
        WRITER,
        json!({ "id": "I0001", "status": "done", "pad": pad }),
    );
    let log = b.drain_log();
    let text = answer["error"]["message"].as_str().unwrap_or_default();
    assert!(
        text.contains("payload too large"),
        "{answer}; stderr:\n{log}"
    );
    assert!(!log.contains("electing again"), "no election: {log}");
    assert!(
        a.child.try_wait().unwrap().is_none(),
        "the server still runs"
    );
}

/// A writing call that outlasts the bridge's timeout reached the server once and is
/// reported as unanswered, a failure whose outcome is unknown: the class the session's
/// failover does not send again (`a_mutation_whose_answer_was_lost_runs_once` proves that
/// half through a real bridge process), where a read of the same class is sent again. The
/// timeout is the bridge's own, shortened through its constructor so that the test does not
/// wait the production minute.
#[test]
fn a_call_that_outlasts_the_timeout_runs_once_and_its_outcome_is_unknown() {
    use majordomus_cli::mcp::bridge::{Bridge, BridgeError};

    let f = Fixture::new();
    let server = FakeServer::start(
        &f.root(),
        Arc::new(|_, _| Then::Hang(Duration::from_secs(3))),
    );
    let mut bridge = Bridge::with_timeout(server.url.clone(), Duration::from_millis(500));
    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "in-process", "version": "0" } } });
    bridge.handle(&init).expect("the session opens");

    let started = Instant::now();
    let call = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": WRITER, "arguments": {} } });
    let err = bridge
        .handle(&call)
        .expect_err("no answer within the timeout");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "the bridge gave up at its own timeout, not the server's"
    );
    assert!(
        matches!(err, BridgeError::Unanswered { .. }),
        "a timeout leaves the outcome unknown: {err:?}"
    );
    assert_eq!(server.received(WRITER), 1, "sent once, inside the bridge");
    assert!(
        err.calls_for_an_election(),
        "a server that does not answer may be gone"
    );
    assert!(
        !err.may_resend(&call),
        "a write that may have run is not sent again"
    );
    let read = json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": READER, "arguments": {} } });
    assert!(err.may_resend(&read), "a read is");
}
