//! Structural performance guarantees, read from the process itself: after startup, no
//! MCP or HTTP request scans the repository, builds the index or the registry, derives a
//! schema, or builds a projection. Hundreds of requests through the real transports, then
//! the counters `perf.counters` answers must match the ones read before them. A refactor
//! that rebuilds canonical state per request fails here before anyone measures latency.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use common::{Fixture, Served, BIN};
use serde_json::{json, Value};

const REQUESTS: usize = 200;

/// The counters that must not move once a process serves.
fn startup_work(snapshot: &Value) -> Vec<(String, u64)> {
    [
        "repository_scans",
        "index_builds",
        "registry_builds",
        "schema_generations",
        "mcp_projection_builds",
        "openapi_builds",
        "http_projection_builds",
    ]
    .iter()
    .map(|k| {
        (
            k.to_string(),
            snapshot[k]
                .as_u64()
                .unwrap_or_else(|| panic!("counter {k}")),
        )
    })
    .collect()
}

#[test]
fn hundreds_of_mcp_requests_rebuild_nothing() {
    let f = Fixture::new();
    let mut child = Command::new(BIN)
        .args(["mcp", "--standalone"])
        .current_dir(f.root())
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env("MAJORDOMUS_LOG", "warn")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut id = 0u64;
    let mut request = |method: &str, params: Value| -> Value {
        id += 1;
        writeln!(
            stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
        )
        .unwrap();
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).expect("a frame")
    };
    request(
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "hot-path", "version": "0" } }),
    );
    // one of each, so that every lazily prepared listing exists before the count is taken
    request("tools/list", json!({}));
    request("resources/list", json!({}));
    request(
        "resources/read",
        json!({ "uri": "majordomus://repository" }),
    );
    let before = request(
        "tools/call",
        json!({ "name": "majordomus_perf", "arguments": {} }),
    )["result"]["structuredContent"]
        .clone();
    let baseline = startup_work(&before);
    assert_eq!(before["repository_scans"], 1, "one scan at startup");
    assert_eq!(before["index_builds"], 1);
    assert_eq!(before["registry_builds"], 1);
    assert!(before["schema_generations"].as_u64().unwrap() > 0);
    assert_eq!(
        before["mcp_projection_builds"], 2,
        "tools and resources, once each"
    );

    let calls: [(&str, Value); 7] = [
        ("tools/list", json!({})),
        ("resources/list", json!({})),
        (
            "tools/call",
            json!({ "name": "majordomus_search", "arguments": { "query": "fixture" } }),
        ),
        (
            "tools/call",
            json!({ "name": "majordomus_list", "arguments": { "kind": "rule" } }),
        ),
        (
            "tools/call",
            json!({ "name": "majordomus_get", "arguments": { "uri": "majordomus://rule/project.alpha@1" } }),
        ),
        (
            "resources/read",
            json!({ "uri": "majordomus://rule/project.alpha@1" }),
        ),
        (
            "tools/call",
            json!({ "name": "majordomus_get", "arguments": { "uri": "majordomus://repository" } }),
        ),
    ];
    for i in 0..REQUESTS {
        let (method, params) = &calls[i % calls.len()];
        let frame = request(method, params.clone());
        assert!(frame.get("result").is_some(), "{frame}");
    }
    let after = request(
        "tools/call",
        json!({ "name": "majordomus_perf", "arguments": {} }),
    )["result"]["structuredContent"]
        .clone();
    assert_eq!(
        startup_work(&after),
        baseline,
        "a request rebuilt canonical state:\nbefore {before}\nafter {after}"
    );
    let executions = after["executions"].as_u64().unwrap() - before["executions"].as_u64().unwrap();
    assert!(
        executions >= (REQUESTS / 2) as u64,
        "tool calls went through the executor: {executions}"
    );
    assert!(
        after["cache_hits"].as_u64().unwrap() > before["cache_hits"].as_u64().unwrap(),
        "the repeated search was answered from the cache"
    );
    drop(stdin);
    assert_eq!(child.wait().unwrap().code(), Some(0));
}

#[test]
fn hundreds_of_http_requests_rebuild_nothing_and_openapi_is_built_once() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let _ = s.get("/openapi.json");
    let (_, _, docs) = s.request("GET", "/docs", None);
    assert!(docs.contains("swagger-ui"));
    let (_, before) = s.get("/api/v1/perf");
    let baseline = startup_work(&before);
    assert_eq!(before["openapi_builds"], 1);
    assert_eq!(before["http_projection_builds"], 1);
    let paths = [
        "/openapi.json",
        "/docs",
        "/api/v1/capabilities",
        "/api/v1/search?query=fixture",
        "/api/v1/objects?kind=rule",
        "/api/v1/object?uri=majordomus://rule/project.alpha@1",
        "/api/v1/repository",
        "/",
    ];
    for i in 0..REQUESTS {
        let (status, _, _) = s.request("GET", paths[i % paths.len()], None);
        assert_eq!(status, 200, "{}", paths[i % paths.len()]);
    }
    let (_, after) = s.get("/api/v1/perf");
    assert_eq!(
        startup_work(&after),
        baseline,
        "a request rebuilt canonical state:\nbefore {before}\nafter {after}"
    );
    assert_eq!(
        after["openapi_builds"], 1,
        "the document is rendered once and served from memory"
    );
    assert!(after["cache_hits"].as_u64().unwrap() > before["cache_hits"].as_u64().unwrap());
}
