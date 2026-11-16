//! `plan.drift` through the built executable (I2741, ADR 0166 decision 2): in a fixture
//! repository a READY issue whose merged pull request names it is `merged_not_recorded` with
//! the pull request and the merge commit; judged as a head's gate it is pending; with no
//! window declared the gate refuses with exit 12. The command line, HTTP and MCP answer the
//! same document, because all three are projections of the one capability declaration.

mod common;

use std::io::Write;
use std::process::Stdio;

use common::{run_in, without_observation_times, Fixture, Served};
use serde_json::{json, Value};

/// I0001 merged by pull request #5, whose title names it; no stamp records it.
fn fixture() -> (Fixture, String) {
    let f = Fixture::new();
    let trunk = f
        .git(&["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string();
    f.git(&["checkout", "-q", "-b", "feature/the-work"]);
    f.write("lib/work.sh", "echo work\n");
    f.commit("wip");
    f.git(&["checkout", "-q", &trunk]);
    f.git(&[
        "merge",
        "-q",
        "--no-ff",
        "feature/the-work",
        "-m",
        "Merge pull request #5 from someone/feature/the-work",
        "-m",
        "feat: the work (I0001)",
    ]);
    let merge = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    (f, merge)
}

fn model(f: &Fixture, window: u64, enforcement: &str) {
    f.write(
        ".ai/repo/ci/plan.yaml",
        &format!("version: 1\ndrift:\n  window: {window}\n  enforcement: {enforcement}\n"),
    );
}

fn cli(f: &Fixture, args: &[&str]) -> (i32, Value, String) {
    let mut argv = vec!["plan", "drift"];
    argv.extend_from_slice(args);
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(&f.root(), &argv, "");
    let v = serde_json::from_str(&out).unwrap_or(Value::Null);
    (code, v, err)
}

#[test]
fn a_ready_issue_merged_by_a_pull_request_is_merged_not_recorded_with_its_proof() {
    let (f, merge) = fixture();
    model(&f, 86_400, "blocking");

    let (code, v, err) = cli(&f, &[]);
    assert_eq!(code, 0, "a report exits 0: {err}");
    assert_eq!(v["verdict"], "report");
    let findings = v["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1, "{v}");
    let finding = &findings[0];
    assert_eq!(finding["issue"], "I0001");
    assert_eq!(finding["status"], "READY");
    assert_eq!(finding["class"], "merged_not_recorded");
    assert_eq!(finding["proof"]["pull_request"], 5);
    assert_eq!(finding["proof"]["merge_commit"], merge.as_str());
    assert_eq!(finding["past_window"], false, "merged a moment ago");
    assert!(finding["remedy"]
        .as_str()
        .unwrap()
        .contains("majordomus plan verify|done I0001"));
    assert_eq!(v["window"]["seconds"], 86_400);
    assert_eq!(v["tallies"]["merged_not_recorded"], 1);

    // on a head it is pending, and the trunk inside the window agrees
    for gate in ["head", "trunk"] {
        let (code, v, err) = cli(&f, &["--gate", gate]);
        assert_eq!(code, 0, "{gate}: {err}");
        assert_eq!(v["verdict"], "pending", "{gate}");
        assert_eq!(v["findings"][0]["verdict"], "pending", "{gate}");
    }

    // without a window nothing can be called young: the gate refuses to judge
    std::fs::remove_file(f.root().join(".ai/repo/ci/plan.yaml")).unwrap();
    let (code, v, _) = cli(&f, &["--gate", "head"]);
    assert_eq!(code, 12);
    assert_eq!(v["verdict"], "unjudged");
    assert_eq!(v["window"]["state"], "unread");

    // the text names the proof and the verdict
    model(&f, 86_400, "advisory");
    let (code, out, _) = run_in(&f.root(), &["plan", "drift", "--gate", "trunk"], "");
    assert_eq!(code, 0);
    assert!(
        out.contains("PENDING  I0001  READY    merged_not_recorded #5 "),
        "{out}"
    );
    assert!(out.contains("plan drift (trunk): pending"), "{out}");
}

#[test]
fn a_merged_pull_requests_forge_title_names_the_issue_its_git_history_does_not() {
    let f = Fixture::new();
    model(&f, 86_400, "advisory");
    let trunk = f
        .git(&["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string();
    f.git(&["checkout", "-q", "-b", "integration/top"]);
    f.write("lib/top.sh", "echo top\n");
    f.commit("chore: a member");
    let member = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    f.git(&["checkout", "-q", &trunk]);
    f.git(&[
        "merge",
        "-q",
        "--no-ff",
        "integration/top",
        "-m",
        "Merge branch 'integration/top'",
    ]);
    let top = f.git(&["rev-parse", "HEAD"]).trim().to_string();

    // the history alone names nothing
    let (_, v, _) = cli(&f, &[]);
    assert_eq!(v["findings"].as_array().map(Vec::len), Some(0), "{v}");
    let titles = v["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "merged_titles")
        .cloned()
        .unwrap();
    assert_eq!(
        titles["state"], "unread",
        "never observed is unread, not empty"
    );

    // the forge's title of #8, merged by that commit, names I0001
    let observation = json!({
        "schema": 1, "repository": "someone/fixture", "repository_url": "https://example.invalid/f",
        "observed_at": "2026-10-16T09:00:00Z", "deployments": [], "releases": [],
        "merged_pull_requests": [
            {"number": 8, "title": "feat: a piece merged in a top (I0001)", "head_ref": "feature/in-a-top",
             "head_sha": member, "merge_commit": top, "merged_at": "2026-10-16T08:00:00Z"},
            {"number": 4, "title": "feat: merged somewhere else (I0001)", "head_ref": "feature/elsewhere",
             "head_sha": "0000000000000000000000000000000000000000", "merge_commit": "", "merged_at": "2026-10-16T08:00:00Z"}
        ]
    });
    f.write(
        ".ai/local/state/integration/deployments.json",
        &serde_json::to_string_pretty(&observation).unwrap(),
    );
    let (_, v, _) = cli(&f, &[]);
    let finding = &v["findings"][0];
    assert_eq!(finding["class"], "merged_not_recorded", "{v}");
    assert_eq!(finding["proof"]["via"], "forge_title");
    assert_eq!(
        finding["proof"]["pull_request"], 8,
        "#4 merged nothing in this history"
    );
    assert_eq!(finding["proof"]["merge_commit"], top.as_str());
}

fn mcp_tool(f: &Fixture, name: &str, arguments: Value) -> Value {
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

#[test]
fn plan_drift_is_one_answer_on_the_command_line_http_and_mcp() {
    let (f, _) = fixture();
    model(&f, 86_400, "advisory");
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let (_, from_cli, _) = cli(&f, &["--at", &head]);
    let from_mcp = mcp_tool(&f, "majordomus_plan_drift", json!({ "at": head }));
    let mut served = Served::start(&f.root(), &[]);
    let (status, from_http) = served.get(&format!("/api/v1/plan/drift?at={head}"));
    served.stop();
    assert_eq!(status, 200, "{from_http}");
    // the mesh claims are a source whose reading depends on the process that answers: the
    // served answer may hold the mesh where the command line does not
    let norm = |v: &Value| {
        let mut v = v.clone();
        if let Some(s) = v["sources"].as_array_mut() {
            s.retain(|s| s["name"] != "claims");
        }
        without_observation_times(&v.to_string())
    };
    assert_eq!(norm(&from_cli), norm(&from_mcp));
    assert_eq!(norm(&from_cli), norm(&from_http));
    assert_eq!(from_cli["findings"][0]["class"], "merged_not_recorded");
}

#[test]
fn plan_drift_of_one_issue_as_a_heads_gate_and_what_it_refuses_to_judge() {
    let (f, merge) = fixture();
    model(&f, 86_400, "blocking");
    let (code, v, err) = cli(&f, &["I0001", "--gate", "head", "--at", &merge]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(v["gate"], "head");
    assert_eq!(v["verdict"], "pending", "{v}");
    assert_eq!(v["findings"].as_array().unwrap().len(), 1);

    let (code, _, err) = cli(&f, &["I9999"]);
    assert_ne!(code, 0);
    assert!(err.contains("I9999"), "{err}");
    let (code, _, err) = cli(&f, &["--at", "no-such-rev"]);
    assert_ne!(code, 0);
    assert!(err.contains("no-such-rev"), "{err}");
}

#[test]
fn plan_drift_reads_the_mesh_claims_where_the_mesh_runs() {
    let (f, _) = fixture();
    model(&f, 86_400, "advisory");
    let state = f.parent().join("xdg-state");
    let key = majordomus_cli::mesh::identity::NodeIdentity::load_or_create(
        &state.join("majordomus").join("node.json"),
    )
    .expect("a node identity")
    .public
    .public_key;
    f.write(
        ".ai/repo/mesh/majordomus.yaml",
        &format!(
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\n\
             multicast:\n  enabled: false\ntailscale:\n  enabled: false\ntrust:\n  policy: deny_unknown\n  allow:\n    - {key}\n\
             cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 30\n  repository: fixture\n"
        ),
    );
    let mut served = Served::start_with_env(
        &f.root(),
        &["--discovery", "filesystem"],
        &[("XDG_STATE_HOME", state.to_str().unwrap())],
    );
    let post = |path: &str, body: Value| -> (u16, Value) {
        let (status, _, text) = served.request("POST", path, Some(&body.to_string()));
        (status, serde_json::from_str(&text).unwrap_or(Value::Null))
    };
    let (status, opened) = post(
        "/api/v1/mesh/sessions",
        json!({"session": "s1", "client": "test", "intent": "the work", "issue": "I0001"}),
    );
    assert_eq!(status, 200, "{opened}");
    let (status, claimed) = post(
        "/api/v1/mesh/claims",
        json!({"session": "s1", "scope": ["lib"], "issue": "I0001", "intent": "the work"}),
    );
    assert_eq!(status, 200, "{claimed}");
    // a claim that names no issue names nothing the plan holds
    let (status, claimed) = post(
        "/api/v1/mesh/claims",
        json!({"session": "s1", "scope": ["docs"], "intent": "something else"}),
    );
    assert_eq!(status, 200, "{claimed}");
    let (status, v) = served.get("/api/v1/plan/drift");
    served.stop();
    assert_eq!(status, 200, "{v}");
    let claims = v["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "claims")
        .cloned()
        .unwrap();
    assert_eq!(claims["state"], "read", "{claims}");
    assert_eq!(claims["detail"], "1 issue(s) named by a live mesh claim");
}
