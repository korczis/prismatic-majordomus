//! The MCP server as a client sees it: a child process, real protocol messages on stdin,
//! and only protocol messages on stdout.

mod common;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use common::{rule, Fixture, BIN};
use serde_json::{json, Value};

/// Send `requests` (one JSON message per line), close stdin, collect every stdout line
/// parsed as JSON, keyed by id, plus stderr and the exit code.
fn session(
    cwd: &std::path::Path,
    extra_args: &[&str],
    requests: &[Value],
) -> (i32, Vec<Value>, String) {
    let mut args = vec!["mcp"];
    args.extend_from_slice(extra_args);
    let mut child = Command::new(BIN)
        .args(&args)
        .current_dir(cwd)
        .env("MAJORDOMUS_LOG", "debug")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn majordomus mcp");
    {
        let mut stdin = child.stdin.take().unwrap();
        for r in requests {
            writeln!(stdin, "{r}").unwrap();
        }
        // stdin drops here: EOF ends the session
    }
    let stdout = BufReader::new(child.stdout.take().unwrap());
    let mut frames = Vec::new();
    for line in stdout.lines() {
        let line = line.expect("stdout is UTF-8");
        assert!(
            !line.trim().is_empty(),
            "an empty stdout line is not a protocol frame"
        );
        let v: Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("stdout frame is not JSON ({e}): {line}"));
        assert_eq!(v["jsonrpc"], "2.0", "not a JSON-RPC frame: {line}");
        frames.push(v);
    }
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        frames,
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn by_id(frames: &[Value]) -> BTreeMap<u64, &Value> {
    frames
        .iter()
        .filter_map(|f| f["id"].as_u64().map(|id| (id, f)))
        .collect()
}

fn init() -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "0" } } })
}
fn initialized() -> Value {
    json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
}

#[test]
fn handshake_discovery_and_a_real_round_trip() {
    let f = Fixture::new();
    let requests = [
        init(),
        initialized(),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/list" }),
        json!({ "jsonrpc": "2.0", "id": 4, "method": "resources/read", "params": { "uri": "majordomus://rule/project.alpha@1" } }),
        json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/list" }),
        json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": { "name": "majordomus_get", "arguments": { "uri": "majordomus://rule/project.alpha@1" } } }),
        json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": "majordomus_list", "arguments": { "kind": "rule" } } }),
        json!({ "jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": { "name": "majordomus_search", "arguments": { "query": "normative" } } }),
        json!({ "jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": { "name": "majordomus_repository", "arguments": {} } }),
        json!({ "jsonrpc": "2.0", "id": 10, "method": "resources/read", "params": { "uri": "majordomus://repository" } }),
    ];
    let (code, frames, stderr) = session(&f.root(), &[], &requests);
    assert_eq!(code, 0, "server exits 0 at EOF; stderr:\n{stderr}");
    assert!(
        stderr.contains("index built"),
        "diagnostics go to stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("input ended"),
        "clean shutdown is logged:\n{stderr}"
    );
    let r = by_id(&frames);
    assert_eq!(
        r.len(),
        10,
        "one response per request, none for the notification: {frames:?}"
    );

    let init = &r[&1]["result"];
    assert_eq!(init["protocolVersion"], "2025-06-18");
    assert_eq!(init["serverInfo"]["name"], "majordomus");
    assert_eq!(init["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(init["capabilities"]["resources"].is_object());
    assert!(init["capabilities"]["tools"].is_object());
    assert!(
        init["capabilities"].get("prompts").is_none(),
        "prompts are not advertised"
    );
    assert!(init["instructions"].as_str().unwrap().contains("state ok"));

    assert_eq!(r[&2]["result"], json!({}));

    let resources = r[&3]["result"]["resources"].as_array().unwrap();
    let alpha = resources
        .iter()
        .find(|x| x["uri"] == "majordomus://rule/project.alpha@1")
        .expect("alpha listed");
    assert_eq!(alpha["name"], "project.alpha@1");
    assert_eq!(alpha["title"], "Alpha");
    assert_eq!(alpha["mimeType"], "text/markdown");
    assert_eq!(
        alpha["_meta"]["majordomus"]["provenance"]["path"],
        ".ai/repo/rules/project/alpha.v1.md"
    );
    assert_eq!(
        alpha["_meta"]["majordomus"]["provenance"]["section"],
        "rules"
    );
    assert_eq!(resources[0]["uri"], "majordomus://repository");

    let contents = &r[&4]["result"]["contents"][0];
    assert_eq!(contents["uri"], "majordomus://rule/project.alpha@1");
    assert_eq!(contents["mimeType"], "text/markdown");
    assert_eq!(contents["text"], rule("project.alpha", 1, "Alpha"));

    let tools: Vec<&str> = r[&5]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    // ordered by canonical id, one per capability with an MCP tool exposure
    assert_eq!(
        tools,
        [
            "majordomus_capability",
            "majordomus_capabilities",
            "majordomus_get",
            "majordomus_list",
            "majordomus_search",
            "majordomus_repository"
        ]
    );
    let list_tool = &r[&5]["result"]["tools"][3];
    assert_eq!(list_tool["_meta"]["majordomus"]["id"], "objects.list");
    assert!(list_tool["inputSchema"]["properties"]["kind"].is_object());
    assert!(list_tool["outputSchema"]["properties"]["objects"].is_object());
    assert!(r[&5]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .all(|t| t["annotations"]["readOnlyHint"] == true));

    let get = &r[&6]["result"];
    assert_eq!(get["isError"], false);
    assert_eq!(
        get["structuredContent"]["provenance"]["path"],
        ".ai/repo/rules/project/alpha.v1.md"
    );
    assert_eq!(get["structuredContent"]["metadata"]["id"], "project.alpha");
    assert_eq!(get["content"][0]["type"], "text");

    let list = &r[&7]["result"]["structuredContent"];
    assert_eq!(list["count"], 1);
    assert_eq!(
        list["objects"][0]["uri"],
        "majordomus://rule/project.alpha@1"
    );

    let search = &r[&8]["result"]["structuredContent"];
    assert_eq!(search["count"], 1);
    assert!(search["hits"][0]["snippet"]
        .as_str()
        .unwrap()
        .contains("normative"));

    let repo = &r[&9]["result"]["structuredContent"];
    assert_eq!(repo["state"], "ok");
    assert_eq!(repo["repository"]["root"], f.root().to_str().unwrap());
    assert_eq!(repo["kinds"]["rule"], 1);
    let text: Value =
        serde_json::from_str(r[&10]["result"]["contents"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text["state"], "ok");
}

#[test]
fn protocol_errors_are_protocol_shaped() {
    let f = Fixture::new();
    let mut child = Command::new(BIN)
        .arg("mcp")
        .current_dir(f.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{}", init()).unwrap();
        writeln!(stdin, "this is not json").unwrap();
        writeln!(
            stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "id": 2, "method": "prompts/list" })
        )
        .unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/read", "params": { "uri": "majordomus://rule/nope@1" } })).unwrap();
        writeln!(
            stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "id": 4, "method": "resources/read" })
        )
        .unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "majordomus_get", "arguments": {} } })).unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": { "name": "no_such_tool" } })).unwrap();
        writeln!(
            stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "method": "notifications/cancelled" })
        )
        .unwrap();
        writeln!(stdin, "{}", json!([{ "jsonrpc": "2.0", "id": 7, "method": "ping" }, { "jsonrpc": "2.0", "id": 8, "method": "ping" }])).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let lines: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).expect("JSON frame"))
        .collect();
    assert_eq!(lines.len(), 8, "{lines:?}");
    assert_eq!(lines[1]["error"]["code"], -32700);
    assert_eq!(lines[1]["id"], Value::Null);
    assert_eq!(lines[2]["error"]["code"], -32601);
    assert_eq!(lines[3]["error"]["code"], -32002);
    assert_eq!(lines[4]["error"]["code"], -32602);
    assert_eq!(
        lines[5]["result"]["isError"], true,
        "a refused tool call is a result, not a protocol error"
    );
    assert_eq!(lines[6]["error"]["code"], -32602);
    let batch = lines[7].as_array().expect("a batch answers with a batch");
    assert_eq!(batch.len(), 2);
}

#[test]
fn stdout_is_protocol_only_even_at_trace_level_with_a_degraded_layer() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/project/broken.v1.md",
        "---\nid: project.broken\n",
    );
    f.commit("broken");
    let mut child = Command::new(BIN)
        .arg("mcp")
        .current_dir(f.root())
        .env("MAJORDOMUS_LOG", "trace")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{}", init()).unwrap();
        writeln!(
            stdin,
            "{}",
            json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" })
        )
        .unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    for line in stdout.lines() {
        let v: Value = serde_json::from_str(line)
            .unwrap_or_else(|_| panic!("non-protocol stdout line: {line}"));
        assert!(v["id"].is_number());
    }
    assert!(
        stderr.contains("malformed_front_matter"),
        "the diagnostic is on stderr:\n{stderr}"
    );
    let init_frame: Value = serde_json::from_str(stdout.lines().next().unwrap()).unwrap();
    assert!(init_frame["result"]["instructions"]
        .as_str()
        .unwrap()
        .contains("state degraded"));
}

#[test]
fn strict_refuses_a_degraded_layer_before_serving() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/rules/project/broken.v1.md",
        "---\nid: project.broken\n",
    );
    f.commit("broken");
    let (code, out, err) =
        common::run_in(&f.root(), &["mcp", "--strict"], &format!("{}\n", init()));
    assert_eq!(code, 10, "{err}");
    assert!(out.is_empty(), "nothing is served: {out}");
    assert!(err.contains("refusing to serve under --strict"), "{err}");
}

#[test]
fn serving_mutates_nothing() {
    let f = Fixture::new();
    let before = f.snapshot();
    let requests = [
        init(),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/read", "params": { "uri": "majordomus://document/README.md" } }),
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "majordomus_search", "arguments": { "query": "fixture" } } }),
    ];
    let (code, _, _) = session(&f.root(), &[], &requests);
    assert_eq!(code, 0);
    let (code, _, _) = common::run_in(
        &f.root(),
        &["mcp", "--inspect", "--discovery", "filesystem"],
        "",
    );
    assert_eq!(code, 0);
    assert_eq!(
        f.snapshot(),
        before,
        "serving or inspecting changed the repository"
    );
    assert!(!f.path(".ai/local/cache").exists());
}

#[test]
fn a_client_that_closes_its_read_end_ends_the_session() {
    let f = Fixture::new();
    let mut child = Command::new(BIN)
        .arg("mcp")
        .current_dir(f.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let mut stdin = child.stdin.take().unwrap();
    // Enough frames to overrun any pipe buffer if the server kept writing.
    for i in 0..2000u64 {
        let msg = json!({ "jsonrpc": "2.0", "id": i, "method": "resources/list" });
        if writeln!(stdin, "{msg}").is_err() {
            break;
        }
    }
    drop(stdin);
    let status = child.wait().unwrap();
    assert_eq!(
        status.code(),
        Some(0),
        "a gone client is a clean end, not a crash"
    );
}
