//! Intents through the built executable: the command line, HTTP and MCP answer the same
//! derivation; a recorded passing run is the only thing that moves a criterion to met; and an
//! intent model whose links do not resolve is refused, naming the finding.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::{run_in, Fixture, Served, BIN};
use majordomus_cli::evidence::{digest_of, Execution, Ledger, Origin, Outcome, Runner, TestId};
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

#[test]
fn the_command_line_http_and_mcp_answer_the_same_intents() {
    let f = Fixture::new();
    let (code, cli) = cli_json(&f, &["intent", "list"]);
    assert_eq!(code, 0);
    assert_eq!(cli["count"], 1);
    let intent = &cli["intents"][0];
    assert_eq!(intent["id"], "fixture-intent");
    assert_eq!(intent["milestones"][0]["id"], "fixture-milestone");
    assert_eq!(intent["milestones"][0]["resolved"], true);

    let mcp = tool(&f, "majordomus_intents", json!({}));
    let served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/intents");
    assert_eq!(status, 200);
    assert_eq!(cli, http, "the command line and HTTP disagree");
    assert_eq!(cli, mcp, "the command line and MCP disagree");

    let (status, record) = served.get("/api/v1/intents/record?id=fixture-intent");
    assert_eq!(status, 200);
    assert_eq!(
        &record, intent,
        "intent.record answers what intent.list lists"
    );
    let (status, _) = served.get("/api/v1/intents/record?id=absent");
    assert_eq!(status, 404);
}

#[test]
fn a_criterion_is_met_only_by_a_recorded_passing_run_of_the_test_that_is_there() {
    let f = Fixture::new();
    let criterion = |f: &Fixture| {
        let (_, v) = cli_json(f, &["intent", "show", "fixture-intent"]);
        v["satisfaction"][0].clone()
    };
    let before = criterion(&f);
    assert_eq!(before["state"], "not_run");
    assert_eq!(before["met"], false);
    assert_eq!(before["reproduce"], "bash test/run.sh 00_x");

    // a passing run, recorded against the source that is in the tree
    let id = TestId::of("test/cases/00_x.sh").unwrap();
    let source = std::fs::read(f.path(&id.source())).unwrap();
    let run = |outcome: Outcome, digest: String| Execution {
        test: id.as_string(),
        runner: Runner::Suite,
        source: id.source(),
        outcome,
        seconds: 1,
        commit: f.git(&["rev-parse", "HEAD"]).trim().to_string(),
        working_tree: "clean".into(),
        digest,
        at: "2026-09-15T00:00:00Z".into(),
        origin: Origin::Local,
        command: id.reproduce(),
    };
    let record = |e: Execution| {
        let mut ledger = Ledger::empty();
        ledger.merge([e]);
        ledger.save(&f.root()).unwrap();
        f.commit("record a run");
    };

    record(run(Outcome::Pass, digest_of(&source)));
    let after = criterion(&f);
    assert_eq!(after["state"], "current");
    assert_eq!(after["met"], true);

    // the same pass, over a test that has since changed, proves nothing about it
    record(run(Outcome::Pass, digest_of(b"another test")));
    assert_eq!(criterion(&f)["state"], "stale");

    record(run(Outcome::Fail, digest_of(&source)));
    assert_eq!(criterion(&f)["state"], "failing");
}

#[test]
fn an_intent_whose_links_do_not_resolve_is_refused_naming_each_finding() {
    let f = Fixture::new();
    let (code, out, _) = run_in(&f.root(), &["intent", "validate"], "");
    assert_eq!(code, 0, "the fixture's intent model is valid:\n{out}");

    f.write(
        ".ai/repo/project/intents/broken.yaml",
        "id: broken
title: Nothing realises this
statement: \"It becomes true.\"
invariants: []
milestones:
  - no-such-milestone
satisfaction:
  - id: nothing
    criterion: A test that is not there passes
    evidence: test
    ref: test/cases/404_absent.sh
",
    );
    f.commit("a broken intent");
    let (code, out, _) = run_in(&f.root(), &["intent", "validate"], "");
    assert_eq!(code, 10, "{out}");
    for fragment in [
        "unknown_milestone",
        "unresolved_evidence_ref",
        "broken",
        "invalid",
    ] {
        assert!(out.contains(fragment), "missing {fragment}:\n{out}");
    }

    let served = Served::start(&f.root(), &[]);
    let (_, http) = served.get("/api/v1/intents/validate");
    let mcp = tool(&f, "majordomus_intent_validate", json!({}));
    assert_eq!(http["valid"], false);
    assert_eq!(http, mcp);
    let (_, list) = served.get("/api/v1/intents");
    let broken = list["intents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == "broken")
        .unwrap();
    assert_eq!(broken["stage"], "declared");
}

#[test]
fn a_key_the_schema_does_not_declare_is_refused_by_the_index() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/project/intents/stored.yaml",
        &common::INTENT
            .replace("id: fixture-intent", "id: stored")
            .replace("title:", "stage: satisfied\ntitle:"),
    );
    f.commit("a stored stage");
    let (code, v, _) = common::inspect(&f.root(), &[]);
    assert_eq!(code, 10, "a refused record degrades the index");
    let hit = common::diagnostics(&v).iter().any(|d| {
        d["code"] == "unknown_key"
            && d["path"] == ".ai/repo/project/intents/stored.yaml"
            && d["message"].as_str().is_some_and(|m| m.contains("stage"))
    });
    assert!(hit, "{:#?}", common::diagnostics(&v));
}

#[test]
fn preflight_names_the_intent_the_work_serves_or_the_missing_link() {
    let f = Fixture::new();
    let (code, v) = cli_json(&f, &["intent", "preflight", "--issue", "I0001"]);
    assert_eq!(code, 0);
    assert_eq!(v["verdict"], "serves");
    assert_eq!(v["matches"][0]["intent"], "fixture-intent");
    assert_eq!(v["matches"][0]["milestone"], "fixture-milestone");
    assert_eq!(v["governance"], json!(["rule:project.alpha"]));
    let mcp = tool(
        &f,
        "majordomus_intent_preflight",
        json!({ "issue": "I0001" }),
    );
    assert_eq!(v, mcp);

    let (code, v) = cli_json(&f, &["intent", "preflight", "--issue", "I9999"]);
    assert_eq!(code, 10);
    assert!(v["refusal"].as_str().unwrap().contains("I9999"));

    // a milestone no intent names is the missing link
    f.remove(".ai/repo/project/intents/fixture-intent.yaml");
    f.commit("no intent");
    let (code, v) = cli_json(&f, &["intent", "preflight", "--issue", "I0001"]);
    assert_eq!(code, 10);
    assert!(v["refusal"].as_str().unwrap().contains("fixture-milestone"));

    let (code, _, err) = run_in(&f.root(), &["intent", "preflight"], "");
    assert_ne!(code, 0, "a preflight of nothing is refused");
    assert!(err.contains("name the issue"), "{err}");
}
