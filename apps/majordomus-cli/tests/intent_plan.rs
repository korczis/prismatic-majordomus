//! The plan read against the intents, through the built executable: which work carries which
//! criterion, why every issue exists, and the refusals that keep a plan honest — a criterion
//! nothing serves, work with no purpose under an intent, a gap that skips a criterion, and
//! execution that started before the plan was critiqued.

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

fn cli_json(f: &Fixture, args: &[&str]) -> (i32, Value) {
    let mut argv = args.to_vec();
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(&f.root(), &argv, "");
    let v = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("{args:?} did not print JSON ({e}):\n{out}\n{err}"));
    (code, v)
}

/// The codes of `intent validate`, with the subject each is about.
fn findings(f: &Fixture) -> Vec<(String, String)> {
    let (_, v) = cli_json(f, &["intent", "validate"]);
    v["findings"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|x| {
            (
                x["code"].as_str().unwrap_or("").to_string(),
                x["subject"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

fn has(f: &Fixture, code: &str, subject: &str) -> bool {
    findings(f).iter().any(|(c, s)| c == code && s == subject)
}

/// An issue under the fixture's milestone, serving what it is told to serve.
fn issue(f: &Fixture, id: &str, serves: &str) {
    f.write(
        &format!(".ai/repo/project/issues/{id}.yaml"),
        &format!(
            "id: {id}
milestone: fixture-milestone
title: Another bounded piece of work
slug: issue-{id}
priority: p1
profile: implementation
objective: \"Do another bounded piece of work.\"
scope:
  - other
{serves}
acceptance_criteria:
  - The work is done
validation:
  - \"true\"
evidence_required:
  - proof
"
        ),
    );
}

#[test]
fn the_command_line_http_and_mcp_answer_the_same_coverage() {
    let f = Fixture::new();
    let (code, cli) = cli_json(&f, &["intent", "coverage"]);
    assert_eq!(code, 0);

    let criterion = &cli["criteria"][0];
    assert_eq!(criterion["intent"], "fixture-intent");
    assert_eq!(criterion["criterion"], "the-case-passes");
    assert_eq!(criterion["strength"], "covered");
    assert_eq!(criterion["issues"][0]["id"], "I0001");
    assert_eq!(criterion["milestones"], json!(["fixture-milestone"]));

    let purpose = &cli["issues"][0];
    assert_eq!(purpose["issue"], "I0001");
    assert_eq!(purpose["origin"], "intent");
    assert_eq!(purpose["serves"], json!(["fixture-intent#the-case-passes"]));

    let mcp = tool(&f, "majordomus_intent_coverage", json!({}));
    let served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/intents/coverage");
    assert_eq!(status, 200);
    assert_eq!(cli, http, "the command line and HTTP disagree");
    assert_eq!(cli, mcp, "the command line and MCP disagree");
}

#[test]
fn a_criterion_no_work_serves_is_refused_once_the_intent_has_work() {
    let f = Fixture::new();
    let (code, _, _) = run_in(&f.root(), &["intent", "validate"], "");
    assert_eq!(code, 0, "the fixture's plan carries its intent");

    // a second criterion nothing serves, while the intent already has work
    f.write(
        ".ai/repo/project/intents/fixture-intent.yaml",
        &common::INTENT.replace(
            "governance:",
            "  - id: unserved
    criterion: Nothing will make this true
    evidence: test
    ref: test/cases/00_x.sh
governance:",
        ),
    );
    f.commit("a criterion nothing serves");
    let (code, out, _) = run_in(&f.root(), &["intent", "validate"], "");
    assert_eq!(code, 10, "{out}");
    assert!(
        has(&f, "criterion_uncovered", "fixture-intent#unserved"),
        "{:?}",
        findings(&f)
    );

    let (_, cov) = cli_json(&f, &["intent", "coverage"]);
    let unserved = cov["criteria"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["criterion"] == "unserved")
        .unwrap();
    assert_eq!(unserved["strength"], "uncovered");
    assert_eq!(unserved["issues"], json!([]));
}

#[test]
fn work_under_an_intent_answers_why_it_exists_or_is_refused() {
    let f = Fixture::new();
    issue(&f, "I0002", "serves: []");
    f.commit("work with no purpose");
    assert!(
        has(&f, "issue_without_purpose", "I0002"),
        "{:?}",
        findings(&f)
    );

    issue(&f, "I0002", "serves:\n  - fixture-intent#nope");
    f.commit("work serving nothing that exists");
    assert!(
        has(&f, "serves_unknown_criterion", "I0002"),
        "{:?}",
        findings(&f)
    );

    // maintenance under a milestone no intent names is neither refused nor given an intent
    f.write(
        ".ai/repo/project/milestones/ops.yaml",
        "id: ops
title: The machine keeps running
slug: ops
order: 1
priority: p1
problem: \"Things need maintaining.\"
outcome: \"They are maintained.\"
acceptance_criteria:
  - It is maintained
validation:
  - \"true\"
evidence_required: []
",
    );
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &std::fs::read_to_string(f.path(".ai/repo/project/issues/I0002.yaml"))
            .unwrap()
            .replace("milestone: fixture-milestone", "milestone: ops")
            .replace("serves:\n  - fixture-intent#nope\n", ""),
    );
    f.commit("maintenance");
    let (code, out, _) = run_in(&f.root(), &["intent", "validate"], "");
    assert_eq!(code, 0, "maintenance is not a finding:\n{out}");
    let (_, cov) = cli_json(&f, &["intent", "coverage"]);
    let purpose = cov["issues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["issue"] == "I0002")
        .unwrap();
    assert_eq!(purpose["origin"], "maintenance");
}

#[test]
fn a_gap_must_answer_every_criterion_and_a_plan_is_critiqued_before_it_runs() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/project/gaps/fixture-intent.yaml",
        "intent: fixture-intent
observed_at: HEAD
recorded_by: the test
recorded_at: 2026-09-16
observations:
  - id: seen
    statement: The case is not there yet
    source: file:test/cases
conditions: []
",
    );
    f.commit("a gap that answers nothing");
    assert!(
        has(
            &f,
            "gap_criterion_unanswered",
            "fixture-intent#the-case-passes"
        ),
        "{:?}",
        findings(&f)
    );

    f.write(
        ".ai/repo/project/gaps/fixture-intent.yaml",
        "intent: fixture-intent
observed_at: HEAD
recorded_by: the test
recorded_at: 2026-09-16
observations:
  - id: seen
    statement: The case is not there yet
    source: file:test/cases
conditions:
  - criterion: the-case-passes
    state: missing
    because: nothing runs it
    observations: [seen]
",
    );
    f.commit("a complete gap");
    let (code, out, _) = run_in(&f.root(), &["intent", "validate"], "");
    assert_eq!(code, 0, "{out}");

    // the work starts, and the plan was never critiqued
    f.write(
        ".ai/repo/project/issues/I0001.yaml",
        &std::fs::read_to_string(f.path(".ai/repo/project/issues/I0001.yaml"))
            .unwrap()
            .replace(
                "evidence_required:",
                "started_at: 2026-09-16\nevidence_required:",
            ),
    );
    f.commit("start the work");
    assert!(
        has(&f, "executing_without_critique", "fixture-intent"),
        "{:?}",
        findings(&f)
    );

    let critique = |resolution: &str| {
        format!(
            "intent: fixture-intent
reviewed_at: HEAD
reviewed_by: the test
findings:
  - id: thin
    class: insufficient_work
    subject: fixture-intent#the-case-passes
    finding: One case may not be enough
    blocking: true
    resolution:
{resolution}
"
        )
    };
    f.write(
        ".ai/repo/project/critiques/fixture-intent.yaml",
        &critique("      state: open"),
    );
    f.commit("an open blocker");
    assert!(
        has(&f, "executing_with_open_blocker", "fixture-intent"),
        "{:?}",
        findings(&f)
    );

    f.write(
        ".ai/repo/project/critiques/fixture-intent.yaml",
        &critique("      state: planned\n      issue: I0001"),
    );
    f.commit("resolve the blocker");
    let (code, out, _) = run_in(&f.root(), &["intent", "validate"], "");
    assert_eq!(code, 0, "a resolved critique lets the work run:\n{out}");
}
