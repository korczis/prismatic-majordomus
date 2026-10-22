//! Evidence answers by subject, on every surface (I1953, ADR 0087 decision 3).
//!
//! `evidence.subject` judges any subject of the index — a claim, a rule, a feature, a
//! command, a capability, an MCP tool, a use case — and `evidence.coverage` lists the
//! coverage the run records carry. Each is one declaration projected three times, so the
//! command line, HTTP and an MCP `tools/call` must give the same document for the same
//! question, about a real repository with a recorded run in it; an unknown subject is
//! refused on each of them, with exit 13 on the command line.

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
    std::fs::write(&tsv, "00_x\tok\t1\tparallel\n").unwrap();
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
