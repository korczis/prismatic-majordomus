//! The execution plane as a client sees it: a real server on a loopback port, real HTTP
//! requests, and a real WebSocket spoken by a client written here.
//!
//! Nothing is mocked at the boundary that matters. The socket is a `TcpStream`, the
//! handshake is the one RFC 6455 specifies, the frames are parsed from bytes, and the
//! assertions are about what a browser would actually receive.

mod common;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use common::{Fixture, Served};
use serde_json::{json, Value};

/// A WebSocket client: enough of RFC 6455 to read what this server writes.
struct Live {
    stream: TcpStream,
    buffer: Vec<u8>,
}

impl Live {
    fn open(address: &str, target: &str) -> Live {
        let mut stream = TcpStream::connect(address).expect("connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("a read timeout");
        let request = format!(
            "GET {target} HTTP/1.1\r\nHost: {address}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).expect("write");
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            let n = stream.read(&mut byte).expect("read the handshake");
            assert_eq!(n, 1, "the server closed during the handshake");
            head.push(byte[0]);
        }
        let head = String::from_utf8_lossy(&head).into_owned();
        assert!(head.starts_with("HTTP/1.1 101"), "{head}");
        assert!(
            head.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="),
            "the accept value RFC 6455 specifies for this key: {head}"
        );
        Live {
            stream,
            buffer: Vec::new(),
        }
    }

    /// The next text frame, as JSON. `None` when the connection ended or nothing arrived.
    fn next(&mut self) -> Option<Value> {
        loop {
            if let Some(frame) = self.take() {
                match frame {
                    Some(text) => {
                        return Some(serde_json::from_str(&text).expect("a frame is JSON"))
                    }
                    None => continue, // a ping or a close payload: not a message
                }
            }
            let mut chunk = [0u8; 8192];
            match self.stream.read(&mut chunk) {
                Ok(0) | Err(_) => return None,
                Ok(n) => self.buffer.extend_from_slice(&chunk[..n]),
            }
        }
    }

    /// One whole frame from the buffer: `Some(Some(text))` for a text frame,
    /// `Some(None)` for any other, `None` when the buffer holds no whole frame.
    fn take(&mut self) -> Option<Option<String>> {
        if self.buffer.len() < 2 {
            return None;
        }
        assert_eq!(
            self.buffer[0] & 0x80,
            0x80,
            "a server frame is never fragmented"
        );
        assert_eq!(self.buffer[1] & 0x80, 0, "a server frame is never masked");
        let opcode = self.buffer[0] & 0x0f;
        let (length, header) = match self.buffer[1] & 0x7f {
            126 if self.buffer.len() >= 4 => (
                u16::from_be_bytes([self.buffer[2], self.buffer[3]]) as usize,
                4,
            ),
            127 if self.buffer.len() >= 10 => (
                u64::from_be_bytes(self.buffer[2..10].try_into().unwrap()) as usize,
                10,
            ),
            n if n < 126 => (n as usize, 2),
            _ => return None,
        };
        if self.buffer.len() < header + length {
            return None;
        }
        let payload: Vec<u8> = self.buffer.drain(..header + length).skip(header).collect();
        Some(if opcode == 0x1 {
            Some(String::from_utf8(payload).expect("a text frame is UTF-8"))
        } else {
            None
        })
    }

    /// Read frames until one of `types` arrives, or time runs out.
    fn until(&mut self, types: &[&str]) -> Vec<Value> {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            let Some(frame) = self.next() else { break };
            let last = frame["type"].as_str().unwrap_or_default().to_string();
            seen.push(frame);
            if types.contains(&last.as_str()) {
                return seen;
            }
        }
        seen
    }
}

fn types(frames: &[Value]) -> Vec<String> {
    frames
        .iter()
        .map(|f| f["type"].as_str().unwrap_or_default().to_string())
        .collect()
}

fn start(s: &Served, capability: &str, input: Value) -> Value {
    let body = json!({ "capability": capability, "input": input }).to_string();
    let (status, _, text) = s.request("POST", "/api/v1/executions/start", Some(&body));
    assert_eq!(status, 200, "{text}");
    serde_json::from_str(&text).expect("the answer is JSON")
}

/// Scenario A, end to end: a capability is started over HTTP, streamed over the WebSocket,
/// and read back identically from the snapshot and from the history.
#[test]
fn an_execution_is_started_over_http_and_streamed_over_the_socket() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    // the protocol says where the channel is; nothing here writes the path down
    let (status, protocol) = s.get("/api/v1/executions/protocol");
    assert_eq!(status, 200);
    let channel = protocol["websocket"].as_str().expect("a websocket path");
    assert_eq!(protocol["protocol_version"], "1");
    assert!(protocol["event_types"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t == "execution.completed"));

    let mut live = Live::open(&s.address, channel);
    let started = start(
        &s,
        "executions.demonstrate",
        json!({ "steps": 2, "delay_ms": 20 }),
    );
    let id = started["id"].as_str().expect("an id").to_string();
    assert_eq!(started["state"], "queued");
    assert_eq!(
        started["links"]["cockpit"],
        format!("/cockpit/executions/{id}")
    );

    let frames = live.until(&["execution.completed"]);
    let seen = types(&frames);
    assert_eq!(seen.first().map(String::as_str), Some("stream.ready"));
    for required in [
        "execution.created",
        "execution.started",
        "execution.step.started",
        "execution.log",
        "execution.step.completed",
        "execution.progress",
        "execution.completed",
    ] {
        assert!(
            seen.contains(&required.to_string()),
            "{required} not in {seen:?}"
        );
    }
    // sequences are dense and ordered, which is what a client deduplicates on
    let sequences: Vec<u64> = frames
        .iter()
        .filter_map(|f| f["sequence"].as_u64())
        .collect();
    assert!(
        sequences.windows(2).all(|w| w[1] == w[0] + 1),
        "{sequences:?}"
    );
    assert!(frames
        .iter()
        .all(|f| f["schema_version"].as_str() == Some("1") || f["type"] == "stream.ready"));

    // the snapshot and the last event agree, and the snapshot carries what it produced
    let (status, snapshot) = s.get(&format!("/api/v1/executions/get?id={id}"));
    assert_eq!(status, 200);
    assert_eq!(snapshot["state"], "succeeded");
    assert_eq!(snapshot["output"]["steps"], 2);
    assert_eq!(
        snapshot["output"]["observed"], true,
        "the handler knew something was watching"
    );
    assert_eq!(snapshot["steps"].as_array().unwrap().len(), 2);
    assert_eq!(snapshot["progress"]["current"], 2);

    // and the history reconstructs the same lifecycle
    let (status, history) = s.get(&format!("/api/v1/executions/events?id={id}"));
    assert_eq!(status, 200);
    assert_eq!(history["state"], snapshot["state"]);
    assert_eq!(history["last_sequence"], snapshot["last_sequence"]);
    let from_history: Vec<String> = history["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["type"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        from_history,
        seen.iter()
            .filter(|t| t.starts_with("execution."))
            .cloned()
            .collect::<Vec<_>>(),
        "the stream and the history are the same events in the same order"
    );

    // the listing sees it, and so does the Cockpit page a person would open
    let (_, list) = s.get("/api/v1/executions");
    assert!(list["executions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["id"] == id.as_str()));
    let (status, _, page) = s.request_with(
        "GET",
        &format!("/cockpit/executions/{id}"),
        None,
        &[("Accept", "text/html")],
    );
    assert_eq!(status, 200);
    assert!(page.contains(&id), "the page names the execution");
    assert!(page.contains("succeeded"));
    assert!(
        page.contains("data-mj-socket"),
        "the page carries the live channel it would follow"
    );
}

/// Scenario B: a failure is streamed, rendered and readable, with no Rust internals in it.
#[test]
fn a_failure_is_reported_as_a_diagnostic_and_never_as_a_backtrace() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let started = start(
        &s,
        "executions.demonstrate",
        json!({ "steps": 3, "delay_ms": 0, "fail_at": 2 }),
    );
    let id = started["id"].as_str().unwrap().to_string();
    let mut live = Live::open(&s.address, &format!("/events?execution={id}"));
    let frames = live.until(&["execution.failed"]);
    let failure = frames
        .iter()
        .find(|f| f["type"] == "execution.failed")
        .expect("a failure event");
    assert_eq!(failure["data"]["error"]["code"], "internal");
    assert_eq!(failure["data"]["error"]["correlation_id"], id.as_str());
    let text = failure.to_string();
    assert!(
        !text.contains("panicked") && !text.contains("src/"),
        "{text}"
    );

    let (_, snapshot) = s.get(&format!("/api/v1/executions/get?id={id}"));
    assert_eq!(snapshot["state"], "failed");
    assert!(snapshot["output"].is_null(), "a failure carries no output");
    assert_eq!(
        snapshot["error"]["message"], failure["data"]["error"]["message"],
        "the snapshot and the final event agree"
    );
}

/// Scenario C: cancellation is real. The task stops, the state is `cancelled`, and a
/// capability that does not look at its flag says so instead of pretending.
#[test]
fn cancelling_stops_a_task_and_is_refused_honestly_by_what_cannot_be_stopped() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let started = start(
        &s,
        "executions.demonstrate",
        json!({ "steps": 40, "delay_ms": 250 }),
    );
    let id = started["id"].as_str().unwrap().to_string();
    let mut live = Live::open(&s.address, &format!("/events?execution={id}"));
    // wait until it is really running before asking it to stop
    live.until(&["execution.step.started"]);

    let (status, _, text) = s.request(
        "POST",
        "/api/v1/executions/cancel",
        Some(&json!({ "id": id }).to_string()),
    );
    assert_eq!(status, 200, "{text}");
    let report: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(report["outcome"], "requested");
    assert_eq!(report["cancellable"], true);

    let frames = live.until(&["execution.cancelled"]);
    assert!(types(&frames).contains(&"execution.cancelling".to_string()));
    let (_, snapshot) = s.get(&format!("/api/v1/executions/get?id={id}"));
    assert_eq!(snapshot["state"], "cancelled");
    assert!(
        snapshot["steps"].as_array().unwrap().len() < 40,
        "it stopped early rather than running to the end"
    );

    // a query does not look at its flag, and the policy on the descriptor says so
    let (_, capability) = s.get("/api/v1/capability?id=health.report");
    assert_eq!(capability["execution"]["cancellable"], false);
    assert_eq!(capability["execution"]["effect"], "read");
    let (_, verify) = s.get("/api/v1/capability?id=objects.verify");
    assert_eq!(verify["kind"], "query", "how long it takes is not a kind");
    assert_eq!(
        verify["execution"]["cancellable"], true,
        "its handler looks at the flag, and its descriptor says so"
    );
}

/// Scenario D and E together: a client that goes away, comes back with the sequence it
/// last saw, and is given exactly the gap — no repeat and no hole. This is what a browser
/// reload and a dropped connection both do.
#[test]
fn a_reconnection_asks_for_the_gap_by_cursor_and_gets_exactly_it() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let started = start(
        &s,
        "executions.demonstrate",
        json!({ "steps": 6, "delay_ms": 120 }),
    );
    let id = started["id"].as_str().unwrap().to_string();

    let mut first = Live::open(&s.address, &format!("/events?execution={id}"));
    let early = first.until(&["execution.step.completed"]);
    let cursor = early
        .iter()
        .filter_map(|f| f["sequence"].as_u64())
        .max()
        .expect("a cursor");
    drop(first); // the client goes away; the execution does not

    let mut second = Live::open(
        &s.address,
        &format!("/events?execution={id}&after={cursor}"),
    );
    let rest = second.until(&["execution.completed"]);
    let ready = rest.first().expect("a ready frame");
    assert_eq!(ready["type"], "stream.ready");
    assert_eq!(ready["data"]["scope"], "executions");
    let sequences: Vec<u64> = rest.iter().filter_map(|f| f["sequence"].as_u64()).collect();
    assert!(
        sequences.iter().all(|n| *n > cursor),
        "nothing before the cursor was written again: {sequences:?}"
    );
    assert_eq!(
        sequences.first().copied(),
        Some(cursor + 1),
        "and nothing was skipped"
    );
    assert!(
        sequences.windows(2).all(|w| w[1] == w[0] + 1),
        "{sequences:?}"
    );
    let (_, snapshot) = s.get(&format!("/api/v1/executions/get?id={id}"));
    assert_eq!(snapshot["state"], "succeeded");
}

/// Scenario F: the server is the authority on input, and says which constraint failed.
#[test]
fn the_server_validates_the_input_and_names_what_is_wrong() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let bad = |body: Value| {
        let (status, _, text) =
            s.request("POST", "/api/v1/executions/start", Some(&body.to_string()));
        let v: Value = serde_json::from_str(&text).expect("an error body is JSON");
        (
            status,
            v["error"]["code"].as_str().unwrap_or("").to_string(),
        )
    };
    assert_eq!(
        bad(json!({ "capability": "executions.demonstrate", "input": { "steps": "many" } })),
        (400, "invalid_input".into()),
        "a value of the wrong type"
    );
    assert_eq!(
        bad(json!({ "capability": "executions.demonstrate", "input": { "nope": 1 } })),
        (400, "invalid_input".into()),
        "a property the schema does not allow"
    );
    assert_eq!(
        bad(json!({ "capability": "nothing.here" })),
        (404, "not_found".into())
    );
    let (_, objects) = s.get("/api/v1/objects");
    let a_resource = objects["objects"]
        .as_array()
        .and_then(|all| all.first())
        .map(|o| {
            format!(
                "{}.{}",
                o["kind"].as_str().unwrap(),
                o["identity"].as_str().unwrap()
            )
        })
        .expect("a fixture repository holds objects");
    assert_eq!(
        bad(json!({ "capability": a_resource })),
        (422, "refused".into()),
        "a resource is read, not run"
    );
    // and nothing was created by any of them
    let (_, list) = s.get("/api/v1/executions");
    assert_eq!(list["count"], 0);
}

/// One raw upgrade request, answered rather than upgraded. `Served::request` cannot be
/// used: it sends `Connection: close`, which is not an upgrade, and the point here is what
/// happens to a request that *is* one.
fn refused_upgrade(address: &str, target: &str, extra: &str) -> u16 {
    let mut stream = TcpStream::connect(address).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("a read timeout");
    let request = format!(
        "GET {target} HTTP/1.1\r\nHost: {address}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n{extra}\r\n"
    );
    stream.write_all(request.as_bytes()).expect("write");
    let mut raw = Vec::new();
    let _ = stream.read_to_end(&mut raw);
    let text = String::from_utf8_lossy(&raw).into_owned();
    text.split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("no status in {text}"))
}

/// The live channel refuses what it should before it upgrades anything.
#[test]
fn the_live_channel_refuses_a_foreign_origin_and_a_plain_request() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, _, body) = s.request("GET", "/events", None);
    assert_eq!(status, 426, "{body}");
    assert!(
        body.contains("executions/events"),
        "it says where else to look"
    );

    assert_eq!(
        refused_upgrade(
            &s.address,
            "/events",
            "Origin: https://elsewhere.example\r\n"
        ),
        403,
        "a page on another origin may not follow this"
    );
    assert_eq!(
        refused_upgrade(&s.address, "/events?execution=../../etc/passwd", ""),
        400,
        "an id that is not one is refused, never looked up"
    );
    assert_eq!(
        refused_upgrade(&s.address, "/events?follow=everything", ""),
        400,
        "a parameter this channel does not take"
    );
}

/// A capability that existed before the execution plane, unchanged, run as an execution:
/// the same handler, the same route, the same answer, now with its steps visible.
#[test]
fn an_existing_capability_is_observable_without_a_second_implementation() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, direct) = s.get("/api/v1/health");
    assert_eq!(status, 200);

    let started = start(&s, "health.report", json!({}));
    let id = started["id"].as_str().unwrap().to_string();
    let mut live = Live::open(&s.address, &format!("/events?execution={id}"));
    live.until(&["execution.completed"]);

    let (_, snapshot) = s.get(&format!("/api/v1/executions/get?id={id}"));
    assert_eq!(snapshot["state"], "succeeded");
    assert_eq!(
        snapshot["output"]["checks"].as_array().map(Vec::len),
        direct["checks"].as_array().map(Vec::len),
        "the execution ran the same handler the route runs"
    );
    assert!(
        snapshot["steps"].as_array().is_some_and(|s| !s.is_empty()),
        "and reported the dimensions it decided as steps: {snapshot}"
    );
    let names: Vec<&str> = snapshot["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    let dimensions: Vec<&str> = direct["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(names, dimensions, "one step per dimension, in order");
}

/// The real long-running capability: it reads every object of the layer, reports its way
/// through them, and tells the truth about an edit made after the index was built.
#[test]
fn verifying_the_index_reports_object_by_object_and_finds_a_file_that_moved() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let started = start(&s, "objects.verify", json!({}));
    let id = started["id"].as_str().unwrap().to_string();
    let mut live = Live::open(&s.address, &format!("/events?execution={id}"));
    let frames = live.until(&["execution.completed"]);
    let progress: Vec<&Value> = frames
        .iter()
        .filter(|f| f["type"] == "execution.progress")
        .collect();
    assert!(
        progress.len() > 1,
        "it reports its way through the objects, not once at the end"
    );
    assert!(progress
        .last()
        .and_then(|p| p["data"]["total"].as_u64())
        .is_some_and(|total| total > 0));

    let (_, snapshot) = s.get(&format!("/api/v1/executions/get?id={id}"));
    assert_eq!(snapshot["state"], "succeeded");
    let report = &snapshot["output"];
    assert_eq!(
        report["index_is_current"], true,
        "a checkout nobody has touched since the server started: {report}"
    );
    assert!(
        report["files"].as_u64().unwrap_or(0) > 0,
        "there were files to read"
    );
    assert!(report["objects"].as_u64().unwrap_or(0) >= report["files"].as_u64().unwrap());
    assert!(
        report["fingerprint"]
            .as_str()
            .is_some_and(|f| f.len() == 64),
        "the index fingerprint this answer is about"
    );

    // now change a file the index read, and ask again
    let subject = f.path(".ai/repo/policy.yaml");
    let text = std::fs::read_to_string(&subject).expect("the policy the index read");
    std::fs::write(
        &subject,
        format!("{text}\n# edited after the index was built\n"),
    )
    .unwrap();
    let after = start(&s, "objects.verify", json!({}));
    let id = after["id"].as_str().unwrap().to_string();
    let mut live = Live::open(&s.address, &format!("/events?execution={id}"));
    live.until(&["execution.completed"]);
    let (_, snapshot) = s.get(&format!("/api/v1/executions/get?id={id}"));
    let report = &snapshot["output"];
    assert_eq!(
        report["index_is_current"], false,
        "the server is serving a picture that is no longer true, and says so"
    );
    let findings = report["findings"].as_array().expect("findings");
    assert!(
        findings.iter().any(|d| d["path"] == ".ai/repo/policy.yaml"),
        "{findings:?}"
    );
    assert!(
        snapshot["diagnostics"]
            .as_array()
            .is_some_and(|d| !d.is_empty()),
        "with a finding on the stream as well as in the answer"
    );
}

/// The plane runs registered capabilities and nothing else. There is no endpoint that
/// takes a command, and the one that takes a capability id refuses anything the registry
/// does not hold.
#[test]
fn nothing_here_runs_anything_the_registry_does_not_declare() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    for attempt in [
        json!({ "capability": "sh -c 'echo hi'" }),
        json!({ "capability": "../../bin/sh" }),
        json!({ "capability": "" }),
    ] {
        let (status, _, text) = s.request(
            "POST",
            "/api/v1/executions/start",
            Some(&attempt.to_string()),
        );
        assert!(status == 404 || status == 400, "{status} {text}");
    }
    // and no route takes a command
    let (_, doc) = s.get("/openapi.json");
    let paths = doc["paths"].as_object().unwrap();
    assert!(
        !paths
            .keys()
            .any(|p| p.contains("shell") || p.contains("exec/")),
        "{:?}",
        paths.keys().collect::<Vec<_>>()
    );
    let start_op = &paths["/api/v1/executions/start"]["post"];
    assert_eq!(start_op["operationId"], "executions.start");
    let properties = &start_op["requestBody"]["content"]["application/json"]["schema"];
    let name = properties["$ref"]
        .as_str()
        .and_then(|r| r.rsplit('/').next())
        .expect("the input is a named component");
    let schema = &doc["components"]["schemas"][name];
    assert_eq!(
        schema["properties"]
            .as_object()
            .map(|p| p.keys().cloned().collect::<Vec<_>>()),
        Some(vec!["capability".to_string(), "input".to_string()]),
        "the only things a client may send"
    );
}

/// A cached capability, run as an execution, runs. A cache makes one answer
/// indistinguishable from another, which is right for an answer and wrong for a record of
/// work: an execution answered from a cache would report no steps and claim to have
/// succeeded at something that never happened.
#[test]
fn an_execution_is_never_answered_from_the_cache() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    // warm it: health.report is one of the capabilities a measurement gave a cache
    let (status, _) = s.get("/api/v1/health");
    assert_eq!(status, 200);
    let (_, again) = s.get("/api/v1/health");
    assert!(again["checks"].is_array(), "a second read is answered");

    let started = start(&s, "health.report", json!({}));
    let id = started["id"].as_str().unwrap().to_string();
    let mut live = Live::open(&s.address, &format!("/events?execution={id}"));
    live.until(&["execution.completed"]);
    let (_, snapshot) = s.get(&format!("/api/v1/executions/get?id={id}"));
    assert_eq!(snapshot["state"], "succeeded");
    assert!(
        snapshot["steps"].as_array().is_some_and(|s| !s.is_empty()),
        "the handler ran and said what it was doing: {snapshot}"
    );
    assert!(
        snapshot["event_count"].as_u64().unwrap_or(0) > 4,
        "more than created, queued, started and completed"
    );
}

/// Several clients watching one execution all see it, and one that has stopped reading
/// does not slow it down or stop the others.
#[test]
fn several_clients_follow_one_execution_and_a_silent_one_holds_nobody_up() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let started = start(
        &s,
        "executions.demonstrate",
        json!({ "steps": 3, "delay_ms": 60 }),
    );
    let id = started["id"].as_str().unwrap().to_string();
    let target = format!("/events?execution={id}");

    let mut a = Live::open(&s.address, &target);
    let b = Live::open(&s.address, &target); // opened and never read from
    let all = Live::open(&s.address, "/events"); // every execution of this process

    let seen = a.until(&["execution.completed"]);
    assert!(types(&seen).contains(&"execution.completed".to_string()));
    let (_, snapshot) = s.get(&format!("/api/v1/executions/get?id={id}"));
    assert_eq!(snapshot["state"], "succeeded");
    assert_eq!(snapshot["output"]["steps"], 3);
    drop(b);
    let mut all = all;
    // the unscoped channel saw the same execution
    assert!(all
        .until(&["execution.completed"])
        .iter()
        .any(|f| f["execution_id"] == id.as_str()));
}
