//! Evidence answers by subject, on every surface (I1953, ADR 0087 decision 3).
//!
//! `evidence.subject` judges any subject of the index — a claim, a rule, a feature, a
//! command, a capability, an MCP tool, a use case — and `evidence.coverage` lists the
//! coverage the run records carry. Each is one declaration projected three times, so the
//! command line, HTTP and an MCP `tools/call` must give the same document for the same
//! question, about a real repository with a recorded run in it; an unknown subject is
//! refused on each of them, with exit 13 on the command line.
//!
//! `evidence.runs` and `evidence.run` show the local run index the same way on the three
//! surfaces, and an unknown run is refused on each. The run records an answer by subject
//! reads beside its verdict are supplementary: they only ever weaken it, through the
//! monotone rule, and each weakening names its record (ADR 0087, decision 2).

mod common;

use std::io::Write;
use std::process::Stdio;

use common::{run_in, Fixture, Served};
use serde_json::{json, Value};

/// A use case the fixture adds, so that every kind of the index has a subject: it runs a
/// command, exercises the fixture's rule, evidences its claim and calls the subject tool.
const USE_CASE: &str = "---
id: see-policy
kind: use-case
title: 'See the policy claim'
summary: 'Ask what proves the policy claim.'
category: adoption
status: active
target: guaranteed
actors: [operator]
difficulty: basic
commands: [version]
doctrines: [project.alpha]
claims: [policy-parse]
mcp_tools: [majordomus_evidence_subject]
responsibilities: []
applications: []
---

# Situation

Is the policy claim proven?

# Scenario

```yaml
setup: bare
given:
  - 'nothing installed'
steps:
  - id: print
    run: ['version']
    note: 'the version string'
    expect:
      exit: 0
then:
  - 'the version needs no repository'
```

# Outcome

The claim, from its subject.
";

/// One subject of each kind the index has, in the fixture below.
const SUBJECTS: [&str; 7] = [
    "claim:policy-parse",
    "rule:project.alpha",
    "feature:fixture-feature",
    "command:evidence subject",
    "capability:evidence.subject",
    "mcp:majordomus_evidence_subject",
    "use_case:see-policy",
];

/// The fixture with a use case, and a run of its one case recorded, measured and committed,
/// so that the verdicts are judged over a ledger that holds something.
fn recorded() -> (Fixture, String) {
    recorded_as("ok")
}

/// [`recorded`], with the case's run recorded as `outcome` (a TSV outcome word).
fn recorded_as(outcome: &str) -> (Fixture, String) {
    let f = Fixture::new();
    // the shared fixture's source classes index no use case; the class is the one the
    // installed skeleton declares, so the use case reaches the index as `init` would put it
    f.write(
        ".ai/repo/knowledge/sources.yaml",
        &format!(
            "{}\n  - id: use_case\n    kind: use-case\n    discovery: vcs\n    \
             pathspec: ':(glob).ai/repo/use-cases/*.md'\n    required: false\n",
            common::SOURCES.trim_end()
        ),
    );
    f.write(".ai/repo/use-cases/see-policy.md", USE_CASE);
    f.commit("a use case");
    let reports = tempfile::tempdir().unwrap();
    let tsv = reports.path().join("run.tsv");
    std::fs::write(&tsv, format!("00_x\t{outcome}\t1\tparallel\n")).unwrap();
    let stamp = reports.path().join("stamp.json");
    let (tsv, stamp) = (tsv.display().to_string(), stamp.display().to_string());
    let (code, _, err) = run_in(
        &f.root(),
        &[
            "evidence",
            "stamp",
            "--producer",
            "suite",
            "--report",
            &tsv,
            "--out",
            &stamp,
        ],
        "",
    );
    assert_eq!(code, 0, "{err}");
    let provenance = format!("suite={stamp}");
    let (code, _, err) = run_in(
        &f.root(),
        &[
            "evidence",
            "record",
            "--suite",
            &tsv,
            "--provenance",
            &provenance,
        ],
        "",
    );
    assert_eq!(code, 0, "{err}");
    f.commit("the recorded run");
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    (f, head)
}

/// One MCP `tools/call` over stdio: the whole result frame, so that an error is visible.
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
    frames[1]["result"].clone()
}

/// A query value as a URL carries it: the space of a command's words and the colon of a key.
fn query(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace(':', "%3A")
}

/// The command line's JSON answer, or the exit code and standard error it refused with.
fn cli(f: &Fixture, args: &[&str]) -> Result<Value, (i32, String)> {
    let mut argv = args.to_vec();
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(&f.root(), &argv, "");
    match code {
        0 => Ok(serde_json::from_str(&out).unwrap_or_else(|e| panic!("{e}: {out}"))),
        _ => Err((code, err)),
    }
}

#[test]
fn every_subject_kind_answers_the_same_verdict_on_every_surface() {
    let (f, _) = recorded();
    let s = Served::start(&f.root(), &[]);
    for key in SUBJECTS {
        let by_cli = cli(&f, &["evidence", "subject", key]).unwrap();
        assert_eq!(by_cli["subject"], key, "{by_cli}");
        assert_eq!(by_cli["presented"]["revision"], "working_tree");
        assert!(by_cli["verdict"].is_string() && by_cli["totals"].is_object());

        let (status, by_http) = s.get(&format!("/api/v1/evidence/subject?subject={}", query(key)));
        assert_eq!(status, 200, "{key}: {by_http}");
        assert_eq!(by_http, by_cli, "{key}: HTTP and the command line disagree");

        let by_mcp = mcp_call(&f, "majordomus_evidence_subject", json!({ "subject": key }));
        // a successful call carries `isError: false`; only a refusal carries `true`
        assert_ne!(by_mcp["isError"], json!(true), "{key}: {by_mcp}");
        assert_eq!(
            by_mcp["structuredContent"], by_cli,
            "{key}: MCP and the command line disagree"
        );
    }

    // the recorded run reaches the claim's route, and through it the feature that names it
    let claim = cli(&f, &["evidence", "subject", "claim:policy-parse"]).unwrap();
    assert_eq!(claim["routes"][0]["test"], "suite:00_x", "{claim}");
    assert_eq!(
        claim["routes"][0]["execution"]["outcome"], "pass",
        "{claim}"
    );
    let feature = cli(&f, &["evidence", "subject", "feature:fixture-feature"]).unwrap();
    let members: Vec<&str> = feature["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["subject"].as_str().unwrap())
        .collect();
    assert!(
        members.contains(&"claim:policy-parse") && members.contains(&"rule:project.alpha"),
        "{feature}"
    );
    let member = feature["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["subject"] == "claim:policy-parse")
        .unwrap();
    assert_eq!(
        member["state"], claim["verdict"],
        "a member reads its own verdict"
    );

    // at the checked-out commit, as committed, the three still agree
    let at = cli(
        &f,
        &[
            "evidence",
            "subject",
            "rule:project.alpha",
            "--presented",
            "HEAD",
        ],
    )
    .unwrap();
    let (status, by_http) =
        s.get("/api/v1/evidence/subject?subject=rule%3Aproject.alpha&presented=HEAD");
    assert_eq!(status, 200, "{by_http}");
    assert_eq!(by_http, at);
    let by_mcp = mcp_call(
        &f,
        "majordomus_evidence_subject",
        json!({ "subject": "rule:project.alpha", "presented": "HEAD" }),
    );
    assert_eq!(by_mcp["structuredContent"], at);
}

#[test]
fn an_unknown_subject_is_refused_on_every_surface() {
    let (f, _) = recorded();
    let (code, err) = cli(&f, &["evidence", "subject", "feature:no-such"]).unwrap_err();
    assert_eq!(code, 13, "{err}");
    assert!(err.contains("`feature:no-such` is not a subject"), "{err}");
    assert!(
        err.contains("site/data/registry/evidence-subjects.json"),
        "the refusal does not say where the subjects are listed: {err}"
    );

    let s = Served::start(&f.root(), &[]);
    let (status, body) = s.get("/api/v1/evidence/subject?subject=feature%3Ano-such");
    assert_eq!(status, 404, "{body}");
    assert_eq!(body["error"]["code"], "not_found", "{body}");

    let by_mcp = mcp_call(
        &f,
        "majordomus_evidence_subject",
        json!({ "subject": "feature:no-such" }),
    );
    assert_eq!(by_mcp["isError"], true, "{by_mcp}");
    assert!(by_mcp.to_string().contains("feature:no-such"), "{by_mcp}");
}

#[test]
fn coverage_answers_the_same_on_every_surface_and_is_current_only_at_its_commit() {
    let (f, head) = recorded();
    let one = json!({ "covered": 9, "total": 10, "percent": 90.0 });
    let dims = json!({ "lines": one, "functions": one, "regions": one });
    let run = |id: &str, commit: &str| {
        json!({
            "schema": 1, "id": id, "origin": "local", "commit": commit,
            "working_tree": "clean", "recorded_at": "2026-10-22T12:00:00Z", "ledger": "local",
            "coverage": {
                "commit": commit, "working_tree": "clean",
                "measurement": "scripts/rust-coverage", "test_code": "excluded",
                "crate": dims,
                "domain": { "lines": one, "functions": one, "regions": one,
                            "files": [], "missing": [] },
                "files": {}
            }
        })
    };
    let other = "e".repeat(40);
    f.write(
        ".ai/local/evidence/runs/at-head.json",
        &run("r-head", &head).to_string(),
    );
    f.write(
        ".ai/local/evidence/runs/elsewhere.json",
        &run("r-other", &other).to_string(),
    );
    assert_eq!(
        f.git(&["status", "--porcelain"]),
        "",
        "the local runs dirtied the tree"
    );

    let by_cli = cli(&f, &["evidence", "coverage"]).unwrap();
    assert_eq!(by_cli["presented"]["commit"], head.as_str(), "{by_cli}");
    let states: Vec<(&str, &str)> = by_cli["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["run"].as_str().unwrap(), r["state"].as_str().unwrap()))
        .collect();
    assert_eq!(states, [("r-head", "current"), ("r-other", "stale")]);

    let s = Served::start(&f.root(), &[]);
    let (status, by_http) = s.get("/api/v1/evidence/coverage");
    assert_eq!(status, 200, "{by_http}");
    assert_eq!(by_http, by_cli, "HTTP and the command line disagree");
    let by_mcp = mcp_call(&f, "majordomus_evidence_coverage", json!({}));
    assert_eq!(
        by_mcp["structuredContent"], by_cli,
        "MCP and the command line disagree"
    );

    // a tree the caller knows was dirty leaves nothing current, on every surface
    let dirty = cli(&f, &["evidence", "coverage", "--presented-tree", "dirty"]).unwrap();
    assert_eq!(dirty["records"][0]["state"], "stale", "{dirty}");
    let (_, by_http) = s.get("/api/v1/evidence/coverage?presented_tree=dirty");
    assert_eq!(by_http, dirty);

    // a source outside the checkout is refused
    let (status, body) = s.get("/api/v1/evidence/coverage?from=..");
    assert_eq!(status, 422, "{body}");
}

/// Record one run of the fixture's case as a local run, through the real stamp and recorder:
/// the run lands in the local run index under `.ai/local/`, and the tree stays clean. Returns
/// the run's id.
fn local_run(f: &Fixture, outcome: &str) -> String {
    let reports = tempfile::tempdir().unwrap();
    let tsv = reports.path().join("run.tsv");
    std::fs::write(&tsv, format!("00_x\t{outcome}\t1\tparallel\n")).unwrap();
    let stamp = reports.path().join("stamp.json");
    let (tsv, stamp) = (tsv.display().to_string(), stamp.display().to_string());
    let (code, _, err) = run_in(
        &f.root(),
        &[
            "evidence",
            "stamp",
            "--producer",
            "suite",
            "--report",
            &tsv,
            "--out",
            &stamp,
        ],
        "",
    );
    assert_eq!(code, 0, "{err}");
    let provenance = format!("suite={stamp}");
    let recorded = cli(
        f,
        &[
            "evidence",
            "record",
            "--suite",
            &tsv,
            "--provenance",
            &provenance,
            "--ledger",
            "local",
        ],
    )
    .unwrap_or_else(|(code, err)| panic!("{code}: {err}"));
    assert!(
        recorded["local_run"].is_string(),
        "a local recording did not reach the local run index: {recorded}"
    );
    assert_eq!(
        f.git(&["status", "--porcelain"]),
        "",
        "a local run dirtied the tree"
    );
    recorded["run_record"]["id"].as_str().unwrap().to_string()
}

#[test]
fn runs_and_a_run_answer_the_same_on_every_surface() {
    let (f, head) = recorded();
    let empty = cli(&f, &["evidence", "runs"]).unwrap();
    assert_eq!(empty["source"], ".ai/local/evidence/runs", "{empty}");
    assert_eq!(empty["runs"], json!([]), "{empty}");

    let failed = local_run(&f, "fail");
    let runs = cli(&f, &["evidence", "runs"]).unwrap();
    assert_eq!(runs["present"], true, "{runs}");
    let listed = &runs["runs"][0];
    assert_eq!(listed["id"], failed.as_str(), "{runs}");
    assert_eq!(listed["commit"], head.as_str(), "{runs}");
    assert_eq!(listed["working_tree"], "clean", "{runs}");
    assert_eq!(listed["outcome"], "fail", "{runs}");
    assert_eq!(listed["totals"]["executions"], 1, "{runs}");

    let s = Served::start(&f.root(), &[]);
    let (status, by_http) = s.get("/api/v1/evidence/runs");
    assert_eq!(status, 200, "{by_http}");
    assert_eq!(by_http, runs, "HTTP and the command line disagree");
    let by_mcp = mcp_call(&f, "majordomus_evidence_runs", json!({}));
    assert_ne!(by_mcp["isError"], json!(true), "{by_mcp}");
    assert_eq!(
        by_mcp["structuredContent"], runs,
        "MCP and the command line disagree"
    );

    // one run, by its id and by its file, with its cases and its record
    let one = cli(&f, &["evidence", "run", &failed]).unwrap();
    assert_eq!(
        one["run"], *listed,
        "the run is not summarised as the list shows it"
    );
    assert_eq!(one["record"]["id"], failed.as_str(), "{one}");
    assert!(one["cases"].is_array(), "{one}");
    assert_eq!(one["record"]["executions"][0]["outcome"], "fail", "{one}");
    let file = listed["file"].as_str().unwrap();
    let by_file = cli(&f, &["evidence", "run", file]).unwrap();
    assert_eq!(by_file, one, "a run named by its file is not the same run");

    let (status, by_http) = s.get(&format!("/api/v1/evidence/run?id={}", query(&failed)));
    assert_eq!(status, 200, "{by_http}");
    assert_eq!(by_http, one, "HTTP and the command line disagree");
    let by_mcp = mcp_call(&f, "majordomus_evidence_run", json!({ "id": failed }));
    assert_ne!(by_mcp["isError"], json!(true), "{by_mcp}");
    assert_eq!(
        by_mcp["structuredContent"], one,
        "MCP and the command line disagree"
    );

    // a source outside the checkout is refused
    let (status, body) = s.get("/api/v1/evidence/runs?from=..");
    assert_eq!(status, 422, "{body}");
}

#[test]
fn an_unknown_run_is_refused_on_every_surface() {
    let (f, _) = recorded();
    local_run(&f, "ok");
    let unknown = "local:000000000000:20260101T000000Z";
    let (code, err) = cli(&f, &["evidence", "run", unknown]).unwrap_err();
    assert_eq!(code, 13, "{err}");
    assert!(err.contains(&format!("`{unknown}` is not a run")), "{err}");
    assert!(
        err.contains("majordomus evidence runs"),
        "the refusal does not say where the runs are listed: {err}"
    );

    let s = Served::start(&f.root(), &[]);
    let (status, body) = s.get(&format!("/api/v1/evidence/run?id={}", query(unknown)));
    assert_eq!(status, 404, "{body}");
    assert_eq!(body["error"]["code"], "not_found", "{body}");

    let by_mcp = mcp_call(&f, "majordomus_evidence_run", json!({ "id": unknown }));
    assert_eq!(by_mcp["isError"], true, "{by_mcp}");
    assert!(by_mcp.to_string().contains(unknown), "{by_mcp}");
}

/// The verdict a subject answers with, its route on the fixture's test, and its
/// supplementary section, on the command line.
fn judged(f: &Fixture, key: &str) -> (String, Value, Value) {
    let v = cli(f, &["evidence", "subject", key]).unwrap();
    let route = v["routes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["test"] == "suite:00_x")
        .cloned()
        .unwrap_or(Value::Null);
    (
        v["verdict"].as_str().unwrap().to_string(),
        route,
        v["supplementary"].clone(),
    )
}

/// A supplementary record only ever weakens a verdict, and each weakening names its record
/// (I1953's `supplementary_only_weakens`, ADR 0087 decision 2): a clean failing local run of
/// the claim's test caps a passing claim at stale, on every surface and through the subjects
/// made of it, naming the run; a passing run never raises a failing claim.
#[test]
fn supplementary_only_weakens() {
    // a passing claim, and a failing local run of its test at a commit after the evidence
    let (f, _) = recorded();
    let (before, route, supplementary) = judged(&f, "claim:policy-parse");
    assert!(
        before == "proven" || before == "inputs_unchanged",
        "the fixture's claim does not start passing: {before}"
    );
    assert_eq!(supplementary["present"], false, "{supplementary}");
    assert_eq!(supplementary["records"], json!([]));
    assert_eq!(route["state"], before.as_str(), "{route}");

    let failed = local_run(&f, "fail");
    let (after, route, supplementary) = judged(&f, "claim:policy-parse");
    assert_eq!(
        after, "stale",
        "a clean failing run did not withhold proven"
    );
    assert_eq!(route["state"], "stale", "{route}");
    assert!(
        route["detail"]
            .as_str()
            .unwrap_or_default()
            .contains(&failed),
        "the weakened route does not name its record: {route}"
    );
    assert_eq!(
        supplementary["weakened"],
        json!([{
            "test": "suite:00_x", "from": before, "to": "stale", "by": [failed],
            "detail": route["detail"]
        }]),
        "{supplementary}"
    );
    assert_eq!(supplementary["records"][0]["id"], failed.as_str());
    assert_eq!(
        supplementary["records"][0]["weakened"],
        json!(["suite:00_x"])
    );

    // the subjects made of the claim read the weakened member, never their old verdict
    let feature = cli(&f, &["evidence", "subject", "feature:fixture-feature"]).unwrap();
    let member = feature["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["subject"] == "claim:policy-parse")
        .unwrap();
    assert_eq!(member["state"], "stale", "{feature}");

    // the same answer on every surface
    let by_cli = cli(&f, &["evidence", "subject", "claim:policy-parse"]).unwrap();
    let s = Served::start(&f.root(), &[]);
    let (status, by_http) = s.get("/api/v1/evidence/subject?subject=claim%3Apolicy-parse");
    assert_eq!(status, 200, "{by_http}");
    assert_eq!(by_http, by_cli, "HTTP and the command line disagree");
    let by_mcp = mcp_call(
        &f,
        "majordomus_evidence_subject",
        json!({ "subject": "claim:policy-parse" }),
    );
    assert_eq!(
        by_mcp["structuredContent"], by_cli,
        "MCP and the command line disagree"
    );

    // a stronger record never raises: a failing claim and a passing local run of its test
    let (g, _) = recorded_as("fail");
    let (before, _, _) = judged(&g, "claim:policy-parse");
    assert_eq!(
        before, "failing",
        "the fixture's claim does not start failing"
    );
    let passed = local_run(&g, "ok");
    let (after, route, supplementary) = judged(&g, "claim:policy-parse");
    assert_eq!(
        after, "failing",
        "a passing supplementary record raised a verdict"
    );
    assert_eq!(route["state"], "failing", "{route}");
    assert_eq!(supplementary["records"][0]["id"], passed.as_str());
    assert_eq!(supplementary["records"][0]["weakened"], json!([]));
    assert_eq!(supplementary["weakened"], json!([]), "{supplementary}");
}
