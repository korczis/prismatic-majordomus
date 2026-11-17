//! The `observatory` module over MCP, as a client calls it (ADR 0158, I2678, I2679).
//!
//! A repository that carries a recording answers it from `majordomus_observatory_replay`; a
//! process that runs no mesh captures no scene and has no journal to follow, and says so
//! rather than answering an empty mesh as if it were one.

mod common;
use std::io::Write;
use std::process::Stdio;

use common::Fixture;
use serde_json::{json, Value};

/// One `tools/call` over the stdio transport, after `initialize`: the structured result.
fn mcp_call(f: &Fixture, tool: &str, arguments: Value) -> Value {
    let mut child = common::majordomus()
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
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": tool, "arguments": arguments } })).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let frames: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert!(
        frames[1]["result"]["isError"] != json!(true),
        "{tool} answered an error: {}",
        frames[1]
    );
    frames[1]["result"]["structuredContent"].clone()
}

/// The recording a repository carries is the answer of the replay tool, and a process that
/// runs no mesh captures no scene and follows no journal.
#[test]
fn the_observatory_tools_answer_over_mcp() {
    let f = Fixture::new();
    let replay = mcp_call(&f, "majordomus_observatory_replay", json!({}));
    assert_eq!(replay["present"], json!(false), "{replay}");

    f.write(
        ".ai/repo/observatory/recording.json",
        &json!({
            "schema": "observatory-recording/v1",
            "recorded_at": "2026-10-14T05:00:00Z",
            "scenes": { "mesh": { "machines": [{ "id": "m1", "label": "machine 1", "runtimes": [] }], "known": 1, "refused": 0 } },
            "manifest": { "sources": ["mesh.peers"], "removed": ["identities"] },
        })
        .to_string(),
    );
    let replay = mcp_call(&f, "majordomus_observatory_replay", json!({}));
    assert_eq!(replay["present"], json!(true), "{replay}");
    assert_eq!(
        replay["recording"]["scenes"]["mesh"]["machines"][0]["label"],
        "machine 1"
    );

    // no mesh runs in this process: nothing live to record, and the manifest says so
    let capture = mcp_call(&f, "majordomus_observatory_capture", json!({}));
    assert_eq!(capture["schema"], "observatory-recording/v1", "{capture}");
    assert_eq!(capture["scenes"], json!({}), "{capture}");
    assert_eq!(capture["manifest"]["sources"], json!([]), "{capture}");

    // and no journal to follow: inactive, never an expired cursor or invented events
    let events = mcp_call(&f, "majordomus_observatory_events", json!({ "after": 7 }));
    assert_eq!(events["active"], json!(false), "{events}");
    assert_eq!(events["expired"], json!(false), "{events}");
    assert_eq!(events["events"], json!([]), "{events}");
}
