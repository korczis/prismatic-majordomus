//! What remains of an intent (ADR 0117) through the built executable: the command line, HTTP
//! and MCP answer the same remains, outcome and next action for one tree, from the one
//! declaration of `intent_realization.work`, and two reads of an unchanged tree are the same.

// claims: intent-remains-derived

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::{run_in, Fixture, Served, BIN};
use serde_json::{json, Value};

/// One MCP tool call over stdio; the structured content of its answer.
fn tool(f: &Fixture, name: &str, arguments: Value) -> Value {
    let mut child = Command::new(BIN)
        .arg("mcp")
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .current_dir(f.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } })).unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": name, "arguments": arguments } })).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let last = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .last()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .unwrap();
    assert_eq!(last["result"]["isError"], false, "{name}: {last}");
    last["result"]["structuredContent"].clone()
}

fn json_of(f: &Fixture, args: &[&str]) -> Value {
    let mut argv = args.to_vec();
    argv.extend(["--format", "json"]);
    let (_, out, err) = run_in(&f.root(), &argv, "");
    serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("{args:?} did not print JSON ({e}):\n{out}\n{err}"))
}

/// The intents of a realization answer, which is what the three surfaces must agree on; the
/// work units carry observation times of their own.
fn intents(answer: &Value) -> Value {
    answer["intents"].clone()
}

/// Criterion `surfaces-agree-and-repeat`.
#[test]
fn the_three_surfaces_answer_what_remains_alike_and_twice_alike() {
    let f = Fixture::new();
    let cli = json_of(&f, &["intent", "realization"]);
    let view = &cli["intents"][0];
    assert_eq!(view["intent"], "fixture-intent", "{cli}");
    // nothing has run in the fixture and its one issue is open: work is moving
    assert_eq!(view["outcome"], "progressing", "{view}");
    let unmet = &view["unmet"][0];
    assert_eq!(unmet["remains"], "progressing", "{unmet}");
    assert_eq!(unmet["basis"], "work_open", "{unmet}");
    assert_eq!(unmet["next"]["action"], "work", "{unmet}");
    assert!(view["review_revision"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));
    assert!(view["remains_digest"]
        .as_str()
        .is_some_and(|s| !s.is_empty()));

    let mcp = tool(&f, "majordomus_intent_realization", json!({}));
    assert_eq!(
        intents(&cli),
        intents(&mcp),
        "the command line and MCP disagree"
    );
    let served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/intents/realization");
    assert_eq!(status, 200);
    assert_eq!(
        intents(&cli),
        intents(&http),
        "the command line and HTTP disagree"
    );

    let again = json_of(&f, &["intent", "realization"]);
    assert_eq!(
        intents(&cli),
        intents(&again),
        "an unchanged tree answered differently"
    );
}
