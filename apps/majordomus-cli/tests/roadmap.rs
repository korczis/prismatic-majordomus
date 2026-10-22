//! The roadmap through the built executable (ADR 0125): `plan.roadmap` answers with the
//! milestones in the horizons their derived status puts them in, the intents they realise and
//! what the committed facts disagree about — and no milestone carries a version it was given
//! ahead of a release.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::{Fixture, BIN};
use serde_json::{json, Value};

/// One MCP tool call over stdio; the structured content of its answer.
fn tool(f: &Fixture, name: &str) -> Value {
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
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": name, "arguments": {} } })).unwrap();
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

#[test]
fn the_roadmap_is_answered_in_horizons_with_the_intents_each_milestone_realises() {
    let f = Fixture::new();
    let answer = tool(&f, "majordomus_plan_roadmap");
    let inferred = &answer["inferred"];
    assert_eq!(inferred["schema"], "majordomus/roadmap/v1");

    let horizons: Vec<&str> = inferred["horizons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["horizon"].as_str().unwrap())
        .collect();
    assert_eq!(horizons, ["now", "next", "later", "done"]);

    // the fixture's milestone stands in exactly one horizon, and names the intent over it
    let placed: Vec<&Value> = inferred["horizons"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|h| h["milestones"].as_array().unwrap())
        .filter(|m| m["id"] == "fixture-milestone")
        .collect();
    assert_eq!(placed.len(), 1, "{inferred}");
    assert_eq!(placed[0]["intents"], json!(["fixture-intent"]));
    assert!(placed[0].get("version").is_none());

    // every inference is stated, so a reader can judge it
    assert!(inferred["basis"]
        .as_array()
        .unwrap()
        .iter()
        .any(|b| b.as_str().unwrap().contains("published release")));
}
