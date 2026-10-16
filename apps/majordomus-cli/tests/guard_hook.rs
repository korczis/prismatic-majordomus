//! `majordomus guard hook` as a provider's pre-tool hook runs it: the payload on stdin, the
//! verdict as the exit code (I2735). The judgement is proved in `src/guard.rs`; this proves
//! the command reads stdin, asks the checkout's running server for its claims, and answers
//! with the code the provider reads.

mod common;

use std::io::Write;
use std::process::Stdio;

/// Run `guard hook` in `dir` with `payload` on stdin: the exit code.
fn hook(dir: &std::path::Path, payload: &str) -> i32 {
    let mut child = common::majordomus()
        .args(["guard", "hook", "--provider", "claude-code"])
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the executable starts");
    child
        .stdin
        .take()
        .expect("a stdin")
        .write_all(payload.as_bytes())
        .expect("the payload is written");
    let status = child.wait().expect("the hook ends");
    status.code().expect("an exit code")
}

#[test]
fn the_guard_lets_the_tool_run_when_the_payload_names_no_file_of_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(hook(dir.path(), "not json"), 0);
    assert_eq!(hook(dir.path(), r#"{"tool_input":{"command":"ls"}}"#), 0);
    let edit = r#"{"session_id":"s1","tool_input":{"file_path":"a.md"}}"#;
    assert_eq!(hook(dir.path(), edit), 0, "no manifest is above the file");
}

/// The repository declares its mesh enabled, but the declaration is outside every source
/// class the fixture indexes, so its server runs without the mesh: the guard asks that
/// server, hears that the mesh does not run, and lets the edit through, saying why.
#[test]
fn beside_a_server_that_does_not_run_the_mesh_the_guard_asks_it_and_enforces_nothing() {
    let f = common::Fixture::new();
    let policy = format!(
        "{}\nsession:\n  edits_require_claim: true\n",
        common::POLICY
    );
    f.write(".ai/repo/policy.yaml", &policy);
    f.write(".ai/repo/mesh/mesh.yaml", "enabled: true\n");
    let _served = common::Served::start(&f.root(), &[]);

    let edit = r#"{"session_id":"s1","tool_input":{"file_path":"docs/new.md"}}"#;
    assert_eq!(
        hook(&f.root(), edit),
        0,
        "an undecidable edit is let through"
    );

    let out = common::majordomus()
        .args(["run", "guard.edit", "--format", "json", "--input"])
        .arg(r#"{"path":"docs/new.md","session":"claude-code-s1"}"#)
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .current_dir(f.root())
        .output()
        .expect("guard.edit runs");
    let answer: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON");
    let verdict = &answer["output"];
    assert_eq!(verdict["allowed"], true, "{answer}");
    assert_eq!(verdict["ground"], "undecided", "{answer}");
    let detail = verdict["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains("the server does not run the mesh"),
        "the running server was asked: {detail}"
    );
}

/// The guard over MCP, as a client asks it: `majordomus_guard_edit` sent as a JSON-RPC
/// tools/call to the checkout's server. The server decided its own mesh, so it answers from
/// that decision rather than asking anyone.
#[test]
fn over_mcp_the_guard_answers_from_the_servers_own_mesh() {
    use serde_json::{json, Value};

    let f = common::Fixture::new();
    let policy = format!(
        "{}\nsession:\n  edits_require_claim: true\n",
        common::POLICY
    );
    f.write(".ai/repo/policy.yaml", &policy);
    f.write(".ai/repo/mesh/mesh.yaml", "enabled: true\n");
    let served = common::Served::start(&f.root(), &[]);

    let initialize = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "guard-test", "version": "0.0.0" },
        },
    });
    let (status, headers, _) = served.request("POST", "/mcp", Some(&initialize.to_string()));
    assert_eq!(status, 200, "initialize was not accepted");
    let session = headers
        .iter()
        .find(|(k, _)| k == "mcp-session-id")
        .map(|(_, v)| v.clone())
        .expect("the server mints a session id");
    let header = [("Mcp-Session-Id", session.as_str())];
    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    served.request_with("POST", "/mcp", Some(&initialized.to_string()), &header);

    let call = json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {
            "name": "majordomus_guard_edit",
            "arguments": { "path": "docs/new.md", "session": "claude-code-s1" },
        },
    });
    let (_, _, body) = served.request_with("POST", "/mcp", Some(&call.to_string()), &header);
    let answer: Value = serde_json::from_str(&body).unwrap_or_else(|e| panic!("{e}: {body}"));
    assert_eq!(answer["result"]["isError"], false, "{body}");
    let verdict = &answer["result"]["structuredContent"];
    assert_eq!(verdict["allowed"], true, "{verdict}");
    assert_eq!(verdict["ground"], "undecided", "{verdict}");
    assert!(
        verdict["detail"]
            .as_str()
            .is_some_and(|d| d.contains("does not run the mesh")),
        "{verdict}"
    );
    // the client ends its session, so the server can stop at once
    served.request_with("DELETE", "/mcp", None, &header);
}
