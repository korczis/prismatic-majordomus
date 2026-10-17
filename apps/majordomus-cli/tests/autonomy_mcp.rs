//! The autonomy capabilities over MCP, as a client asks them (I2790): `majordomus_autonomy`,
//! `majordomus_autonomy_explain` and `majordomus_autonomy_friction` sent as JSON-RPC
//! tools/call frames to a checkout's server whose home directory is a fixture, so that the
//! settings it reads and the transcripts it counts are the test's and nobody else's.

mod common;

use serde_json::{json, Value};

/// The client's transcript directory name of a path: every byte that is not a letter, a
/// digit or `-` becomes `-`.
fn slug(path: &std::path::Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

#[test]
fn over_mcp_the_autonomy_tools_read_the_home_they_are_given() {
    let f = common::Fixture::new();
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    std::fs::create_dir_all(h.join(".claude")).unwrap();
    std::fs::write(
        h.join(".claude/settings.json"),
        r#"{"permissions":{"allow":["Bash(private-host *)"]},"autoMode":{"allow":["$defaults"]}}"#,
    )
    .unwrap();
    // one denial in a transcript of this checkout, under every spelling of its path
    let line = |content: Value| {
        json!({ "timestamp": "2099-01-01T00:00:00.000Z", "message": { "content": content } })
            .to_string()
    };
    let transcript = [
        line(json!([{ "type": "tool_use", "id": "t1", "name": "Bash",
                      "input": { "command": "private-command" } }])),
        line(json!([{ "type": "tool_result", "tool_use_id": "t1", "is_error": true,
                      "content": "denied by the Claude Code auto mode classifier. Reason: [CI Bypass]." }])),
    ]
    .join("\n");
    let mut roots = vec![f.root()];
    if let Ok(c) = f.root().canonicalize() {
        roots.push(c);
    }
    for root in &roots {
        let dir = h.join(".claude/projects").join(slug(root));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("s.jsonl"), &transcript).unwrap();
    }
    let home_str = h.to_str().unwrap();
    let served = common::Served::start_with_env(&f.root(), &[], &[("HOME", home_str)]);

    let initialize = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "autonomy-test", "version": "0.0.0" },
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

    let call = |id: u64, name: &str, arguments: Value| -> Value {
        let frame = json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": name, "arguments": arguments },
        });
        let (_, _, body) = served.request_with("POST", "/mcp", Some(&frame.to_string()), &header);
        let answer: Value = serde_json::from_str(&body).unwrap_or_else(|e| panic!("{e}: {body}"));
        assert_eq!(answer["result"]["isError"], false, "{name}: {body}");
        answer["result"]["structuredContent"].clone()
    };

    let status = call(2, "majordomus_autonomy", json!({}));
    assert_eq!(
        status["schema"], "majordomus.autonomy.status/v1",
        "{status}"
    );
    let user = status["layers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["scope"] == "user")
        .expect("the user layer is read")
        .clone();
    assert_eq!(
        (user["allow"].as_u64(), user["auto_mode_allow"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(user["path"], "~/.claude/settings.json");
    assert_ne!(
        status["profile"], "isolated",
        "no container was verified: {status}"
    );
    assert!(
        !status.to_string().contains("private-host"),
        "rule text never leaves"
    );

    let decision = call(
        3,
        "majordomus_autonomy_explain",
        json!({ "command": "git push --force origin master", "profile": "isolated" }),
    );
    assert_eq!(decision["decision"], "require_approval", "{decision}");
    assert_eq!(decision["risk"], "external", "{decision}");

    let friction = call(
        4,
        "majordomus_autonomy_friction",
        json!({ "since_days": 1 }),
    );
    assert_eq!(friction["by_reason"]["CI Bypass"], 1, "{friction}");
    assert_eq!(friction["by_tool"]["Bash"], 1, "{friction}");
    assert!(
        !friction.to_string().contains("private-command"),
        "no payload leaves"
    );

    served.request_with("DELETE", "/mcp", None, &header);
}
