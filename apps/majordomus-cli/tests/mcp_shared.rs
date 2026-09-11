//! The shared server as clients see it: the first `majordomus mcp` in a repository binds
//! loopback HTTP beside its stdio session and says where; every later one bridges its
//! stdio to it; peers see each other and what they announced; MCP over HTTP at `/mcp`
//! works for a client that speaks it directly; the server outlives its owner while a
//! peer is attached, ends when the last one leaves, and a bridged peer takes over when
//! its server dies. `--standalone` and `--inspect` touch no port and no lease. A storm of
//! clients released in the same instant still leaves one server, one board and a
//! repository nobody wrote to.

mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use common::{Fixture, BIN};
use serde_json::{json, Value};

const WAIT: Duration = Duration::from_secs(20);

/// A `majordomus mcp` child with its stdout frames and stderr lines readable with a timeout.
struct Mcp {
    child: Child,
    stdin: Option<ChildStdin>,
    out: Receiver<String>,
    err: Receiver<String>,
    seen_err: Vec<String>,
    next_id: u64,
}

impl Mcp {
    fn spawn(cwd: &Path, args: &[&str]) -> Self {
        let mut all = vec!["mcp"];
        all.extend_from_slice(args);
        let mut child = Command::new(BIN)
            .args(&all)
            .current_dir(cwd)
            .env("MAJORDOMUS_LOG", "info")
            .env("MAJORDOMUS_SHARE", common::dist_share())
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
            next_id: 1,
        }
    }

    /// Wait for a stderr line containing `needle`; every line read is kept.
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

    /// Drain whatever stderr has produced so far, without waiting.
    fn drain_log(&mut self) -> String {
        while let Ok(line) = self.err.try_recv() {
            self.seen_err.push(line);
        }
        self.seen_err.join("\n")
    }

    /// The URL from a `listening on http://...` or `already running at http://...` line.
    fn url_in(line: &str) -> String {
        let start = line.find("http://").expect("a URL in the line");
        let rest = &line[start..];
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ')' || c == ';' || c == ',')
            .unwrap_or(rest.len());
        rest[..end].to_string()
    }

    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin still open");
        writeln!(stdin, "{message}").unwrap();
        stdin.flush().unwrap();
    }

    /// Send a request and read its response frame.
    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let line = self.out.recv_timeout(WAIT).unwrap_or_else(|_| {
            panic!(
                "no stdout frame for {method} within {WAIT:?}; stderr:\n{}",
                self.drain_log()
            )
        });
        let v: Value = serde_json::from_str(&line).expect("a JSON frame");
        assert_eq!(v["id"], id, "frame answers the request: {line}");
        v
    }

    fn initialize(&mut self, client: &str) -> Value {
        let r = self.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": client, "version": "1.2.3" } }),
        );
        self.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        r
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        self.request(
            "tools/call",
            json!({ "name": tool, "arguments": arguments }),
        )["result"]
            .clone()
    }

    /// Close stdin and wait for the exit code.
    fn close(&mut self) -> i32 {
        drop(self.stdin.take());
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.code().unwrap_or(-1);
            }
            assert!(
                Instant::now() < deadline,
                "the process did not exit within {WAIT:?} after stdin closed; stderr:\n{}",
                self.drain_log()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn alive(&mut self) -> bool {
        self.child.try_wait().unwrap().is_none()
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One HTTP request to `url` (`http://host:port`); (status, headers lowercased, body).
fn http(
    url: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> (u16, Vec<(String, String)>, String) {
    let host = url.strip_prefix("http://").unwrap();
    let mut stream = TcpStream::connect(host).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let body = body.unwrap_or("");
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    stream.write_all(req.as_bytes()).unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    let text = String::from_utf8(raw).unwrap();
    let (head, body) = text.split_once("\r\n\r\n").expect("header/body split");
    let mut lines = head.lines();
    let status: u16 = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .filter_map(|l| {
            l.split_once(": ")
                .map(|(k, v)| (k.to_lowercase(), v.to_string()))
        })
        .collect();
    (status, headers, body.to_string())
}

fn get_json(url: &str, path: &str) -> (u16, Value) {
    let (status, _, body) = http(url, "GET", path, &[], None);
    let v = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("GET {path}: not JSON ({e}): {body}"));
    (status, v)
}

fn lease_path(f: &Fixture) -> std::path::PathBuf {
    f.path(".ai/local/state/mcp/server.json")
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
fn one_server_per_repository_and_peers_see_each_other() {
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let line = a.wait_log("listening on http://");
    let url = Mcp::url_in(&line);
    assert!(
        line.contains(&format!("swagger {url}/swagger")),
        "the log names Swagger UI at its own mount: {line}"
    );
    assert!(line.contains(&format!("{url}/openapi.json")), "{line}");
    assert!(line.contains(&format!("{url}/mcp")), "{line}");
    assert!(
        lease_path(&f).exists(),
        "the lease is written under .ai/local/"
    );
    let lease: Value =
        serde_json::from_str(&std::fs::read_to_string(lease_path(&f)).unwrap()).unwrap();
    assert_eq!(lease["url"], url);
    assert_eq!(lease["root"], f.root().to_str().unwrap());

    let init = a.initialize("claude-code");
    let instructions = init["result"]["instructions"].as_str().unwrap();
    assert!(
        instructions.contains(&url),
        "instructions name the server: {instructions}"
    );
    assert!(instructions.contains("You are peer p1"), "{instructions}");
    assert!(
        instructions.contains("majordomus_announce"),
        "{instructions}"
    );
    // A session that has announced nothing is told so, here, on every initialize — which
    // is also every reconnect. A re-established transport keeps the work and loses the
    // place on the board, and this is the only moment the protocol can say it.
    assert!(
        instructions.contains("You have not announced anything"),
        "a silent peer is not told it is silent: {instructions}"
    );
    assert!(
        instructions.contains("again if this connection is ever re-established"),
        "the reconnect case is not named: {instructions}"
    );

    let (status, index) = get_json(&url, "/");
    assert_eq!(status, 200);
    // the index names the repository and withholds where it sits on the host: whoever can
    // reach the socket has no use for the checkout path, and somebody else would
    assert_eq!(
        index["repository"],
        f.root().file_name().unwrap().to_str().unwrap()
    );
    assert!(
        !index.to_string().contains(f.root().to_str().unwrap()),
        "the index carries the checkout path: {index}"
    );
    assert!(
        index["surfaces"]
            .as_array()
            .expect("the index lists the surfaces")
            .iter()
            .any(|s| s["id"] == "mcp" && s["path"] == "/mcp"),
        "the shared server serves MCP over HTTP and says so: {index}"
    );
    let (status, _, html) = http(&url, "GET", "/swagger", &[], None);
    assert_eq!(status, 200);
    assert!(
        html.contains("swagger-ui-dist@"),
        "Swagger UI is served beside the stdio session"
    );
    let (status, doc) = get_json(&url, "/openapi.json");
    assert_eq!(status, 200);
    assert_eq!(
        doc["paths"]["/api/v1/peers"]["get"]["operationId"],
        "peers.list"
    );
    assert_eq!(
        doc["paths"]["/api/v1/peers/announce"]["post"]["operationId"],
        "peers.announce"
    );
    let (_, peers) = get_json(&url, "/api/v1/peers");
    assert_eq!(peers["count"], 1);
    assert_eq!(peers["peers"][0]["id"], "p1");
    assert_eq!(peers["peers"][0]["client"]["name"], "claude-code");
    assert_eq!(peers["peers"][0]["transport"], "stdio");
    assert!(peers.get("caller").is_none(), "plain HTTP has no caller");

    // a second client in the same repository: no second server, a bridge to the first
    let mut b = Mcp::spawn(&f.root(), &[]);
    let joined = b.wait_log("bridging this stdio session");
    assert!(joined.contains(&url), "{joined}");
    let init_b = b.initialize("codex");
    let instructions = init_b["result"]["instructions"].as_str().unwrap();
    assert!(instructions.contains("You are peer p2"), "{instructions}");
    assert!(
        instructions.contains("p1 claude-code"),
        "the peer list names the owner: {instructions}"
    );
    assert!(
        !b.drain_log().contains("listening on"),
        "the bridge binds nothing"
    );

    let announced = b.call(
        "majordomus_announce",
        json!({ "intent": "refactor the parser", "scope": ["lib/parse", " lib/lex "] }),
    );
    assert_eq!(announced["isError"], false, "{announced}");
    assert_eq!(announced["structuredContent"]["id"], "p2");
    assert_eq!(
        announced["structuredContent"]["announcement"]["scope"],
        json!(["lib/parse", "lib/lex"])
    );
    let seen = a.call("majordomus_peers", json!({}));
    let sc = &seen["structuredContent"];
    assert_eq!(sc["count"], 2);
    assert_eq!(sc["caller"], "p1");
    let p2 = sc["peers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "p2")
        .unwrap();
    assert_eq!(p2["client"]["name"], "codex");
    assert_eq!(p2["client"]["version"], "1.2.3");
    assert_eq!(p2["transport"], "http");
    assert_eq!(p2["announcement"]["intent"], "refactor the parser");

    let refused = b.call("majordomus_announce", json!({ "intent": "   " }));
    assert_eq!(
        refused["isError"], true,
        "a blank intent is refused: {refused}"
    );
    let (status, _, body) = http(
        &url,
        "POST",
        "/api/v1/peers/announce",
        &[],
        Some("{\"intent\":\"x\"}"),
    );
    assert_eq!(status, 422, "over plain HTTP there is no caller: {body}");
    assert!(body.contains("refused"));

    // the same answer through both: the stdio owner and the bridged peer
    let via_a = a.call(
        "majordomus_get",
        json!({ "uri": "majordomus://rule/project.alpha@1" }),
    );
    let via_b = b.call(
        "majordomus_get",
        json!({ "uri": "majordomus://rule/project.alpha@1" }),
    );
    assert_eq!(via_a["structuredContent"], via_b["structuredContent"]);

    assert_eq!(b.close(), 0, "a bridge ends with 0 when its client goes");
    let seen = a.call("majordomus_peers", json!({}));
    assert_eq!(
        seen["structuredContent"]["count"], 1,
        "the bridge told the server it left"
    );

    assert_eq!(a.close(), 0);
    assert!(
        !lease_path(&f).exists(),
        "the lease is removed when the server stops"
    );
    assert!(
        TcpStream::connect(url.strip_prefix("http://").unwrap()).is_err(),
        "the port is closed"
    );
}

#[test]
fn the_server_outlives_its_owner_while_a_peer_is_attached() {
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let url = Mcp::url_in(&a.wait_log("listening on http://"));
    a.initialize("claude-code");
    let mut b = Mcp::spawn(&f.root(), &[]);
    b.wait_log("bridging this stdio session");
    b.initialize("gemini-cli");
    drop(a.stdin.take());
    a.wait_log("serving until the last peer leaves");
    std::thread::sleep(Duration::from_millis(300));
    assert!(a.alive(), "the server waits for its peer");
    let tools = b.request("tools/list", json!({}));
    assert!(tools["result"]["tools"].as_array().unwrap().len() >= 8);
    let (_, peers) = get_json(&url, "/api/v1/peers");
    assert_eq!(
        peers["count"], 1,
        "the owner's stdio peer is gone, the bridge remains"
    );
    assert_eq!(peers["peers"][0]["client"]["name"], "gemini-cli");
    assert_eq!(b.close(), 0);
    let deadline = Instant::now() + WAIT;
    while a.alive() {
        assert!(
            Instant::now() < deadline,
            "the server did not stop after its last peer left:\n{}",
            a.drain_log()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(a.child.wait().unwrap().code(), Some(0));
    assert!(!lease_path(&f).exists());
}

#[test]
fn mcp_over_http_directly_with_sessions() {
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let url = Mcp::url_in(&a.wait_log("listening on http://"));
    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "direct-http", "version": "0" } } }).to_string();
    let (status, headers, body) = http(&url, "POST", "/mcp", &[], Some(&init));
    assert_eq!(status, 200, "{body}");
    let session = headers
        .iter()
        .find(|(k, _)| k == "mcp-session-id")
        .map(|(_, v)| v.clone())
        .expect("initialize answers with a session id");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["result"]["serverInfo"]["name"], "majordomus");
    assert!(v["result"]["instructions"]
        .as_str()
        .unwrap()
        .contains("You are peer p"));

    let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }).to_string();
    let (status, _, body) = http(
        &url,
        "POST",
        "/mcp",
        &[("Mcp-Session-Id", &session)],
        Some(&list),
    );
    assert_eq!(status, 200);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert!(v["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == "majordomus_peers"));

    let (status, _, body) = http(&url, "POST", "/mcp", &[], Some(&list));
    assert_eq!(
        status, 400,
        "a request without a session is refused: {body}"
    );
    assert!(body.contains("session_required"));
    let (status, _, body) = http(
        &url,
        "POST",
        "/mcp",
        &[("Mcp-Session-Id", "nope")],
        Some(&list),
    );
    assert_eq!(
        status, 404,
        "an unknown session is 404 so that the client re-initialises: {body}"
    );
    let (status, _, body) = http(
        &url,
        "POST",
        "/mcp",
        &[("Mcp-Session-Id", &session)],
        Some("not json"),
    );
    assert_eq!(status, 400, "{body}");
    let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string();
    let (status, _, body) = http(
        &url,
        "POST",
        "/mcp",
        &[("Mcp-Session-Id", &session)],
        Some(&note),
    );
    assert_eq!(
        (status, body.as_str()),
        (202, ""),
        "a notification is accepted with no body"
    );
    let (status, _, _) = http(&url, "GET", "/mcp", &[], None);
    assert_eq!(status, 405, "no server-initiated stream");

    let (_, peers) = get_json(&url, "/api/v1/peers");
    assert_eq!(
        peers["count"], 2,
        "the owner's stdio peer and the HTTP session"
    );
    assert!(peers["peers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["client"]["name"] == "direct-http" && p["transport"] == "http"));

    let announce = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "majordomus_announce", "arguments": { "intent": "reviewing docs" } } }).to_string();
    let (status, _, body) = http(
        &url,
        "POST",
        "/mcp",
        &[("Mcp-Session-Id", &session)],
        Some(&announce),
    );
    assert_eq!(status, 200);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(
        v["result"]["structuredContent"]["announcement"]["intent"],
        "reviewing docs"
    );

    let (status, _, _) = http(
        &url,
        "DELETE",
        "/mcp",
        &[("Mcp-Session-Id", &session)],
        None,
    );
    assert_eq!(status, 204);
    let (status, _, _) = http(
        &url,
        "POST",
        "/mcp",
        &[("Mcp-Session-Id", &session)],
        Some(&list),
    );
    assert_eq!(status, 404, "a deleted session is gone");
    let (_, peers) = get_json(&url, "/api/v1/peers");
    assert_eq!(peers["count"], 1);
    assert_eq!(a.close(), 0);
}

#[test]
fn a_stale_lease_is_taken_over() {
    let f = Fixture::new();
    let port = free_port();
    let stale = json!({ "schema": "majordomus-mcp-lease/v1", "pid": 1, "token": "old", "root": f.root(), "url": format!("http://127.0.0.1:{port}"), "started_at": "2026-01-01T00:00:00Z" });
    f.write(".ai/local/state/mcp/server.json", &stale.to_string());
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let line = a.wait_log("stale lease");
    assert!(line.contains(&format!("127.0.0.1:{port}")), "{line}");
    let url = Mcp::url_in(&a.wait_log("listening on http://"));
    let lease: Value =
        serde_json::from_str(&std::fs::read_to_string(lease_path(&f)).unwrap()).unwrap();
    assert_eq!(lease["url"], url, "the lease now names the live server");
    assert_ne!(lease["token"], "old");
    assert_eq!(a.close(), 0);
}

#[test]
fn a_taken_port_falls_back_to_a_free_one_and_serve_defers_to_the_running_server() {
    let f = Fixture::new();
    let holder = TcpListener::bind("127.0.0.1:0").unwrap();
    let taken = holder.local_addr().unwrap().port();
    let mut a = Mcp::spawn(&f.root(), &["--http-port", &taken.to_string()]);
    let warn = a.wait_log("taking a free port");
    assert!(
        warn.contains(&format!("cannot bind 127.0.0.1:{taken}")),
        "{warn}"
    );
    let url = Mcp::url_in(&a.wait_log("listening on http://"));
    assert_ne!(url, format!("http://127.0.0.1:{taken}"));
    drop(holder);

    // `serve` in the same repository does not start a second server
    let (code, out, err) = common::run_in(&f.root(), &["serve", "--port", "0"], "");
    assert_eq!(code, 0, "{err}");
    assert!(out.is_empty());
    assert!(
        err.contains("already running at") && err.contains(&url),
        "{err}"
    );
    assert!(!err.contains("listening on"), "{err}");
    assert_eq!(a.close(), 0);
}

#[test]
fn standalone_and_inspect_touch_no_port_and_no_lease() {
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f.root(), &["--standalone"]);
    a.wait_log("standalone");
    let init = a.initialize("claude-code");
    let instructions = init["result"]["instructions"].as_str().unwrap();
    assert!(!instructions.contains("http://"), "{instructions}");
    let peers = a.call("majordomus_peers", json!({}));
    assert_eq!(
        peers["structuredContent"]["count"], 1,
        "the one client is still a peer of its own process"
    );
    assert_eq!(a.close(), 0);
    assert!(!a.drain_log().contains("listening on"));
    assert!(!lease_path(&f).exists(), "standalone writes nothing");

    let (code, _, err) = common::run_in(&f.root(), &["mcp", "--inspect"], "");
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("listening on"));
    assert!(!lease_path(&f).exists(), "inspect writes nothing");
}

#[test]
fn a_bridged_peer_takes_over_when_its_server_dies() {
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let url_a = Mcp::url_in(&a.wait_log("listening on http://"));
    a.initialize("claude-code");
    let mut b = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    b.wait_log("bridging this stdio session");
    let init_b = b.initialize("codex");
    assert!(init_b["result"]["instructions"]
        .as_str()
        .unwrap()
        .contains("You are peer p2"));

    a.child.kill().unwrap();
    let _ = a.child.wait();

    let repo = b.call("majordomus_repository", json!({}));
    assert_eq!(
        repo["isError"], false,
        "the request after the crash is answered: {repo}"
    );
    assert_eq!(repo["structuredContent"]["state"], "ok");
    let log = b.drain_log();
    assert!(log.contains("electing again"), "{log}");
    assert!(log.contains("took over as the shared server"), "{log}");
    let line = log
        .lines()
        .find(|l| l.contains("listening on http://"))
        .expect("the new server says where it listens");
    let url_b = Mcp::url_in(line);
    assert_ne!(url_b, url_a);
    let lease: Value =
        serde_json::from_str(&std::fs::read_to_string(lease_path(&f)).unwrap()).unwrap();
    assert_eq!(lease["url"], url_b, "the lease names the new server");
    let peers = b.call("majordomus_peers", json!({}));
    let sc = &peers["structuredContent"];
    assert_eq!(sc["count"], 1);
    assert_eq!(
        sc["peers"][0]["client"]["name"], "codex",
        "the client's identity survived the takeover"
    );
    assert_eq!(sc["peers"][0]["transport"], "stdio");
    let (status, _, html) = http(&url_b, "GET", "/swagger", &[], None);
    assert_eq!(status, 200);
    assert!(html.contains("swagger-ui"));
    assert_eq!(b.close(), 0);
    assert!(!lease_path(&f).exists());
}

#[test]
fn a_bridged_peer_re_attaches_when_another_process_took_the_lease_first() {
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    a.wait_log("listening on http://");
    a.initialize("claude-code");
    let mut b = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    b.wait_log("bridging this stdio session");
    b.initialize("codex");
    // a round trip after the initialized notification, so that nothing of b's is still
    // in flight when a dies (a message in flight would make b take over itself)
    b.request("ping", json!({}));
    a.child.kill().unwrap();
    let _ = a.child.wait();
    // before b notices, a third client starts and becomes the server
    let mut c = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let url_c = Mcp::url_in(&c.wait_log("listening on http://"));
    c.initialize("gemini-cli");
    let repo = b.call("majordomus_repository", json!({}));
    assert_eq!(repo["isError"], false, "{repo}");
    let log = b.drain_log();
    assert!(log.contains("re-attached to the shared server"), "{log}");
    assert!(log.contains(&url_c), "{log}");
    let peers = c.call("majordomus_peers", json!({}));
    let names: Vec<&str> = peers["structuredContent"]["peers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["client"]["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"codex") && names.contains(&"gemini-cli"),
        "{names:?}"
    );
    b.send(&json!({ "jsonrpc": "2.0", "method": "notifications/cancelled" }));
    assert_eq!(b.close(), 0);
    assert_eq!(c.close(), 0);
}

#[test]
fn a_peer_that_cannot_serve_the_layer_says_so_instead_of_taking_over() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/project/broken.v1.md",
        "---\nid: project.broken\n",
    );
    f.commit("broken");
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    a.wait_log("listening on http://");
    a.initialize("claude-code");
    let mut b = Mcp::spawn(&f.root(), &["--http-port", "0", "--strict"]);
    b.wait_log("bridging this stdio session");
    b.initialize("codex");
    b.request("ping", json!({}));
    a.child.kill().unwrap();
    let _ = a.child.wait();
    // a notification after the crash produces no frame; a request produces an error frame
    b.send(&json!({ "jsonrpc": "2.0", "method": "notifications/cancelled" }));
    let answer = b.request("ping", json!({}));
    assert_eq!(answer["error"]["code"], -32603, "{answer}");
    let message = answer["error"]["message"].as_str().unwrap();
    assert!(message.contains("shared server unavailable"), "{message}");
    assert!(
        message.contains("refusing to serve under --strict"),
        "{message}"
    );
    let log = b.drain_log();
    assert!(
        log.contains("cannot take over as the shared server"),
        "{log}"
    );
    assert_eq!(b.close(), 0);
    assert!(
        !lease_path(&f).exists(),
        "the failed takeover left no lease behind"
    );
}

// ---------------------------------------------------------------- nothing left behind locks anyone out

/// Write the lease file by hand, whatever the fixture already has there.
fn plant_lease(f: &Fixture, text: &str) {
    let path = lease_path(f);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, text).unwrap();
}

/// Make a file look an hour old, so that no grace period applies to it.
fn age_file(path: &Path) {
    let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_modified(std::time::SystemTime::now() - Duration::from_secs(3600))
        .unwrap();
}

#[test]
fn a_corrupt_lease_is_taken_over() {
    let f = Fixture::new();
    plant_lease(&f, "{not a lease");
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let line = a.wait_log("corrupt lease");
    assert!(line.contains("taking it over"), "{line}");
    assert!(line.contains("server.json"), "the path is named: {line}");
    let url = Mcp::url_in(&a.wait_log("listening on http://"));
    let lease: Value =
        serde_json::from_str(&std::fs::read_to_string(lease_path(&f)).unwrap()).unwrap();
    assert_eq!(lease["url"], url, "the lease is now the live server's");
    let init = a.initialize("claude-code");
    assert!(init["result"]["instructions"]
        .as_str()
        .unwrap()
        .contains(&url));
    assert_eq!(a.close(), 0);
    assert!(!lease_path(&f).exists());
}

#[test]
fn a_lease_of_another_schema_and_an_old_empty_file_are_taken_over() {
    let f = Fixture::new();
    plant_lease(
        &f,
        r#"{"schema":"something/v9","url":"http://127.0.0.1:1"}"#,
    );
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let line = a.wait_log("corrupt lease");
    assert!(
        line.contains("not a majordomus-mcp-lease/v1 document"),
        "{line}"
    );
    a.wait_log("listening on http://");
    assert_eq!(a.close(), 0);

    // an empty file older than the bind grace: its owner died between creating and writing it
    plant_lease(&f, "");
    age_file(&lease_path(&f));
    let started = Instant::now();
    let mut b = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let line = b.wait_log("empty lease");
    assert!(line.contains("taking it over"), "{line}");
    b.wait_log("listening on http://");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "no grace is waited for a file that is already old: {:?}",
        started.elapsed()
    );
    assert_eq!(b.close(), 0);
}

#[test]
fn an_abandoned_lease_is_taken_over_without_waiting_when_it_is_old() {
    let f = Fixture::new();
    let doc = json!({ "schema": "majordomus-mcp-lease/v1", "pid": 1, "token": "old", "root": f.root(), "url": null, "started_at": "2026-01-01T00:00:00Z" });
    plant_lease(&f, &doc.to_string());
    age_file(&lease_path(&f));
    let started = Instant::now();
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let line = a.wait_log("abandoned lease");
    assert!(line.contains("never published a URL"), "{line}");
    a.wait_log("listening on http://");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(a.close(), 0);
}

#[test]
fn an_unwritable_lease_directory_degrades_to_a_standalone_session() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let dir = f.path(".ai/local/state/mcp");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(dir.join("probe"), "").is_ok() {
        // permissions do not bind this user (root): there is nothing to observe here
        let _ = std::fs::remove_file(dir.join("probe"));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let line = a.wait_log("serving this client alone");
    assert!(line.contains("cannot use the shared server"), "{line}");
    assert!(
        line.contains("server.json"),
        "the lease path is named: {line}"
    );
    let init = a.initialize("claude-code");
    assert_eq!(init["result"]["serverInfo"]["name"], "majordomus");
    let instructions = init["result"]["instructions"].as_str().unwrap();
    assert!(
        !instructions.contains("http://"),
        "no server URL is promised: {instructions}"
    );
    let repo = a.call("majordomus_repository", json!({}));
    assert_eq!(repo["isError"], false, "{repo}");
    let peers = a.call("majordomus_peers", json!({}));
    assert_eq!(peers["structuredContent"]["count"], 1, "{peers}");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let log = a.drain_log();
    assert!(!log.contains("listening on"), "no port was bound:\n{log}");
    assert_eq!(a.close(), 0);
    assert!(!lease_path(&f).exists());
}

#[test]
fn a_non_loopback_bind_is_warned_about() {
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f.root(), &["--http-host", "0.0.0.0", "--http-port", "0"]);
    let warn = a.wait_log("not a loopback address");
    assert!(warn.contains("0.0.0.0:"), "{warn}");
    assert!(warn.contains("bind 127.0.0.1"), "{warn}");
    a.wait_log("listening on http://0.0.0.0:");
    assert_eq!(a.close(), 0);
}

#[test]
fn sigterm_removes_the_lease_before_the_process_dies() {
    use std::os::unix::process::ExitStatusExt;
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    a.wait_log("listening on http://");
    assert!(lease_path(&f).exists());
    let sent = Command::new("kill")
        .args(["-TERM", &a.child.id().to_string()])
        .status()
        .unwrap();
    assert!(sent.success(), "kill -TERM");
    let status = a.child.wait().unwrap();
    assert_eq!(status.signal(), Some(15), "died of SIGTERM: {status:?}");
    assert!(
        !lease_path(&f).exists(),
        "the handler removed the lease before the process died"
    );
    // the next client finds no stale lease and starts cleanly
    let mut b = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    b.wait_log("listening on http://");
    let log = b.drain_log();
    assert!(
        !log.contains("stale lease") && !log.contains("taking it over"),
        "{log}"
    );
    assert_eq!(b.close(), 0);
}

#[test]
fn two_clients_starting_together_share_one_server() {
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let mut b = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let ia = a.initialize("first");
    let ib = b.initialize("second");
    for m in [&mut a, &mut b] {
        let peers = m.call("majordomus_peers", json!({}));
        assert_eq!(peers["structuredContent"]["count"], 2, "{peers}");
    }
    // exactly one serves and the other bridges; the log lines may still be arriving
    let deadline = Instant::now() + WAIT;
    let (la, lb) = loop {
        let (la, lb) = (a.drain_log(), b.drain_log());
        let serving = [&la, &lb]
            .iter()
            .filter(|l| l.contains("listening on http://"))
            .count();
        let bridging = [&la, &lb]
            .iter()
            .filter(|l| l.contains("bridging this stdio session"))
            .count();
        if (serving, bridging) == (1, 1) {
            break (la, lb);
        }
        assert!(
            Instant::now() < deadline,
            "one server and one bridge expected; a:\n{la}\nb:\n{lb}"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let a_serves = la.contains("listening on http://");
    let serving_log = if a_serves { &la } else { &lb };
    let url = Mcp::url_in(
        serving_log
            .lines()
            .find(|l| l.contains("listening on http://"))
            .unwrap(),
    );
    for init in [&ia, &ib] {
        let instructions = init["result"]["instructions"].as_str().unwrap();
        assert!(instructions.contains(&url), "{instructions}");
    }
    assert!(
        !la.contains("WARN") && !lb.contains("WARN"),
        "a clean start on both sides; a:\n{la}\nb:\n{lb}"
    );
    let (server, bridge) = if a_serves {
        (&mut a, &mut b)
    } else {
        (&mut b, &mut a)
    };
    assert_eq!(bridge.close(), 0);
    assert_eq!(server.close(), 0);
    assert!(!lease_path(&f).exists());
}

#[test]
fn mcp_over_http_refuses_malformed_traffic_and_keeps_serving() {
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    let url = Mcp::url_in(&a.wait_log("listening on http://"));
    a.initialize("owner");
    let (status, _, body) = http(&url, "GET", "/mcp", &[], None);
    assert_eq!(status, 405, "{body}");
    assert!(body.contains("method_not_allowed"), "{body}");
    let (status, _, body) = http(&url, "PUT", "/mcp", &[], Some("{}"));
    assert_eq!(status, 405, "{body}");
    let (status, _, body) = http(&url, "POST", "/mcp", &[], Some("not json"));
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("invalid_json"), "{body}");
    let ping = r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
    let (status, _, body) = http(
        &url,
        "POST",
        "/mcp",
        &[("Mcp-Session-Id", "bogus")],
        Some(ping),
    );
    assert_eq!(status, 404, "{body}");
    assert!(body.contains("session_not_found"), "{body}");

    // an oversized body is refused with 413; the write may be cut short by the server
    let big = format!(
        r#"{{"jsonrpc":"2.0","id":9,"method":"ping","params":{{"pad":"{}"}}}}"#,
        "x".repeat(2 * 1024 * 1024)
    );
    let host = url.strip_prefix("http://").unwrap();
    let mut stream = TcpStream::connect(host).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let head = format!(
        "POST /mcp HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        big.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(big.as_bytes());
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    let first = String::from_utf8_lossy(&raw);
    let first = first.lines().next().unwrap_or("").to_string();
    assert!(first.starts_with("HTTP/1.1 413"), "{first}");

    // a session opened over HTTP: a batch is answered as a batch, an unknown method as an
    // error frame, a notification with 202, and none of it disturbs the stdio session
    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "raw", "version": "0" } } }).to_string();
    let (status, headers, _) = http(&url, "POST", "/mcp", &[], Some(&init));
    assert_eq!(status, 200);
    let sid = headers
        .iter()
        .find(|(k, _)| k == "mcp-session-id")
        .map(|(_, v)| v.clone())
        .expect("a session id");
    let batch = json!([
        { "jsonrpc": "2.0", "id": 2, "method": "ping" },
        { "jsonrpc": "2.0", "id": 3, "method": "no/such" }
    ])
    .to_string();
    let (status, _, body) = http(
        &url,
        "POST",
        "/mcp",
        &[("Mcp-Session-Id", &sid)],
        Some(&batch),
    );
    assert_eq!(status, 200, "{body}");
    let answers: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(answers.as_array().map(Vec::len), Some(2), "{body}");
    assert_eq!(answers[0]["result"], json!({}));
    assert_eq!(answers[1]["error"]["code"], -32601, "{body}");
    let note = r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{}}"#;
    let (status, _, body) = http(
        &url,
        "POST",
        "/mcp",
        &[("Mcp-Session-Id", &sid)],
        Some(note),
    );
    assert_eq!(status, 202, "{body}");
    let peers = a.call("majordomus_peers", json!({}));
    assert_eq!(peers["structuredContent"]["count"], 2, "{peers}");
    let (status, _, _) = http(&url, "DELETE", "/mcp", &[("Mcp-Session-Id", &sid)], None);
    assert_eq!(status, 204);
    let (_, peers) = get_json(&url, "/api/v1/peers");
    assert_eq!(peers["count"], 1);
    assert_eq!(a.close(), 0);
}

#[test]
fn a_bridge_is_transparent_and_a_restarted_server_answers_the_same_bytes() {
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    a.wait_log("listening on http://");
    a.initialize("first");
    let mut b = Mcp::spawn(&f.root(), &[]);
    b.wait_log("bridging this stdio session");
    b.initialize("second");
    let probes: Vec<(&str, Value)> = vec![
        ("tools/list", json!({})),
        ("resources/list", json!({})),
        (
            "tools/call",
            json!({ "name": "majordomus_capabilities", "arguments": {} }),
        ),
        (
            "tools/call",
            json!({ "name": "majordomus_list", "arguments": {} }),
        ),
        (
            "tools/call",
            json!({ "name": "majordomus_repository", "arguments": {} }),
        ),
        (
            "resources/read",
            json!({ "uri": "majordomus://repository" }),
        ),
    ];
    let answers = |m: &mut Mcp| -> Vec<String> {
        probes
            .iter()
            .map(|(method, params)| m.request(method, params.clone())["result"].to_string())
            .collect()
    };
    let direct = answers(&mut a);
    let bridged = answers(&mut b);
    assert_eq!(
        direct, bridged,
        "a bridged session sees exactly what a local one sees"
    );
    assert_eq!(b.close(), 0);
    assert_eq!(a.close(), 0);
    // a fresh server in the same repository answers byte for byte the same
    let mut c = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    c.wait_log("listening on http://");
    c.initialize("third");
    let again = answers(&mut c);
    assert_eq!(
        direct, again,
        "the same repository yields the same projection after a restart"
    );
    assert_eq!(c.close(), 0);
}

#[test]
fn an_announcement_outlives_the_server_it_was_made_to() {
    // takeover: b announced through a; a dies; b takes over, and its own board carries
    // the announcement it made to a
    let f = Fixture::new();
    let mut a = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    a.wait_log("listening on http://");
    a.initialize("claude-code");
    let mut b = Mcp::spawn(&f.root(), &["--http-port", "0"]);
    b.wait_log("bridging this stdio session");
    b.initialize("codex");
    let announced = b.call(
        "majordomus_announce",
        json!({ "intent": "the ordering rule", "scope": ["apps"] }),
    );
    assert_eq!(announced["isError"], false, "{announced}");
    b.request("ping", json!({}));
    a.child.kill().unwrap();
    let _ = a.child.wait();
    let peers = b.call("majordomus_peers", json!({}));
    let log = b.drain_log();
    assert!(log.contains("took over as the shared server"), "{log}");
    assert!(
        log.contains("announcement was carried onto this server's board"),
        "{log}"
    );
    let sc = &peers["structuredContent"];
    let me = sc["peers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["client"]["name"] == "codex")
        .unwrap_or_else(|| panic!("the client is on its own board: {sc}"));
    assert_eq!(me["attached"], true);
    assert_eq!(
        me["announcement"]["intent"], "the ordering rule",
        "the announcement survived the takeover: {sc}"
    );
    assert_eq!(me["announcement"]["scope"][0], "apps");
    assert_eq!(b.close(), 0);

    // re-attach: b announced through a; a dies; c becomes the server first; b's next
    // message re-attaches to c, and the bridge says the announcement again
    let g = Fixture::new();
    let mut a = Mcp::spawn(&g.root(), &["--http-port", "0"]);
    a.wait_log("listening on http://");
    a.initialize("claude-code");
    let mut b = Mcp::spawn(&g.root(), &["--http-port", "0"]);
    b.wait_log("bridging this stdio session");
    b.initialize("codex");
    let announced = b.call(
        "majordomus_announce",
        json!({ "intent": "the installer", "scope": ["site"] }),
    );
    assert_eq!(announced["isError"], false, "{announced}");
    b.request("ping", json!({}));
    a.child.kill().unwrap();
    let _ = a.child.wait();
    let mut c = Mcp::spawn(&g.root(), &["--http-port", "0"]);
    c.wait_log("listening on http://");
    c.initialize("gemini-cli");
    let repo = b.call("majordomus_repository", json!({}));
    assert_eq!(repo["isError"], false, "{repo}");
    // `drain_log` is non-blocking, and the line this proves may still be in the pipe when
    // the answer to the request has already arrived: waiting for the last of them —
    // "re-attached" is logged after the replay it follows — is what makes the log complete
    // rather than merely current. Under load this raced and lost.
    b.wait_log("re-attached to the shared server");
    let log = b.drain_log();
    assert!(log.contains("announcement was replayed"), "{log}");
    let peers = c.call("majordomus_peers", json!({}));
    let sc = &peers["structuredContent"];
    let codex = sc["peers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["client"]["name"] == "codex")
        .unwrap_or_else(|| panic!("the re-attached client is on c's board: {sc}"));
    assert_eq!(
        codex["announcement"]["intent"], "the installer",
        "the announcement reached the server the client re-attached to: {sc}"
    );
    b.send(&json!({ "jsonrpc": "2.0", "method": "notifications/cancelled" }));
    assert_eq!(b.close(), 0);
    assert_eq!(c.close(), 0);
}

// ---------------------------------------------------------------- the storm

/// How many clients start in the same instant. Two were tested, and two is not a storm: the
/// race the election has to survive is the one where several processes read the same absent
/// lease in the same millisecond, and only a barrier makes that happen on purpose.
const STORM: usize = 6;

#[test]
fn a_storm_of_clients_converges_on_one_server_and_one_board() {
    let f = Fixture::new();
    let before = f.snapshot();
    let root = f.root();
    // released together, so the election is decided under contention rather than in the
    // order a test happened to spawn its children
    let gate = std::sync::Arc::new(std::sync::Barrier::new(STORM));
    let mut clients: Vec<(usize, Mcp, Value)> = (0..STORM)
        .map(|i| {
            let root = root.clone();
            let gate = std::sync::Arc::clone(&gate);
            std::thread::spawn(move || {
                gate.wait();
                let mut m = Mcp::spawn(&root, &["--http-port", "0"]);
                let init = m.initialize(&format!("storm-{i}"));
                (i, m, init)
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().expect("a client thread"))
        .collect();

    // exactly one bound a port; every other one bridged to it. The log lines may still be
    // arriving when the last initialize has been answered, so this is waited for.
    let deadline = Instant::now() + WAIT;
    let logs = loop {
        let logs: Vec<String> = clients.iter_mut().map(|(_, m, _)| m.drain_log()).collect();
        let serving = logs
            .iter()
            .filter(|l| l.contains("listening on http://"))
            .count();
        let bridging = logs
            .iter()
            .filter(|l| l.contains("bridging this stdio session"))
            .count();
        if (serving, bridging) == (1, STORM - 1) {
            break logs;
        }
        assert!(
            Instant::now() < deadline,
            "one server and {} bridges expected, found {serving} and {bridging}:\n{}",
            STORM - 1,
            logs.join("\n---\n")
        );
        std::thread::sleep(Duration::from_millis(100));
    };
    let owner = logs
        .iter()
        .position(|l| l.contains("listening on http://"))
        .expect("one of them serves");
    let url = Mcp::url_in(
        logs[owner]
            .lines()
            .find(|l| l.contains("listening on http://"))
            .unwrap(),
    );
    let lease: Value =
        serde_json::from_str(&std::fs::read_to_string(lease_path(&f)).unwrap()).unwrap();
    assert_eq!(lease["url"], url, "the lease names the one server");

    // every client was told about that server, and each was given an identity of its own
    let mut ids = std::collections::BTreeSet::new();
    for (i, _, init) in &clients {
        let instructions = init["result"]["instructions"]
            .as_str()
            .unwrap_or_else(|| panic!("client {i} was not told anything: {init}"));
        assert!(
            instructions.contains(&url),
            "client {i} was not told about the one server: {instructions}"
        );
        let id: String = instructions
            .split("You are peer ")
            .nth(1)
            .and_then(|rest| rest.split(|c: char| !c.is_ascii_alphanumeric()).next())
            .unwrap_or_else(|| panic!("client {i} was given no peer id: {instructions}"))
            .to_string();
        assert!(ids.insert(id.clone()), "peer id {id} was handed out twice");
    }
    assert_eq!(ids.len(), STORM, "{ids:?}");

    // each of them has a working session: it announces a scope of its own and the board
    // takes it without finding a collision that is not there
    for (i, m, _) in clients.iter_mut() {
        let announced = m.call(
            "majordomus_announce",
            json!({ "intent": format!("storm client {i}"), "scope": [format!("lib/storm-{i}")] }),
        );
        assert_eq!(announced["isError"], false, "client {i}: {announced}");
        // an absent `overlaps` and an empty one are the same answer: nothing was held
        assert_eq!(
            announced["structuredContent"]["overlaps"]
                .as_array()
                .map_or(0, Vec::len),
            0,
            "client {i} collided with a scope nobody holds: {announced}"
        );
    }

    // and reads the layer through it, and sees the whole board
    let mut boards: Vec<(String, Vec<(String, String, String)>)> = Vec::new();
    for (i, m, _) in clients.iter_mut() {
        let repo = m.call("majordomus_repository", json!({}));
        assert_eq!(
            repo["isError"], false,
            "client {i} has no working session: {repo}"
        );
        assert_eq!(repo["structuredContent"]["state"], "ok", "client {i}");
        let peers = m.call("majordomus_peers", json!({}));
        let sc = &peers["structuredContent"];
        assert_eq!(
            sc["count"].as_u64(),
            Some(STORM as u64),
            "client {i} sees a partial board: {sc}"
        );
        let mut seen: Vec<(String, String, String)> = sc["peers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p["id"].as_str().unwrap_or_default().to_string(),
                    p["client"]["name"].as_str().unwrap_or_default().to_string(),
                    p["announcement"]["intent"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                )
            })
            .collect();
        seen.sort();
        boards.push((sc["caller"].as_str().unwrap_or_default().to_string(), seen));
    }

    // one board, not six: whoever won the election, every client read the same thing
    let (_, first) = &boards[0];
    for (caller, seen) in &boards {
        assert_eq!(seen, first, "the board {caller} read differs from p1's");
    }
    let names: std::collections::BTreeSet<&str> =
        first.iter().map(|(_, name, _)| name.as_str()).collect();
    let expected: std::collections::BTreeSet<String> =
        (0..STORM).map(|i| format!("storm-{i}")).collect();
    assert_eq!(
        names,
        expected.iter().map(String::as_str).collect(),
        "the board names every client that started"
    );
    let callers: std::collections::BTreeSet<&str> =
        boards.iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(
        callers.len(),
        STORM,
        "each client answers as itself: {callers:?}"
    );
    assert_eq!(
        callers,
        ids.iter().map(String::as_str).collect(),
        "the identity a client was given at initialize is the one the board calls it"
    );
    for (_, name, intent) in first {
        let i = name.strip_prefix("storm-").expect(name);
        assert_eq!(intent, &format!("storm client {i}"), "{first:?}");
    }

    // the bridges leave, then the one that serves; the lease goes with it and the
    // repository is what it was before any of them started
    for (i, m, _) in clients.iter_mut() {
        if *i != owner {
            assert_eq!(m.close(), 0, "client {i} did not end cleanly");
        }
    }
    assert_eq!(
        clients[owner].1.close(),
        0,
        "the server did not end cleanly"
    );
    assert!(
        !lease_path(&f).exists(),
        "the lease outlived the last client of the storm"
    );
    assert_eq!(before, f.snapshot(), "the storm changed the repository");
}
