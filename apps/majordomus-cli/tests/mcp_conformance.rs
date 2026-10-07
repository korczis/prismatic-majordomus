//! The MCP server held to the protocol and to its own boundaries, from the outside: a
//! built executable, real pipes, a disposable repository, and nothing asserted that the
//! registry could not have said first.
//!
//! `mcp_stdio.rs` proves the round trip and the classic error codes, `mcp_shared.rs` the
//! shared server and its lease, and `properties.rs` that direct, HTTP and MCP answers agree
//! in one process. What none of them asks, and this file does:
//!
//! - whether every protocol version the server names is the one it negotiates, and whether
//!   it advertises a capability exactly when the methods behind it answer;
//! - whether a request id of any JSON type comes back as it was sent, and whether a
//!   session survives every shape of malformed traffic;
//! - whether a tool refuses what its own advertised input schema refuses;
//! - whether the listings are the same bytes across calls, processes, concurrent sessions
//!   and the order in which the repository's files were written;
//! - whether a secret in the server's environment or in the repository's git configuration
//!   ever reaches a client, over every read tool and every listed resource;
//! - whether a path argument can reach a file outside the repository;
//! - where the server finds its repository when a client launches it somewhere else, and
//!   what it says when there is none;
//! - whether a downstream repository with its own layer, or with only the required part of
//!   one, is served as itself;
//! - whether the command line, HTTP and MCP — three real processes — answer one read alike.
//!
//! Every sweep is over the registry: a tool added tomorrow is in it without an edit here.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Fixture, BIN};
use majordomus_cli::capability::{
    BenchmarkPolicy, CapabilityKind, CaseContext, Effect, HttpMethod, WaiverReason,
};
use serde_json::{json, Value};

/// How long one session may take before the test calls it hung. Generous: these run on
/// machines with dozens of other builds, and a slow answer is not a defect — no answer is.
const SESSION_LIMIT: Duration = Duration::from_secs(600);

/// What one session produced: the exit code, the raw streams, and every stdout line parsed.
struct Exchange {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    frames: Vec<Value>,
}

impl Exchange {
    /// The frames that answer a request, by id. A batch answer is opened.
    fn by_id(&self) -> BTreeMap<u64, Value> {
        let mut out = BTreeMap::new();
        for f in &self.frames {
            let items = match f {
                Value::Array(items) => items.clone(),
                other => vec![other.clone()],
            };
            for item in items {
                if let Some(id) = item["id"].as_u64() {
                    out.insert(id, item);
                }
            }
        }
        out
    }

    /// The raw stdout line that answers request `id`: what a client's parser receives.
    fn raw(&self, id: u64) -> &str {
        self.stdout
            .lines()
            .find(|l| {
                serde_json::from_str::<Value>(l)
                    .map(|v| v["id"].as_u64() == Some(id))
                    .unwrap_or(false)
            })
            .unwrap_or_else(|| panic!("no frame answers id {id}:\n{}", self.stdout))
    }
}

/// Run `majordomus <args>` in `cwd`, write `input` to its stdin and close it, and collect
/// everything. Writing and reading run on their own threads, so a large session cannot
/// deadlock on a full pipe in either direction.
fn exchange_raw(
    f: &Fixture,
    cwd: &Path,
    args: &[&str],
    env: &[(&str, &str)],
    input: Vec<u8>,
) -> Exchange {
    let mut command = Command::new(BIN);
    command
        .args(args)
        .current_dir(cwd)
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env("MAJORDOMUS_LOG", "info")
        .env_remove("MAJORDOMUS_ROOT")
        // per-user state (the mesh's node identity) lives beside the fixture and is removed
        // with it: no test here reads or writes the developer's own
        .env("XDG_STATE_HOME", state_home(f))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        command.env(k, v);
    }
    let mut child = command.spawn().expect("spawn majordomus");
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        // a server that ends early closes its stdin; what was not written was not needed
        let _ = stdin.write_all(&input);
    });
    let mut out = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        out.read_to_string(&mut s).expect("stdout is UTF-8");
        s
    });
    let mut err = child.stderr.take().unwrap();
    let errors = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = err.read_to_end(&mut s);
        String::from_utf8_lossy(&s).into_owned()
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break status;
        }
        if started.elapsed() > SESSION_LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "`majordomus {}` did not end within {SESSION_LIMIT:?}",
                args.join(" ")
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    writer.join().unwrap();
    let stdout = reader.join().unwrap();
    let stderr = errors.join().unwrap();
    let frames = stdout
        .lines()
        .map(|l| {
            serde_json::from_str::<Value>(l)
                .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {l}\nstderr:\n{stderr}"))
        })
        .collect();
    Exchange {
        code: status.code(),
        stdout,
        stderr,
        frames,
    }
}

/// Where a child process of this fixture keeps per-user state.
fn state_home(f: &Fixture) -> String {
    f.parent().join("state").to_str().unwrap().to_string()
}

/// One session of requests, one JSON message per line.
fn exchange(
    f: &Fixture,
    cwd: &Path,
    args: &[&str],
    env: &[(&str, &str)],
    requests: &[Value],
) -> Exchange {
    let mut input = String::new();
    for r in requests {
        input.push_str(&r.to_string());
        input.push('\n');
    }
    exchange_raw(f, cwd, args, env, input.into_bytes())
}

/// A standalone stdio session in the fixture: no port, no lease, nothing written. The
/// shared-server path is `mcp_shared.rs`'s; every question here is about the protocol and
/// what the surface answers, which is the same surface either way.
fn standalone(f: &Fixture, requests: &[Value]) -> Exchange {
    standalone_with(f, &[], requests)
}

fn standalone_with(f: &Fixture, env: &[(&str, &str)], requests: &[Value]) -> Exchange {
    let root = f.root();
    let repo = root.to_str().unwrap();
    exchange(
        f,
        &root,
        &["mcp", "--standalone", "--repo", repo],
        env,
        requests,
    )
}

fn initialize(id: u64, version: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": "initialize", "params": {
        "protocolVersion": version, "capabilities": {},
        "clientInfo": { "name": "conformance", "version": "0" } } })
}

fn initialized() -> Value {
    json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
}

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn call(id: u64, tool: &str, arguments: Value) -> Value {
    request(
        id,
        "tools/call",
        json!({ "name": tool, "arguments": arguments }),
    )
}

/// The opening every well-behaved client sends.
fn opening() -> Vec<Value> {
    vec![initialize(1, "2025-06-18"), initialized()]
}

/// One executable the registry exposes as an MCP tool, with the representative inputs its
/// input type declares for this repository.
struct ToolCase {
    id: String,
    tool: String,
    cases: Vec<(&'static str, Value)>,
}

/// Every tool that only reads, with its benchmark cases computed against `f`'s own index:
/// the inputs the capability itself says are representative, so nothing here invents an
/// argument. A capability waived as an external dependency or as transient state is left
/// out, because its honest answer depends on something this fixture cannot hold.
fn read_tools(f: &Fixture) -> Vec<ToolCase> {
    let app = common::load_app(f);
    let ctx = &app.context;
    let case_ctx = CaseContext::of(ctx);
    ctx.registry
        .iter()
        .filter(|c| c.execution.effect == Effect::Read)
        .filter(|c| c.kind == CapabilityKind::Query)
        .filter(|c| {
            !matches!(
                c.benchmark,
                BenchmarkPolicy::Waived {
                    reason: WaiverReason::ExternalDependency | WaiverReason::TransientState
                }
            )
        })
        .filter_map(|c| {
            let tool = c.exposure.mcp.as_ref()?.tool.clone()?;
            let cases = ctx
                .registry
                .cases(c.id.as_str())
                .map(|p| {
                    p(&case_ctx)
                        .into_iter()
                        .map(|n| (n.name, n.input))
                        .collect()
                })
                .unwrap_or_default();
            Some(ToolCase {
                id: c.id.to_string(),
                tool,
                cases,
            })
        })
        .collect()
}

// ------------------------------------------------------------------------- the handshake

/// The published MCP revisions, newest first, plus one that never existed. They are the
/// protocol's facts, not the server's: which of them the server speaks is what the test
/// finds out, so a revision the server adopts or retires moves no line here.
const REVISIONS: &[&str] = &[
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
    "1999-01-01",
];

/// Version negotiation, as the specification states it: asked for a revision it speaks,
/// the server answers with that revision; asked for any other — or for none — it answers
/// with one it does speak, and always the same one. `serverInfo.version` is the build's own
/// version, never a second constant.
#[test]
fn initialize_answers_a_version_it_speaks_and_one_fallback_otherwise() {
    let f = Fixture::new();
    let mut requests = Vec::new();
    // one session per initialize would be truer to a client, and five times the cost; the
    // negotiation is per request, so one session asking several times proves the same thing
    for (i, v) in REVISIONS.iter().enumerate() {
        requests.push(initialize(i as u64 + 1, v));
    }
    let absent = REVISIONS.len() as u64 + 1;
    requests.push(
        json!({ "jsonrpc": "2.0", "id": absent, "method": "initialize",
        "params": { "capabilities": {}, "clientInfo": { "name": "no-version", "version": "0" } } }),
    );
    let x = standalone(&f, &requests);
    assert_eq!(x.code, Some(0), "{}", x.stderr);
    let r = x.by_id();
    let answered = |id: u64| {
        r[&id]["result"]["protocolVersion"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let spoken: BTreeSet<String> = REVISIONS
        .iter()
        .enumerate()
        .filter(|(i, v)| answered(*i as u64 + 1) == **v)
        .map(|(_, v)| v.to_string())
        .collect();
    assert!(
        !spoken.is_empty(),
        "the server speaks no published revision of the protocol"
    );
    let fallback = answered(absent);
    assert!(
        spoken.contains(&fallback),
        "asked for no version, the server answers {fallback}, which it does not echo when asked"
    );
    for (i, v) in REVISIONS.iter().enumerate() {
        let id = i as u64 + 1;
        let result = &r[&id]["result"];
        if !spoken.contains(*v) {
            assert_eq!(
                answered(id),
                fallback,
                "asked for {v}, which it does not speak, the server answers neither it nor its fallback"
            );
        }
        assert_eq!(result["serverInfo"]["version"], majordomus_cli::VERSION);
        assert_eq!(result["serverInfo"]["name"], "majordomus");
    }
    assert!(
        !spoken.contains("1999-01-01"),
        "a revision that never existed was echoed"
    );
}

/// A capability object is a promise: `tools` in `initialize` means `tools/list` answers,
/// and an absent `prompts` means `prompts/list` is a method the server does not have. The
/// two must agree in both directions, whichever capabilities the server grows.
#[test]
fn a_capability_is_advertised_exactly_when_its_methods_answer() {
    let f = Fixture::new();
    let families: [(&str, &[&str]); 3] = [
        ("tools", &["tools/list"]),
        ("resources", &["resources/list", "resources/templates/list"]),
        ("prompts", &["prompts/list"]),
    ];
    let mut requests = opening();
    let mut id = 10;
    for (_, methods) in &families {
        for m in *methods {
            requests.push(request(id, m, json!({})));
            id += 1;
        }
    }
    let x = standalone(&f, &requests);
    let r = x.by_id();
    let advertised = &r[&1]["result"]["capabilities"];
    let mut id = 10;
    for (family, methods) in &families {
        let promised = advertised.get(*family).is_some_and(Value::is_object);
        for m in *methods {
            let answer = &r[&id];
            if promised {
                assert!(
                    answer.get("result").is_some(),
                    "{family} is advertised and {m} does not answer: {answer}"
                );
            } else {
                assert_eq!(
                    answer["error"]["code"], -32601,
                    "{family} is not advertised and {m} answers as if it were: {answer}"
                );
            }
            id += 1;
        }
    }
}

// ------------------------------------------------------------------------- the envelope

/// JSON-RPC lets a request id be a string or a number, and the response carries it back
/// unchanged: a client correlating by `"7"` must not receive `7`, and a client that uses
/// UUIDs must get its UUID.
#[test]
fn a_request_id_comes_back_exactly_as_it_was_sent() {
    let f = Fixture::new();
    let ids = [
        json!("a-string-id"),
        json!("7"),
        json!("id — ✓ ∞ 🐎"),
        json!(0),
        json!(-1),
        json!(9_007_199_254_740_991_u64),
        json!(""),
    ];
    let mut requests = opening();
    for id in &ids {
        requests.push(json!({ "jsonrpc": "2.0", "id": id, "method": "ping" }));
    }
    let x = standalone(&f, &requests);
    assert_eq!(x.code, Some(0), "{}", x.stderr);
    // the first frame answers initialize; the rest answer the pings, in order
    let answers: Vec<&Value> = x.frames.iter().skip(1).collect();
    assert_eq!(answers.len(), ids.len(), "{}", x.stdout);
    for (sent, got) in ids.iter().zip(answers) {
        assert_eq!(&got["id"], sent, "the id came back changed: {got}");
        assert_eq!(got["result"], json!({}), "{got}");
    }
}

/// Every malformed shape a client or a broken pipe can produce gets the error JSON-RPC
/// names for it, nothing else on stdout, and the session goes on answering afterwards.
/// The shapes `mcp_stdio.rs` already sends are not repeated here.
#[test]
fn malformed_traffic_is_answered_and_the_session_survives() {
    let f = Fixture::new();
    let mut input = String::new();
    for r in opening() {
        input.push_str(&format!("{r}\n"));
    }
    let lines: Vec<(Option<i64>, String)> = vec![
        // not an object
        (Some(-32600), "42".into()),
        (Some(-32600), "\"a string\"".into()),
        (Some(-32600), "null".into()),
        // an empty batch
        (Some(-32600), "[]".into()),
        // a frame with an id and no method
        (Some(-32600), json!({ "jsonrpc": "2.0", "id": 20 }).to_string()),
        // arguments that are not an object
        (Some(-32602), call(21, "majordomus_list", json!([])).to_string()),
        // a tool name that is not a string
        (
            Some(-32602),
            request(22, "tools/call", json!({ "name": 7 })).to_string(),
        ),
        // a URI that is not a string
        (
            Some(-32602),
            request(23, "resources/read", json!({ "uri": 7 })).to_string(),
        ),
        // truncated JSON
        (Some(-32700), "{\"jsonrpc\":\"2.0\",\"id\":24,\"method\":".into()),
        // notifications: an unknown one and a batch of them are answered with nothing
        (None, json!({ "jsonrpc": "2.0", "method": "notifications/no-such-thing" }).to_string()),
        (
            None,
            json!([{ "jsonrpc": "2.0", "method": "notifications/progress" },
                   { "jsonrpc": "2.0", "method": "notifications/cancelled", "params": { "requestId": 1 } }])
            .to_string(),
        ),
    ];
    for (_, l) in &lines {
        input.push_str(l);
        input.push('\n');
    }
    let mut bytes = input.into_bytes();
    // a line that is not UTF-8 at all
    bytes.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":25,\"method\":\"\xff\xfe\"}\n");
    // ... and after all of it, the session still answers
    bytes.extend_from_slice(format!("{}\n", request(99, "ping", json!({}))).as_bytes());
    let root = f.root();
    let x = exchange_raw(
        &f,
        &root,
        &["mcp", "--standalone", "--repo", root.to_str().unwrap()],
        &[],
        bytes,
    );
    assert_eq!(x.code, Some(0), "{}", x.stderr);
    let expected: Vec<i64> = lines
        .iter()
        .filter_map(|(c, _)| *c)
        .chain([-32700])
        .collect();
    // frame 0 answers initialize; the last answers the closing ping
    let errors: Vec<&Value> = x.frames[1..x.frames.len() - 1].iter().collect();
    assert_eq!(
        errors.len(),
        expected.len(),
        "one frame per malformed request, none per notification:\n{}",
        x.stdout
    );
    for (want, got) in expected.iter().zip(&errors) {
        assert_eq!(got["error"]["code"], *want, "{got}");
        assert!(
            got["error"]["message"]
                .as_str()
                .is_some_and(|m| !m.trim().is_empty()),
            "an error says what was wrong: {got}"
        );
    }
    let last = x.frames.last().unwrap();
    assert_eq!(
        last["id"], 99,
        "the session answered after the malformed traffic: {last}"
    );
    assert_eq!(last["result"], json!({}));
}

/// A tool's `inputSchema` is what a client builds its arguments from, so it is also what
/// the tool must refuse: a call without a property the schema requires, or with one of the
/// wrong type, is a refusal the client can read (`isError` and a reason) — never a
/// protocol error, never a silent default, never a crash. Every tool with a required
/// property is asked, whatever it does, because a refusal of its input runs no handler.
#[test]
fn every_tool_refuses_a_call_its_input_schema_refuses() {
    let f = Fixture::new();
    let mut requests = opening();
    requests.push(request(2, "tools/list", json!({})));
    let listing = standalone(&f, &requests);
    let tools = listing.by_id()[&2]["result"]["tools"]
        .as_array()
        .expect("tools")
        .clone();
    let mut requests = opening();
    let mut asked = Vec::new();
    let mut id = 10;
    for t in &tools {
        let schema = &t["inputSchema"];
        let Some(required) = schema["required"].as_array() else {
            continue;
        };
        for name in required.iter().filter_map(Value::as_str) {
            let declared = &schema["properties"][name]["type"];
            // the wrong type for whatever the property declares
            let wrong = match declared.as_str() {
                Some("string") => json!(12345),
                Some("integer" | "number") => json!("not a number"),
                Some("boolean") => json!("not a boolean"),
                Some("array") => json!("not an array"),
                Some("object") => json!("not an object"),
                _ => continue,
            };
            requests.push(call(id, t["name"].as_str().unwrap(), json!({})));
            asked.push((id, t["name"].clone(), name.to_string(), "missing"));
            id += 1;
            requests.push(call(
                id,
                t["name"].as_str().unwrap(),
                json!({ name: wrong }),
            ));
            asked.push((id, t["name"].clone(), name.to_string(), "wrong type"));
            id += 1;
        }
    }
    assert!(
        asked.len() >= 10,
        "the registry exposes tools with required properties, or this proves nothing"
    );
    let x = standalone(&f, &requests);
    assert_eq!(x.code, Some(0), "{}", x.stderr);
    let r = x.by_id();
    let mut wrong = Vec::new();
    for (id, tool, property, how) in &asked {
        let answer = &r[id];
        let refused = answer["result"]["isError"] == true
            && answer["result"]["content"][0]["text"]
                .as_str()
                .is_some_and(|t| !t.trim().is_empty());
        if !refused {
            wrong.push(format!("{tool} with `{property}` {how}: {answer}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} call(s) the advertised input schema refuses were not refused as a result:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

/// A stdio session is a sequence: a thousand requests sent without waiting are a thousand
/// answers in the order they were asked, none lost and none merged.
#[test]
fn a_long_pipelined_session_answers_every_request_in_order() {
    let f = Fixture::new();
    let mut requests = opening();
    for i in 0..1000u64 {
        let id = 100 + i;
        requests.push(match i % 4 {
            0 => request(id, "ping", json!({})),
            1 => request(id, "tools/list", json!({})),
            2 => request(
                id,
                "resources/read",
                json!({ "uri": "majordomus://repository" }),
            ),
            _ => call(id, "majordomus_list", json!({ "kind": "rule" })),
        });
    }
    let x = standalone(&f, &requests);
    assert_eq!(x.code, Some(0), "{}", x.stderr);
    let ids: Vec<u64> = x.frames.iter().filter_map(|f| f["id"].as_u64()).collect();
    let expected: Vec<u64> = std::iter::once(1).chain(100..1100).collect();
    assert_eq!(
        ids, expected,
        "answers are one per request, in request order"
    );
    assert!(
        x.frames.iter().all(|f| f.get("error").is_none()),
        "no request failed"
    );
}

// ------------------------------------------------------------------------- determinism

/// The listings a client caches are the same bytes every time it asks: twice in one
/// session, in a second process, and in a repository whose files were written in the
/// opposite order. Nothing in them may depend on the filesystem, a hash map or a clock.
#[test]
fn listings_are_identical_across_calls_processes_and_source_order() {
    fn layered(order: &[usize]) -> Fixture {
        let f = Fixture::new();
        for &i in order {
            f.write(
                &format!(".ai/repo/rules/project/extra{i}.v1.md"),
                &common::rule(&format!("project.extra{i}"), 1, &format!("Extra {i}")),
            );
        }
        f.commit("extra rules");
        f
    }
    let listings = |f: &Fixture| {
        let mut requests = opening();
        requests.push(request(2, "tools/list", json!({})));
        requests.push(request(3, "resources/list", json!({})));
        requests.push(request(4, "tools/list", json!({})));
        requests.push(request(5, "resources/list", json!({})));
        let x = standalone(f, &requests);
        assert_eq!(x.code, Some(0), "{}", x.stderr);
        let strip = |id: u64| {
            let mut v: Value = serde_json::from_str(x.raw(id)).unwrap();
            v["id"] = Value::Null;
            serde_json::to_string(&v).unwrap()
        };
        assert_eq!(strip(2), strip(4), "tools/list changed between two calls");
        assert_eq!(
            strip(3),
            strip(5),
            "resources/list changed between two calls"
        );
        (strip(2), strip(3))
    };
    let forward = layered(&(0..12).collect::<Vec<_>>());
    let backward = layered(&(0..12).rev().collect::<Vec<_>>());
    let shuffled = layered(&[5, 11, 0, 7, 3, 9, 1, 10, 4, 8, 2, 6]);
    let a = listings(&forward);
    let b = listings(&forward);
    assert_eq!(a, b, "a second process listed different bytes");
    assert_eq!(
        a,
        listings(&backward),
        "the listing follows the order files were written"
    );
    assert_eq!(
        a,
        listings(&shuffled),
        "the listing follows the order files were written"
    );
    assert!(
        !a.0.contains(forward.root().to_str().unwrap()),
        "tools/list names the machine path of the checkout"
    );
}

/// Concurrent clients of one repository see one answer: eight sessions started together,
/// each listing and reading, produce the same bytes as each other.
#[test]
fn concurrent_sessions_answer_byte_for_byte_alike() {
    let f = std::sync::Arc::new(Fixture::new());
    let sessions: Vec<_> = (0..8)
        .map(|_| {
            let f = std::sync::Arc::clone(&f);
            std::thread::spawn(move || {
                let mut requests = opening();
                requests.push(request(2, "tools/list", json!({})));
                requests.push(request(3, "resources/list", json!({})));
                requests.push(request(
                    4,
                    "resources/read",
                    json!({ "uri": "majordomus://rule/project.alpha@1" }),
                ));
                requests.push(call(
                    5,
                    "majordomus_search",
                    json!({ "query": "normative" }),
                ));
                let x = standalone(&f, &requests);
                assert_eq!(x.code, Some(0), "{}", x.stderr);
                (2..=5).map(|id| x.raw(id).to_string()).collect::<Vec<_>>()
            })
        })
        .collect();
    let answers: Vec<Vec<String>> = sessions.into_iter().map(|s| s.join().unwrap()).collect();
    for (i, a) in answers.iter().enumerate().skip(1) {
        assert_eq!(
            a, &answers[0],
            "session {i} answered differently from session 0"
        );
    }
}

// ------------------------------------------------------------------------- secrets

/// Values shaped like the credentials a developer's shell and a CI runner really hold,
/// plus one with no recognisable shape under a name that says what it is. Each is unique
/// to this file, so finding it anywhere in the output can only be a leak.
const SECRETS: &[(&str, &str)] = &[
    (
        "GITHUB_TOKEN",
        "ghp_mjconformanceA1b2C3d4E5f6G7h8I9j0K1l2M3n4",
    ),
    ("GH_TOKEN", "gho_mjconformanceZ9y8X7w6V5u4T3s2R1q0P9o8N7m6"),
    (
        "ANTHROPIC_API_KEY",
        "sk-ant-api03-mjconformance-Qw3rTy7UiOp9AsDfGhJk",
    ),
    (
        "OPENAI_API_KEY",
        "sk-proj-mjconformanceLmNoPqRsTuVwXyZ0123456789",
    ),
    (
        "AWS_SECRET_ACCESS_KEY",
        "mjconformanceWJalrXUtnFEMIK7MDENGbPxRfiCY",
    ),
    ("MAJORDOMUS_MESH_TOKEN", "mjconformance-mesh-secret-4b9e1f"),
    (
        "DATABASE_URL",
        "postgres://admin:mjconformance-dbpass@db.internal:5432/x",
    ),
    (
        "MJ_UNSHAPED_SECRET",
        "mjconformance-plain-value-no-shape-0451",
    ),
];

/// The marker every planted secret shares: what the output is searched for.
const MARK: &str = "mjconformance";

/// Everything the server lists in this repository, read, and every read tool, asked with
/// the inputs its own capability declares: the listing session, the sweep session, and
/// what each request of the sweep asked for.
fn sweep(f: &Fixture) -> (Exchange, Exchange, BTreeMap<u64, String>) {
    // what the server lists, first, in this repository
    let mut requests = opening();
    requests.push(request(2, "tools/list", json!({})));
    requests.push(request(3, "resources/list", json!({})));
    let listing = standalone_with(f, SECRETS, &requests);
    let resources: Vec<String> = listing.by_id()[&3]["result"]["resources"]
        .as_array()
        .expect("resources")
        .iter()
        .map(|r| r["uri"].as_str().unwrap().to_string())
        .collect();

    // then everything it lists, read, and every read tool, asked
    let mut requests = opening();
    let mut asked: BTreeMap<u64, String> = BTreeMap::new();
    let mut id = 1000;
    for uri in &resources {
        requests.push(request(id, "resources/read", json!({ "uri": uri })));
        asked.insert(id, format!("resources/read {uri}"));
        id += 1;
    }
    let tools = read_tools(f);
    assert!(
        tools.len() >= 20,
        "the sweep asks the registry's read tools, or it proves nothing"
    );
    for t in &tools {
        let cases = if t.cases.is_empty() {
            vec![("empty", json!({}))]
        } else {
            t.cases.clone()
        };
        for (name, input) in cases {
            requests.push(call(id, &t.tool, input));
            asked.insert(id, format!("{} ({}) case {name}", t.tool, t.id));
            id += 1;
        }
    }
    let x = standalone_with(f, SECRETS, &requests);

    (listing, x, asked)
}

/// A secret in the server's environment, in a remote URL and in an HTTP header of the
/// repository's git configuration, and in an untracked `.env`, never reaches a client — not
/// through `initialize`, not through a listing, not through any listed resource, and not
/// through any read tool asked with the inputs its own capability declares. stderr is held
/// to the same standard: it is the log an operator pastes into an issue.
///
/// The same sweep also holds the surface to its listing: a resource it lists is one it can
/// read, and a read tool asked with its own representative input does not fail as a
/// protocol error.
#[test]
fn no_secret_in_the_environment_or_the_repository_reaches_a_client() {
    let f = Fixture::new();
    f.git(&[
        "remote",
        "add",
        "origin",
        "https://x-access-token:ghs_mjconformanceRemoteUrlToken0001@github.com/o/r.git",
    ]);
    f.git(&[
        "config",
        "http.https://github.com/.extraheader",
        "AUTHORIZATION: basic mjconformanceExtraHeaderSecret",
    ]);
    f.write(".env", "API_TOKEN=mjconformance-dotenv-secret\n");

    let (listing, x, asked) = sweep(&f);
    assert_eq!(x.code, Some(0), "{}", x.stderr);

    for (stream, text) in [
        ("listing stdout", &listing.stdout),
        ("listing stderr", &listing.stderr),
        ("sweep stdout", &x.stdout),
        ("sweep stderr", &x.stderr),
    ] {
        let leaks = around(text, MARK);
        assert!(
            leaks.is_empty(),
            "a planted secret reached the client through {stream}, {} time(s):\n{}",
            leaks.len(),
            leaks.join("\n")
        );
    }

    let r = x.by_id();
    let mut failures = Vec::new();
    for (id, what) in &asked {
        match r.get(id) {
            None => failures.push(format!("{what}: no answer")),
            Some(a) if what.starts_with("resources/read") && a.get("error").is_some() => {
                failures.push(format!("{what}: listed but unreadable: {}", a["error"]))
            }
            Some(a) if a.get("error").is_some() => {
                failures.push(format!("{what}: a protocol error: {}", a["error"]))
            }
            Some(_) => {}
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} request(s) did not answer as the listing promised:\n{}",
        failures.len(),
        asked.len(),
        failures.join("\n")
    );
}

/// The shell tool, as a repository runs it from this checkout.
fn shell(f: &Fixture, args: &[&str], stdin: &str) -> (Option<i32>, String, String) {
    let tool = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin/majordomus");
    let mut child = Command::new(tool)
        .args(args)
        .current_dir(f.root())
        .env_remove("MAJORDOMUS_ROOT")
        .env_remove("MAJORDOMUS_SHARE")
        .env("XDG_STATE_HOME", state_home(f))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the shell tool");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .expect("stdin");
    let out = child.wait_with_output().expect("wait");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Every file under `dir`, `.git` excepted, whose bytes contain `mark`.
fn files_holding(dir: &Path, mark: &str, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read_dir").flatten() {
        let path = entry.path();
        if path.file_name().is_some_and(|n| n == ".git") {
            continue;
        }
        if path.is_dir() {
            files_holding(&path, mark, found);
        } else if let Ok(bytes) = std::fs::read(&path) {
            if String::from_utf8_lossy(&bytes).contains(mark) {
                found.push(path.display().to_string());
            }
        }
    }
}

/// The rule is not "drop whatever stands before the at sign". Over ssh the login is how
/// the remote is reached and is no secret, so `ssh://git:<password>@host` is served as
/// `ssh://git@host`: the password goes and the identity still says which repository this
/// is. And the server is not the only writer: the shell tool writes the same identity into
/// a shared record, which is tracked and pushed, so the record it writes under that remote
/// holds no password either, and neither does what the server answers when it reads the
/// record back.
#[test]
fn a_remote_keeps_its_login_and_no_shared_record_holds_its_password() {
    const SERVED: &str = "ssh://git@host.invalid/o/r.git";
    let f = Fixture::empty_git();
    for args in [&["init"][..], &["update"][..]] {
        let (code, _, err) = shell(&f, args, "");
        assert_eq!(code, Some(0), "majordomus {args:?}: {err}");
    }
    f.git(&["add", "-A"]);
    f.commit("the layer");
    f.git(&[
        "remote",
        "add",
        "origin",
        "ssh://git:mjconformanceSshPassword0002@host.invalid/o/r.git",
    ]);

    // the shell writes a shared record under that remote
    let (code, _, err) = shell(&f, &["session", "start", "--worker", "conformance/ssh"], "");
    assert_eq!(code, Some(0), "session start: {err}");
    let (code, out, err) = shell(
        &f,
        &["session", "close"],
        "The remote carried a password and the record does not.\n",
    );
    assert_eq!(code, Some(0), "session close: {err}");
    let record = f.root().join(out.lines().last().unwrap_or_default().trim());
    assert!(
        record.starts_with(f.root().join(".ai/repo/sessions")) && record.is_file(),
        "session close named no shared record: {out}"
    );
    let written = std::fs::read_to_string(&record).expect("the record");
    assert!(
        written.contains(SERVED),
        "the shared record does not name the repository by its login-only URL:\n{written}"
    );
    let mut holding = Vec::new();
    files_holding(&f.root(), MARK, &mut holding);
    assert!(
        holding.is_empty(),
        "the remote's password was written into: {}",
        holding.join(", ")
    );

    // and the server, reading that repository and that record, answers the same
    let (listing, x, _) = sweep(&f);
    assert_eq!(x.code, Some(0), "{}", x.stderr);
    for (stream, text) in [
        ("listing stdout", &listing.stdout),
        ("listing stderr", &listing.stderr),
        ("sweep stdout", &x.stdout),
        ("sweep stderr", &x.stderr),
    ] {
        let leaks = around(text, MARK);
        assert!(
            leaks.is_empty(),
            "the remote's password reached the client through {stream}, {} time(s):\n{}",
            leaks.len(),
            leaks.join("\n")
        );
    }
    assert!(
        x.stdout.contains(SERVED),
        "no answer of the sweep names the remote as {SERVED}: the login was dropped with the password, or the identity is not served"
    );
}

// ------------------------------------------------------------------------- paths

/// A file outside the repository holds a marker. A symlink committed inside the repository
/// points at the directory that holds it. Every read tool whose input has a path-like
/// property is asked for it — relative escapes, an absolute path, through the symlink — and
/// so is `resources/read`. Whatever each answers, the marker is in none of it: a path
/// argument names something inside the repository or is refused.
#[test]
fn path_arguments_never_reach_outside_the_repository() {
    const OUTSIDE: &str = "mjconformance-outside-the-repository-7f3a";
    let f = Fixture::new();
    let outside = f.parent().join("outside.txt");
    std::fs::write(&outside, format!("{OUTSIDE}\n")).unwrap();
    std::os::unix::fs::symlink(f.parent(), f.path("docs/escape")).unwrap();
    // the control: a file inside the repository with a marker of its own, asked for through
    // the same requests. If nothing can return it, "nothing returned the outside file"
    // would be true of a sweep that cannot see file content at all.
    const INSIDE: &str = "mjconformance-inside-the-repository-2c91";
    f.write("docs/inside.md", &format!("# Inside\n\n{INSIDE}\n"));
    f.commit("a symlink out of the repository");
    let absolute = outside.to_str().unwrap().to_string();
    let payloads = [
        "../outside.txt".to_string(),
        "docs/../../outside.txt".to_string(),
        "./../outside.txt".to_string(),
        absolute.clone(),
        "docs/escape/outside.txt".to_string(),
        "docs/escape".to_string(),
        "docs/inside.md".to_string(),
    ];
    const PATHLIKE: &[&str] = &["path", "file", "paths", "dir", "directory", "work_dir"];

    let app = common::load_app(&f);
    let ctx = &app.context;
    let case_ctx = CaseContext::of(ctx);
    let mut requests = opening();
    let mut asked = BTreeMap::new();
    let mut id = 10;
    let mut tools_asked = BTreeSet::new();
    for c in ctx.registry.iter() {
        if c.execution.effect != Effect::Read {
            continue;
        }
        let Some(tool) = c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()) else {
            continue;
        };
        let (properties, _) = c.input.properties();
        let base = ctx
            .registry
            .cases(c.id.as_str())
            .and_then(|p| p(&case_ctx).into_iter().next())
            .map(|n| n.input)
            .unwrap_or_else(|| json!({}));
        for (name, _) in properties
            .iter()
            .filter(|(n, _)| PATHLIKE.contains(&n.as_str()))
        {
            for payload in &payloads {
                let mut args = if base.is_object() {
                    base.clone()
                } else {
                    json!({})
                };
                args[name.as_str()] = json!(payload);
                requests.push(call(id, &tool, args));
                asked.insert(id, format!("{tool}.{name} = {payload}"));
                tools_asked.insert(tool.clone());
                id += 1;
            }
        }
    }
    for uri in [
        "majordomus://document/docs/inside.md",
        "majordomus://document/../outside.txt",
        "majordomus://document/docs/escape/outside.txt",
        &format!("majordomus://document/{absolute}"),
        &format!("majordomus://document/{}", absolute.trim_start_matches('/')),
    ] {
        requests.push(request(id, "resources/read", json!({ "uri": uri })));
        asked.insert(id, format!("resources/read {uri}"));
        id += 1;
    }
    assert!(
        tools_asked.len() >= 3,
        "the registry exposes read tools that take a path, or this proves nothing: {tools_asked:?}"
    );
    let x = standalone(&f, &requests);
    assert_eq!(x.code, Some(0), "{}", x.stderr);
    let r = x.by_id();
    let reached: Vec<String> = asked
        .iter()
        .filter(|(id, _)| r.get(id).is_some_and(|a| a.to_string().contains(OUTSIDE)))
        .map(|(id, what)| {
            format!(
                "{what}: {}",
                r[id].to_string().chars().take(300).collect::<String>()
            )
        })
        .collect();
    assert!(
        reached.is_empty(),
        "a path argument read a file outside the repository:\n{}",
        reached.join("\n")
    );
    assert!(
        !x.stderr.contains(OUTSIDE),
        "the outside file's content reached the log"
    );
    let seen: Vec<&String> = asked
        .iter()
        .filter(|(id, _)| r.get(id).is_some_and(|a| a.to_string().contains(INSIDE)))
        .map(|(_, what)| what)
        .collect();
    assert!(
        !seen.is_empty(),
        "no request returned the content of a file inside the repository, so this sweep \
         cannot see file content and proves nothing about the one outside"
    );
    assert_eq!(asked.len(), r.len() - 1, "every request was answered");
}

// ------------------------------------------------------------------------- where it runs

/// A client launches the server from wherever its own process happens to be: the
/// repository root, a directory deep inside it, or somewhere else entirely with `--repo`.
/// Each of them serves the same repository, and says which one it is.
#[test]
fn the_server_finds_its_repository_from_a_subdirectory_and_from_elsewhere() {
    let f = Fixture::new();
    std::fs::create_dir_all(f.path("docs/deep/er")).unwrap();
    let root = f.root();
    let mut requests = opening();
    requests.push(call(2, "majordomus_repository", json!({})));
    requests.push(request(3, "tools/list", json!({})));
    let from_root = exchange(&f, &root, &["mcp", "--standalone"], &[], &requests);
    let from_deep = exchange(
        &f,
        &f.path("docs/deep/er"),
        &["mcp", "--standalone"],
        &[],
        &requests,
    );
    let from_elsewhere = exchange(
        &f,
        &f.parent(),
        &["mcp", "--standalone", "--repo", root.to_str().unwrap()],
        &[],
        &requests,
    );
    for (where_, x) in [
        ("root", &from_root),
        ("subdirectory", &from_deep),
        ("--repo", &from_elsewhere),
    ] {
        assert_eq!(x.code, Some(0), "launched from the {where_}: {}", x.stderr);
        let r = x.by_id();
        let repo = &r[&2]["result"]["structuredContent"];
        assert_eq!(
            repo["repository"]["root"],
            root.to_str().unwrap(),
            "launched from the {where_}, the server serves another root: {repo}"
        );
        assert_eq!(
            x.raw(3),
            from_root.raw(3),
            "launched from the {where_}, the tool listing differs"
        );
    }
}

/// Outside any repository there is nothing to serve, and a client is better told so at
/// once than handed an empty server: the process exits non-zero, stdout carries no frame a
/// client could mistake for a session, and stderr says what was looked for. The same holds
/// for a git repository that has no `.ai/` layer at all.
#[test]
fn outside_a_repository_the_server_refuses_and_says_why() {
    for (what, f) in [
        ("a plain directory", Fixture::plain_dir()),
        ("a bare git repository", Fixture::empty_git()),
    ] {
        let mut requests = opening();
        requests.push(request(2, "tools/list", json!({})));
        for args in [vec!["mcp", "--standalone"], vec!["mcp"]] {
            let x = exchange(&f, &f.root(), &args, &[], &requests);
            assert_ne!(
                x.code,
                Some(0),
                "{what}, `{}` served: {}",
                args.join(" "),
                x.stdout
            );
            assert!(
                x.frames.iter().all(|fr| fr.get("result").is_none()),
                "{what}: a refusing server answered a request: {}",
                x.stdout
            );
            assert!(
                x.stderr.to_lowercase().contains("repository") || x.stderr.contains(".ai"),
                "{what}: stderr does not say what was missing:\n{}",
                x.stderr
            );
        }
    }
}

// ------------------------------------------------------------------------- downstream

/// A downstream repository is served as itself: its own rules are resources and answers,
/// and what it lacks is absent rather than broken. The project here carries only what the
/// sources declare required, plus one rule of its own; every optional section of the
/// fixture — plan, why, features, deployments, knowledge notes, claims — is removed.
///
/// What is asserted is a relation, not a count: everything listed reads, nothing listed is
/// the Majordomus source repository's, the tool set is the one `mcp --inspect` reports for
/// this repository, and the downstream rule is the only rule it serves.
#[test]
fn a_downstream_repository_with_its_own_minimal_layer_is_served_as_itself() {
    let f = Fixture::new();
    for optional in [
        ".ai/repo/project",
        ".ai/repo/why",
        ".ai/repo/features",
        ".ai/repo/deployments",
        ".ai/repo/knowledge/curated",
        ".ai/repo/rules/project",
        "docs/claims",
        "docs/CLAIMS.yaml",
        "lib",
        "test",
    ] {
        f.git(&["rm", "-r", "-q", optional]);
    }
    f.write(
        ".ai/repo/rules/downstream/house-style.v1.md",
        &common::rule("downstream.house-style", 1, "House style"),
    );
    f.commit("a downstream layer: required parts and one rule of its own");

    let mut requests = opening();
    requests.push(request(2, "tools/list", json!({})));
    requests.push(request(3, "resources/list", json!({})));
    requests.push(call(4, "majordomus_list", json!({ "kind": "rule" })));
    requests.push(call(5, "majordomus_repository", json!({})));
    let x = standalone(&f, &requests);
    assert_eq!(x.code, Some(0), "{}", x.stderr);
    let r = x.by_id();

    let rules = &r[&4]["result"]["structuredContent"];
    let uris: Vec<&str> = rules["objects"]
        .as_array()
        .expect("objects")
        .iter()
        .map(|o| o["uri"].as_str().unwrap())
        .collect();
    assert_eq!(
        uris,
        ["majordomus://rule/downstream.house-style@1"],
        "the downstream repository serves its own rule and only its own"
    );
    let repo = &r[&5]["result"]["structuredContent"];
    assert_eq!(repo["repository"]["root"], f.root().to_str().unwrap());

    let resources: Vec<String> = r[&3]["result"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["uri"].as_str().unwrap().to_string())
        .collect();
    let mut reads = opening();
    for (i, uri) in resources.iter().enumerate() {
        reads.push(request(
            100 + i as u64,
            "resources/read",
            json!({ "uri": uri }),
        ));
    }
    let read = standalone(&f, &reads);
    let answered = read.by_id();
    let unreadable: Vec<String> = resources
        .iter()
        .enumerate()
        .filter(|(i, _)| answered[&(100 + *i as u64)].get("error").is_some())
        .map(|(i, uri)| format!("{uri}: {}", answered[&(100 + i as u64)]["error"]))
        .collect();
    assert!(
        unreadable.is_empty(),
        "listed and unreadable:\n{}",
        unreadable.join("\n")
    );
    let foreign: Vec<&String> = resources
        .iter()
        .filter(|u| u.contains("project.alpha") || u.contains("fixture-"))
        .collect();
    assert!(
        foreign.is_empty(),
        "removed objects are still served: {foreign:?}"
    );

    // the tool set is the repository's, as the inspection of the same repository reports it
    let (code, inspect, err) = common::inspect(&f.root(), &[]);
    assert_eq!(code, 0, "{err}");
    let inspected: BTreeSet<String> = inspect["tools"]
        .as_array()
        .map(|ts| {
            ts.iter()
                .filter_map(|t| t["name"].as_str().or_else(|| t.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let listed: BTreeSet<String> = r[&2]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert!(
        !listed.is_empty(),
        "a downstream repository is served tools"
    );
    assert_eq!(
        listed, inspected,
        "tools/list and `mcp --inspect` disagree about this repository"
    );
}

// ------------------------------------------------------------------------- three surfaces

/// The same read through three real processes — `majordomus run` on the command line,
/// `majordomus serve` over HTTP, and an MCP stdio session — answers the same data.
/// `properties.rs` proves this inside one process over a synthetic repository; this proves
/// it across the process boundary, through each surface's own serialiser and argument
/// handling, over a full fixture.
///
/// The capabilities are chosen by the registry, not by name: from every module, the first
/// deterministic read in canonical order that is exposed on all three surfaces, asked with
/// its own first benchmark case. A module that gains such a capability is in the test.
#[test]
fn the_command_line_http_and_mcp_answer_one_read_alike() {
    let f = Fixture::new();
    let app = common::load_app(&f);
    let ctx = &app.context;
    let case_ctx = CaseContext::of(ctx);
    // A capability tagged with one of these answers about the process that runs it — its
    // health, its lease, its peers, the executions it holds — so three processes are three
    // honest answers. The tags are the registry's own words; no capability is named here.
    const ABOUT_THE_PROCESS: &[&str] = &[
        "introspection",
        "diagnostic",
        "control-plane",
        "coordination",
    ];

    struct Candidate {
        module: String,
        id: String,
        tool: String,
        path: String,
        input: Value,
    }
    let mut candidates = Vec::new();
    for c in ctx.registry.iter() {
        if c.execution.effect != Effect::Read
            || c.kind != CapabilityKind::Query
            || c.tags
                .iter()
                .any(|t| ABOUT_THE_PROCESS.contains(&t.as_str()))
        {
            continue;
        }
        let (Some(tool), Some(route)) = (
            c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()),
            c.exposure.http.as_ref(),
        ) else {
            continue;
        };
        if route.method != HttpMethod::Get {
            continue;
        }
        let Some(input) = ctx
            .registry
            .cases(c.id.as_str())
            .and_then(|p| p(&case_ctx).into_iter().next())
            .map(|n| n.input)
        else {
            continue;
        };
        // a query string carries scalars; a case with a nested value is not a GET's
        if input
            .as_object()
            .is_some_and(|m| m.values().any(|v| v.is_object() || v.is_array()))
        {
            continue;
        }
        candidates.push(Candidate {
            module: c.module.to_string(),
            id: c.id.to_string(),
            tool,
            path: route.path.clone(),
            input,
        });
    }

    // The registry has no field that says "a function of the input and the repository
    // alone", so that is measured: every candidate is asked in two separate MCP processes,
    // and one whose two answers differ (an ephemeral port, a duration, a process id) is
    // not a read three processes could agree on. What is compared across surfaces is the
    // first candidate of each module that MCP itself answers the same way twice.
    let mut requests = opening();
    for (i, c) in candidates.iter().enumerate() {
        requests.push(call(100 + i as u64, &c.tool, c.input.clone()));
    }
    let answers = |x: &Exchange| -> Vec<Value> {
        let r = x.by_id();
        (0..candidates.len())
            .map(|i| r[&(100 + i as u64)]["result"].clone())
            .collect()
    };
    // ... once with no server on this checkout and once beside the one HTTP is asked
    // through, so a read that depends on whether a server is running is measured out too
    let first = answers(&standalone(&f, &requests));
    let state = state_home(&f);
    let served = common::Served::start_with_env(&f.root(), &[], &[("XDG_STATE_HOME", &state)]);
    let second = answers(&standalone(&f, &requests));
    let mut chosen: BTreeMap<&str, (usize, &Candidate)> = BTreeMap::new();
    let mut unstable = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        if without_times(&first[i].to_string()) != without_times(&second[i].to_string()) {
            unstable.push(c.id.as_str());
            continue;
        }
        chosen.entry(c.module.as_str()).or_insert((i, c));
    }
    assert!(
        chosen.len() >= 20,
        "reads from at least twenty modules, or the comparison is a sample of nothing: \
         {} chosen, {} unstable across two MCP processes ({unstable:?})",
        chosen.len(),
        unstable.len()
    );

    // `majordomus run`, one process per read: (exit code, stdout, stderr)
    let run = |id: &str, input: &Value| {
        let out = Command::new(BIN)
            .args([
                "run",
                id,
                "--input",
                &input.to_string(),
                "--format",
                "json",
                "--quiet",
            ])
            .current_dir(f.root())
            .env("MAJORDOMUS_SHARE", common::dist_share())
            .env_remove("MAJORDOMUS_ROOT")
            .env("XDG_STATE_HOME", &state)
            .stdin(Stdio::null())
            .output()
            .expect("spawn majordomus run");
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };
    let mut disagreements = Vec::new();
    for (module, (i, c)) in &chosen {
        let (id, via_mcp) = (&c.id, &first[*i]);
        let target = target_of(&c.path, &c.input);
        let (code, out, err) = run(id, &c.input);
        let (status, _, body) = served.request("GET", &target, None);
        if via_mcp["isError"] != false {
            // a case whose honest answer is a refusal: the other two refuse it too, each
            // in its own idiom — a non-zero exit, a 4xx — and neither answers with data
            if code == Some(0) {
                disagreements.push(format!("{module}/{id}: MCP refuses and `run` exits 0"));
            }
            if !(400..500).contains(&status) {
                disagreements.push(format!(
                    "{module}/{id}: MCP refuses and GET {target} answers {status}: {}",
                    trim(&body)
                ));
            }
            continue;
        }
        let via_mcp = without_times(&via_mcp["structuredContent"].to_string());
        // `run` prints the execution it ran; the capability's answer is its `output`
        match serde_json::from_str::<Value>(&out).map(|v| without_times(&v["output"].to_string())) {
            Ok(v) if code == Some(0) => {
                if v != via_mcp {
                    disagreements.push(format!(
                        "{module}/{id}: the command line and MCP differ\n  {}",
                        first_difference(&v, &via_mcp)
                    ));
                }
            }
            _ => disagreements.push(format!(
                "{module}/{id}: `run` exited {code:?}: {}{}",
                trim(&err),
                trim(&out)
            )),
        }
        match serde_json::from_str::<Value>(&body).map(|v| without_times(&v.to_string())) {
            Ok(v) if status == 200 => {
                if v != via_mcp {
                    disagreements.push(format!(
                        "{module}/{id}: HTTP and MCP differ\n  {}",
                        first_difference(&v, &via_mcp)
                    ));
                }
            }
            _ => disagreements.push(format!(
                "{module}/{id}: GET {target} answered {status}: {}",
                trim(&body)
            )),
        }
    }
    assert!(
        disagreements.is_empty(),
        "{} disagreement(s) over {} read(s) across the three surfaces:\n{}",
        disagreements.len(),
        chosen.len(),
        disagreements.join("\n")
    );
}

/// Where two texts first differ, with what each holds there.
fn first_difference(a: &str, b: &str) -> String {
    let at = a
        .bytes()
        .zip(b.bytes())
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()));
    let window = |s: &str| {
        let mut from = at.saturating_sub(80);
        while !s.is_char_boundary(from) {
            from -= 1;
        }
        let mut to = (at + 120).min(s.len());
        while !s.is_char_boundary(to) {
            to += 1;
        }
        s[from..to].to_string()
    };
    format!("at byte {at}: …{}…\n       mcp: …{}…", window(a), window(b))
}

/// Every occurrence of `mark` in `text`, each with the request id of the frame that carries
/// it (when the line is a frame) and the text either side: enough to say which answer
/// leaked and what, without printing a megabyte of listing.
fn around(text: &str, mark: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let id = serde_json::from_str::<Value>(line)
            .ok()
            .map(|v| v["id"].to_string())
            .unwrap_or_else(|| "-".into());
        for (at, _) in line.match_indices(mark) {
            let mut from = at.saturating_sub(160);
            while !line.is_char_boundary(from) {
                from -= 1;
            }
            let mut to = (at + mark.len() + 60).min(line.len());
            while !line.is_char_boundary(to) {
                to += 1;
            }
            out.push(format!("  id {id}: …{}…", &line[from..to]));
        }
    }
    out
}

/// The GET target of a capability's route for one input: scalars as a query string.
fn target_of(path: &str, input: &Value) -> String {
    let query: Vec<String> = input
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| {
                    let text = match v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    format!("{}={}", encode(k), encode(&text))
                })
                .collect()
        })
        .unwrap_or_default();
    if query.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", query.join("&"))
    }
}

/// `text` with every UTC timestamp (`2026-10-07T01:11:08Z`) replaced by a placeholder: an
/// answer generated a second later in another process is the same answer.
fn without_times(text: &str) -> String {
    const SHAPE: &[u8] = b"dddd-dd-ddTdd:dd:ddZ";
    let bytes = text.as_bytes();
    let fits = |at: usize| {
        bytes.len() >= at + SHAPE.len()
            && SHAPE.iter().zip(&bytes[at..]).all(|(s, c)| match s {
                b'd' => c.is_ascii_digit(),
                s => s == c,
            })
    };
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < bytes.len() {
        if fits(at) {
            out.push_str("<time>");
            at += SHAPE.len();
        } else {
            let next = (at + 1..=bytes.len())
                .find(|i| text.is_char_boundary(*i))
                .unwrap_or(bytes.len());
            out.push_str(&text[at..next]);
            at = next;
        }
    }
    out
}

fn trim(s: &str) -> String {
    s.chars().take(400).collect()
}

/// Percent-encoding of a query component: everything but the unreserved characters.
fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}
