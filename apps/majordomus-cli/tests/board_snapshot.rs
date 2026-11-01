//! The delivery board's documents (I4294), end to end: `majordomus board snapshot` and the MCP
//! tool `majordomus_board_snapshot` are one capability, the lanes come from the share
//! directory's board.yaml, and an open milestone in no lane is refused by both.

mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde_json::{json, Value};

use common::Fixture;

/// A copy of the distribution's share directory whose board.yaml lanes the fixture's
/// milestone, so that the fixture composes.
fn share_with_lane(dir: &Path) -> PathBuf {
    let share = dir.join("share");
    let ok = std::process::Command::new("cp")
        .arg("-R")
        .arg(common::dist_share())
        .arg(&share)
        .status()
        .expect("cp runs");
    assert!(ok.success(), "the share is copied");
    std::fs::write(
        share.join("board.yaml"),
        "version: 1\nlanes:\n  - key: fixture\n    title: \"The fixture\"\n    milestones:\n      - fixture-milestone\n",
    )
    .expect("board.yaml written");
    share
}

fn cli(f: &Fixture, share: &Path, args: &[&str]) -> (i32, String, String) {
    let out = common::majordomus()
        .args(args)
        .env("MAJORDOMUS_SHARE", share)
        .current_dir(f.root())
        .stdin(Stdio::null())
        .output()
        .expect("the executable runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// One `tools/call` of `majordomus_board_snapshot` over stdio; the result frame.
fn mcp_board_snapshot(f: &Fixture, share: &Path, arguments: Value) -> Value {
    let mut child = common::majordomus()
        .arg("mcp")
        .env("MAJORDOMUS_SHARE", share)
        .current_dir(f.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("mcp starts");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } })).expect("initialize");
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "majordomus_board_snapshot", "arguments": arguments } })).expect("tools/call");
    }
    let out = child.wait_with_output().expect("mcp exits");
    let frames: Vec<Value> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).expect("a JSON-RPC frame"))
        .collect();
    frames[1]["result"].clone()
}

#[test]
fn board_snapshot_is_one_answer_on_the_command_line_and_over_mcp() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().expect("a directory");
    let share = share_with_lane(dir.path());

    let (code, out, err) = cli(&f, &share, &["board", "snapshot", "--by", "s1", "--format", "json"]);
    assert_eq!(code, 0, "{err}");
    let answer: Value = serde_json::from_str(&out).expect("one JSON document");
    let docs = &answer["documents"];
    assert_eq!(docs["issues-fixture"]["lane"], "fixture");
    assert_eq!(docs["issues-fixture"]["items"][0]["id"], "I0001");
    assert_eq!(docs["meta"]["lanes"], json!([["fixture", "The fixture"]]));
    assert_eq!(docs["meta"]["syncedBy"], "s1");
    assert_eq!(docs["meta"]["sha"], answer["master_sha"]);
    assert_eq!(answer["master_sha"].as_str().map(str::len), Some(40));
    assert!(docs["prs"]["unavailable"].is_string(), "{}", docs["prs"]);
    assert_eq!(docs["intents"]["items"][0]["id"], "fixture-intent");
    assert_eq!(docs["milestones"]["items"][0]["l"], "fixture");
    assert!(answer["digest"].as_str().is_some_and(|d| d.starts_with("sha256:")));
    for (id, n) in answer["sizes"].as_object().expect("sizes") {
        assert!(n.as_u64() < Some(250_000), "{id}");
    }

    let mcp = mcp_board_snapshot(&f, &share, json!({ "by": "another" }));
    assert_ne!(mcp["isError"], true, "{mcp}");
    let structured = &mcp["structuredContent"];
    assert_eq!(structured["documents"]["meta"]["syncedBy"], "another");
    assert_eq!(
        structured["digest"], answer["digest"],
        "the same state is the same digest on every surface"
    );
    assert_eq!(structured["documents"]["issues-fixture"], docs["issues-fixture"]);

    let (code, out, _) = cli(&f, &share, &["board", "snapshot"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("digest     sha256:"), "{out}");
    assert!(out.contains("issues-fixture"), "{out}");
}

#[test]
fn board_snapshot_refuses_an_open_milestone_in_no_lane_on_every_surface() {
    let f = Fixture::new();
    // the distribution's lanes are this repository's, and the fixture's milestone is in none
    let share = common::dist_share();
    let (code, out, err) = cli(&f, &share, &["board", "snapshot", "--format", "json"]);
    assert_ne!(code, 0, "{out}");
    assert!(err.contains("fixture-milestone") && err.contains("board.yaml"), "{err}");

    let mcp = mcp_board_snapshot(&f, &share, json!({}));
    assert_eq!(mcp["isError"], true, "{mcp}");
    assert!(mcp.to_string().contains("fixture-milestone"), "{mcp}");
}
