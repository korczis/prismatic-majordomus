//! Intent realization through the built executable: one task carried by two providers across a
//! handover realises one intent, with every link's provenance, and the command line, HTTP and MCP
//! answer the same join. The ledger lines are the ones the shell lifecycle writes; case 388 drives
//! the lifecycle itself.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::{run_in, Fixture, Served, BIN};
use serde_json::{json, Value};

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

fn cli_json(f: &Fixture, args: &[&str]) -> (i32, Value) {
    let mut argv = args.to_vec();
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(&f.root(), &argv, "");
    let v = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("{args:?} did not print JSON ({e}):\n{out}\n{err}"));
    (code, v)
}

/// The lines the lifecycle appends when a Claude Code episode starts task t-1 and hands it over,
/// and a Codex episode resumes it and starts the issue.
fn two_provider_ledger(branch: &str, moved: bool) -> String {
    let env = |ts: &str, ev: &str, session: &str| {
        format!(
            r#""ts":"{ts}","event":"{ev}","head":"abc","branch":"{branch}","by":"majordomus/0.7.0","session":"{session}""#
        )
    };
    let mut lines = vec![
        format!(
            r#"{{{},"owner":"a","provider":"claude-code","provider_session":"win-a"}}"#,
            env("1", "session.started", "s-a")
        ),
        format!(
            r#"{{{},"task_id":"t-1","scope":"lib"}}"#,
            env("2", "task.started", "s-a")
        ),
        format!(
            r#"{{{},"task_id":"t-1","handover_path":".ai/local/state/handovers/h.md"}}"#,
            env("3", "task.handed_over", "s-a")
        ),
        format!(
            r#"{{{},"outcome":"closed"}}"#,
            env("4", "session.closed", "s-a")
        ),
        format!(
            r#"{{{},"owner":"b","provider":"codex","provider_session":"win-b"}}"#,
            env("5", "session.started", "s-b")
        ),
        format!(
            r#"{{{},"task_id":"t-1"}}"#,
            env("6", "task.checkpoint", "s-b")
        ),
    ];
    if moved {
        lines.push(format!(
            r#"{{{},"issue":"I0001"}}"#,
            env("7", "plan_start", "s-b")
        ));
    }
    lines.join("\n") + "\n"
}

#[test]
fn one_task_carried_by_two_providers_across_a_handover_realises_one_intent() {
    let f = Fixture::new();
    f.write(
        ".ai/local/state/ledger.jsonl",
        &two_provider_ledger("master", true),
    );

    let (code, v) = cli_json(&f, &["intent", "realization"]);
    assert_eq!(code, 0, "{v:#}");
    let work = v["work"].as_array().unwrap();
    let task = work
        .iter()
        .find(|w| w["work"]["id"] == "t-1")
        .expect("the task is joined");
    assert_eq!(task["work"]["kind"], "task");
    let providers: Vec<&str> = task["work"]["episodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["provider"].as_str().unwrap())
        .collect();
    assert_eq!(
        providers,
        ["claude-code", "codex"],
        "both episodes, in order, each with its provider"
    );
    assert_eq!(
        task["work"]["handovers"],
        json!([".ai/local/state/handovers/h.md"])
    );
    assert_eq!(task["work"]["moved_issues"], json!(["I0001"]));

    // the Codex episode moved the issue, so the link is observed, not inferred from the scope
    let link = &task["links"][0];
    assert_eq!(link["intent"], "fixture-intent");
    assert_eq!(link["issue"], "I0001");
    assert_eq!(link["criteria"], json!(["the-case-passes"]));
    assert_eq!(link["via"], "moved_issue");
    assert_eq!(link["provenance"], "observed");

    let intent = &v["intents"][0];
    assert_eq!(intent["intent"], "fixture-intent");
    assert_eq!(intent["providers"], json!(["claude-code", "codex"]));
    assert_eq!(intent["work"][0]["id"], "t-1");
    assert_eq!(intent["work"][0]["handovers"], 1);
    assert_eq!(intent["unmet"][0]["id"], "the-case-passes");
    assert_eq!(intent["unmet"][0]["state"], "not_run");
    assert_eq!(intent["unmet"][0]["issues"], json!(["I0001"]));

    let served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/intents/realization");
    assert_eq!(status, 200);
    assert_eq!(v, http, "the command line and HTTP disagree");
    let mcp = tool(&f, "majordomus_intent_realization", json!({}));
    assert_eq!(v, mcp, "the command line and MCP disagree");

    let (status, explained) = served.get("/api/v1/intents/explain?id=fixture-intent");
    assert_eq!(status, 200);
    let because: Vec<&str> = explained["because"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_str().unwrap())
        .collect();
    assert!(because[0].starts_with("planned"), "{because:#?}");
    assert!(
        because
            .iter()
            .any(|b| b.contains("`the-case-passes` is not met") && b.contains("no recorded run")),
        "{because:#?}"
    );
    assert!(
        because
            .iter()
            .any(|b| b.contains("across 1 handover(s), run by claude-code, codex")),
        "{because:#?}"
    );
    assert_eq!(
        explained,
        tool(
            &f,
            "majordomus_intent_explain",
            json!({"id": "fixture-intent"})
        )
    );
    let (status, _) = served.get("/api/v1/intents/explain?id=absent");
    assert_eq!(status, 404);
}

#[test]
fn each_link_says_how_it_is_known_and_the_strongest_wins() {
    let f = Fixture::new();
    // no plan transition and a branch that names nothing: only the scope overlaps
    f.write(
        ".ai/local/state/ledger.jsonl",
        &two_provider_ledger("master", false),
    );
    let (_, v) = cli_json(&f, &["intent", "realization", "--intent", "fixture-intent"]);
    assert_eq!(v["work"][0]["links"][0]["provenance"], "inferred");
    assert_eq!(v["work"][0]["links"][0]["via"], "scope_overlap");

    // the branch names the issue: derived beats inferred, and replaces it
    f.write(
        ".ai/local/state/ledger.jsonl",
        &two_provider_ledger("feature/I0001-work", false),
    );
    let (_, v) = cli_json(&f, &["intent", "realization", "--intent", "fixture-intent"]);
    let links = v["work"][0]["links"].as_array().unwrap();
    assert_eq!(links.len(), 1, "one link per issue: {links:#?}");
    assert_eq!(links[0]["provenance"], "derived");

    // the task's own words cite the issue: declared
    f.write(
        ".ai/local/state/archive/t-1.yaml",
        "id: t-1\ntask: \"finish I0001\"\nscope:\n  - lib\n",
    );
    f.write(".ai/local/state/current.yaml", "id: t-2\n");
    let (_, v) = cli_json(&f, &["intent", "realization", "--intent", "fixture-intent"]);
    let t1 = v["work"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["work"]["id"] == "t-1")
        .unwrap();
    assert_eq!(t1["links"][0]["provenance"], "declared");
    assert_eq!(t1["work"]["title"], "finish I0001");

    // an intent that is not there is refused, never an empty answer
    let (code, _, err) = run_in(
        &f.root(),
        &["intent", "realization", "--intent", "absent"],
        "",
    );
    assert_ne!(code, 0);
    assert!(err.contains("no intent 'absent'"), "{err}");
}

#[test]
fn live_work_that_serves_no_intent_is_named_with_the_missing_link() {
    let f = Fixture::new();
    f.write(
        ".ai/local/state/ledger.jsonl",
        &two_provider_ledger("master", false),
    );
    f.remove(".ai/repo/project/intents/fixture-intent.yaml");
    f.commit("no intent");
    let (code, v) = cli_json(&f, &["intent", "realization"]);
    assert_eq!(code, 0, "an orphan is a warning, not a refusal");
    assert_eq!(v["orphans"], 1);
    let finding = &v["findings"][0];
    assert_eq!(finding["code"], "work_serves_no_intent");
    assert_eq!(finding["subject"], "t-1");
    assert!(
        finding["message"]
            .as_str()
            .unwrap()
            .contains("milestones no intent names: fixture-milestone (I0001)"),
        "{finding:#}"
    );
}

/// The Cockpit's intent page is a projection of `intent_realization.explain`: each criterion
/// links to the test object it names and to the issue serving it, the work realising the intent
/// carries its providers and the provenance of its link, and an unknown intent is a 404.
#[test]
fn the_cockpit_links_each_criterion_to_its_test_and_the_issue_serving_it() {
    let f = Fixture::new();
    f.write(
        ".ai/local/state/ledger.jsonl",
        &two_provider_ledger("master", true),
    );
    let served = Served::start(&f.root(), &[]);
    let page = |target: &str| {
        let (status, _, body) = served.request("GET", target, None);
        (status, body)
    };

    let (status, list) = page("/cockpit/intents");
    assert_eq!(status, 200, "{list}");
    assert!(
        list.contains(r#"href="/cockpit/intents/fixture-intent""#),
        "{list}"
    );
    assert!(list.contains("claude-code, codex"), "{list}");

    let (status, body) = page("/cockpit/intents/fixture-intent");
    assert_eq!(status, 200, "{body}");
    let test_link = "/cockpit/object?uri=majordomus%3A%2F%2Ftest%2Ftest%2Fcases%2F00_x.sh";
    assert!(
        body.contains(test_link),
        "the criterion does not link its test:\n{body}"
    );
    assert!(
        body.contains("/cockpit/object?uri=majordomus%3A%2F%2Fissue%2FI0001"),
        "the criterion does not link the issue serving it:\n{body}"
    );
    for fragment in [
        "not run",
        "bash test/run.sh 00_x",
        "observed",
        "codex",
        "t-1",
    ] {
        assert!(body.contains(fragment), "missing {fragment}:\n{body}");
    }
    // the linked test is a page the Cockpit actually serves
    let (status, _) = page(test_link);
    assert_eq!(status, 200);

    let (status, _) = page("/cockpit/intents/absent");
    assert_eq!(status, 404);
}
