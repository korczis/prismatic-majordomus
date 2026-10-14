//! A server busy with slow calls still answers the probe an election sends (I2126).
//!
//! Every request was answered on one of four workers, so four calls that outlasted the probe
//! timeout made a healthy server look wedged to the next client, which then took its lease.
//! Here four slow calls hold a real socket's workers as they used to, and `GET /` — what the
//! lease probe asks — must still be answered promptly; and the whole contest is run with real
//! processes: a server holding the lease, four tool calls in flight that outlast the declared
//! probe timeout plus busy grace, and a fifth client that elects, attaches, and leaves the
//! lease file exactly as it found it.

mod common;

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use common::Fixture;
use majordomus_cli::http::{server, Router};
use majordomus_cli::lease::Timings;
use majordomus_cli::policy::ServerPolicy;
use serde_json::{json, Value};

fn get(url: &str, path: &str, timeout: Duration) -> Option<u16> {
    majordomus_cli::mcp::bridge::request(url, "GET", path, &[], None, timeout)
        .ok()
        .map(|r| r.status)
}

/// The probe timeout the fixture declares, in seconds.
const PROBE_TIMEOUT_SECONDS: u64 = 1;
/// The busy grace the fixture declares, in seconds.
const BUSY_GRACE_SECONDS: u64 = 2;
/// How long each slow tool call runs: well past the probe timeout plus the busy grace, so an
/// election that waited behind the calls would have given up on the owner and taken over.
const SLOW_CALL: Duration = Duration::from_secs(7);

/// A `majordomus mcp` child whose stderr lines arrive on a channel. Killed when dropped, so a
/// failed assertion leaves no server behind.
struct McpChild {
    child: Child,
    err: Receiver<String>,
    seen: Vec<String>,
}

impl McpChild {
    fn spawn(cwd: &Path, args: &[&str]) -> Self {
        let mut child = common::majordomus()
            .arg("mcp")
            .args(args)
            .current_dir(cwd)
            .env("MAJORDOMUS_LOG", "info")
            .env("MAJORDOMUS_SHARE", common::dist_share())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn majordomus mcp");
        let (tx, err) = mpsc::channel();
        let stderr = child.stderr.take().unwrap();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        McpChild {
            child,
            err,
            seen: Vec::new(),
        }
    }

    /// The first stderr line containing any of `needles`, within `within`.
    fn wait_for(&mut self, needles: &[&str], within: Duration) -> String {
        let deadline = Instant::now() + within;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.err.recv_timeout(left) {
                Ok(line) => {
                    self.seen.push(line.clone());
                    if needles.iter().any(|n| line.contains(n)) {
                        return line;
                    }
                }
                Err(_) => panic!(
                    "no stderr line containing any of {needles:?} within {within:?}:\n{}",
                    self.seen.join("\n")
                ),
            }
        }
    }

    /// Close stdin, the documented way to end a session, and wait a bounded time for the exit.
    fn close(&mut self) -> i32 {
        drop(self.child.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.code().unwrap_or(-1);
            }
            assert!(
                Instant::now() < deadline,
                "the session did not end:\n{}",
                self.seen.join("\n")
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for McpChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The URL in a `listening on http://...` or `already running at http://...` line.
fn url_in(line: &str) -> String {
    let rest = &line[line.find("http://").expect("a URL in the line")..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == ')' || c == ';' || c == ',')
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

/// Open an MCP session over HTTP, as a client that speaks it directly does; the session id.
fn open_session(url: &str) -> String {
    let init = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                    "clientInfo": { "name": "slow-caller", "version": "0" } } })
    .to_string();
    let reply = majordomus_cli::mcp::bridge::request(
        url,
        "POST",
        "/mcp",
        &[],
        Some(&init),
        Duration::from_secs(30),
    )
    .expect("initialize is answered");
    assert_eq!(reply.status, 200, "{}", reply.body);
    reply
        .header("mcp-session-id")
        .expect("initialize answers with a session id")
        .to_string()
}

#[test]
fn the_probe_is_answered_while_every_worker_would_be_busy() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let bound = server::bind("127.0.0.1", 0).expect("a free loopback port");
    let url = bound.url();
    let running = bound.start(Router::new(app.context.clone(), "test"));

    // as many slow calls as there are workers, and one more
    let slow: Vec<_> = (0..=server::WORKERS)
        .map(|_| {
            let url = url.clone();
            std::thread::spawn(move || {
                get(
                    &url,
                    "/api/v1/executions/demonstrate?steps=1&delay_ms=4000",
                    Duration::from_secs(30),
                )
            })
        })
        .collect();
    std::thread::sleep(Duration::from_millis(500));

    let asked = Instant::now();
    let probe = get(&url, "/", Duration::from_secs(10));
    let took = asked.elapsed();
    assert_eq!(probe, Some(200), "the probe was answered");
    assert!(
        took < Duration::from_secs(2),
        "the probe waited {took:?} behind the slow calls: a busy server looks wedged"
    );
    for call in slow {
        assert_eq!(
            call.join().unwrap(),
            Some(200),
            "every slow call is answered too"
        );
    }
    running.stop();
}

/// The acceptance scenario of I2126 with real processes: a server holds the lease, four MCP
/// tool calls are in flight on it that each outlast the declared probe timeout plus busy
/// grace, and a fifth client started in that window attaches to the running server instead
/// of taking it over — the lease file is byte for byte what it was before the calls began.
///
/// The timings are declared in the fixture's policy, which every process of the checkout
/// reads before it elects, so the contest is judged by one second of probe and two of grace
/// rather than by the compiled twelve, and the calls only have to run seven.
#[test]
fn busy_pool_is_not_taken_over_test() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/policy.yaml",
        &format!(
            "{}\nserver:\n  probe_timeout_seconds: {PROBE_TIMEOUT_SECONDS}\n  busy_grace_seconds: {BUSY_GRACE_SECONDS}\n",
            common::POLICY
        ),
    );
    f.commit("declare short lease timings");
    let declared = Timings::from_policy(&ServerPolicy {
        probe_timeout_seconds: Some(PROBE_TIMEOUT_SECONDS),
        busy_grace_seconds: Some(BUSY_GRACE_SECONDS),
        ..ServerPolicy::default()
    });
    let patience = declared.probe_timeout + declared.busy_grace;
    assert!(SLOW_CALL > patience + Duration::from_secs(2));

    let mut owner = McpChild::spawn(&f.root(), &["--http-port", "0"]);
    let url = url_in(&owner.wait_for(&["listening on http://"], Duration::from_secs(60)));
    let lease = f.path(".ai/local/state/mcp/server.json");
    let before = std::fs::read(&lease).expect("the owner wrote its lease");

    // four sessions, each with one slow tool call in flight
    let sessions: Vec<String> = (0..server::WORKERS).map(|_| open_session(&url)).collect();
    let calls_started = Instant::now();
    let calls: Vec<_> = sessions
        .iter()
        .cloned()
        .enumerate()
        .map(|(n, session)| {
            let url = url.clone();
            std::thread::spawn(move || {
                let call = json!({ "jsonrpc": "2.0", "id": n + 2, "method": "tools/call",
                    "params": { "name": "majordomus_demonstrate_execution",
                                "arguments": { "steps": 1,
                                               "delay_ms": SLOW_CALL.as_millis() as u64 } } })
                .to_string();
                let reply = majordomus_cli::mcp::bridge::request(
                    &url,
                    "POST",
                    "/mcp",
                    &[("Mcp-Session-Id", &session)],
                    Some(&call),
                    Duration::from_secs(60),
                );
                (reply, Instant::now())
            })
        })
        .collect();
    // the calls have reached the server and are running
    std::thread::sleep(Duration::from_millis(500));

    let fifth_started = Instant::now();
    let mut fifth = McpChild::spawn(&f.root(), &[]);
    let line = fifth.wait_for(
        &[
            "bridging this stdio session",
            "taking it over",
            "listening on http://",
            "serving this client alone",
        ],
        Duration::from_secs(60),
    );
    let attached = Instant::now();
    assert!(
        line.contains("bridging this stdio session"),
        "the fifth client did not attach to the busy server: {line}\n{}",
        fifth.seen.join("\n")
    );
    assert_eq!(
        url_in(&line),
        url,
        "it attached to the server that holds the lease"
    );
    assert!(
        attached < calls_started + SLOW_CALL,
        "the fifth client decided only after the slow calls ended: {:?}",
        attached - calls_started
    );
    assert_eq!(
        std::fs::read(&lease).expect("the lease is still there"),
        before,
        "the lease file changed while the server was busy"
    );

    for call in calls {
        let (reply, ended) = call.join().unwrap();
        let reply = reply.expect("every slow call is answered");
        assert_eq!(reply.status, 200, "{}", reply.body);
        let v: Value = serde_json::from_str(&reply.body).expect("a JSON-RPC answer");
        assert_eq!(v["result"]["isError"], false, "{v}");
        // the call was in flight for longer than the election would have waited on a server
        // that did not answer: an election stuck behind it would have taken the lease over
        assert!(
            ended.duration_since(fifth_started) > patience,
            "a call ended {:?} after the fifth client started, within the declared {patience:?}",
            ended.duration_since(fifth_started)
        );
    }
    assert_eq!(
        std::fs::read(&lease).expect("the lease is still there"),
        before,
        "the lease file changed once the calls were answered"
    );

    // the HTTP sessions are peers the server outlives its owner for: close them first
    for session in &sessions {
        let closed = majordomus_cli::mcp::bridge::request(
            &url,
            "DELETE",
            "/mcp",
            &[("Mcp-Session-Id", session)],
            None,
            Duration::from_secs(30),
        )
        .expect("a session is closed");
        assert!(closed.status < 300, "{}: {}", closed.status, closed.body);
    }
    assert_eq!(fifth.close(), 0, "the attached session ends cleanly");
    assert_eq!(owner.close(), 0, "the owner ends cleanly");
}
