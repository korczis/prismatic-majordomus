//! Intents through the built executable: the command line, HTTP and MCP answer the same
//! derivation; a recorded passing run is the only thing that moves a criterion to met; and an
//! intent model whose links do not resolve is refused, naming the finding.

mod common;

use std::io::Write;
use std::process::Stdio;

use common::{run_in, Fixture, Served};
use majordomus_cli::evidence::{digest_of, Execution, Ledger, Origin, Outcome, Runner, TestId};
use serde_json::{json, Value};

/// One MCP tool call over stdio; the structured content of its answer.
fn tool(f: &Fixture, name: &str, arguments: Value) -> Value {
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

/// The verdict of the fixture intent as the command line, HTTP and MCP each answer it, for both
/// the list and the record: every one of the six must be the same value.
fn verdict_everywhere(f: &Fixture) -> Value {
    let (_, list) = cli_json(f, &["intent", "list"]);
    let (_, shown) = cli_json(f, &["intent", "show", "fixture-intent"]);
    let served = Served::start(&f.root(), &[]);
    let (_, http_list) = served.get("/api/v1/intents");
    let (_, http_record) = served.get("/api/v1/intents/record?id=fixture-intent");
    let mcp_list = tool(f, "majordomus_intents", json!({}));
    let mcp_record = tool(
        f,
        "majordomus_intent_record",
        json!({ "id": "fixture-intent" }),
    );
    let cli = shown["verdict"].clone();
    assert!(cli.is_object(), "intent show carries no verdict: {shown:#}");
    for (surface, v) in [
        ("intent list", &list["intents"][0]["verdict"]),
        ("GET /api/v1/intents", &http_list["intents"][0]["verdict"]),
        ("GET /api/v1/intents/record", &http_record["verdict"]),
        ("majordomus_intents", &mcp_list["intents"][0]["verdict"]),
        ("majordomus_intent_record", &mcp_record["verdict"]),
    ] {
        assert_eq!(
            v, &cli,
            "{surface} and `intent show` disagree on the verdict"
        );
    }
    for (surface, counts) in [
        ("intent list", &list["verdicts"]),
        ("GET /api/v1/intents", &http_list["verdicts"]),
        ("majordomus_intents", &mcp_list["verdicts"]),
    ] {
        assert_eq!(
            counts,
            &json!({ cli["state"].as_str().unwrap(): 1 }),
            "{surface}"
        );
    }
    assert_eq!(
        shown["stage"], "planned",
        "the stage is the plan's and never moves here"
    );
    cli
}

#[test]
fn the_command_line_http_and_mcp_agree_on_the_verdict_the_evidence_alone_derives() {
    let f = Fixture::new();
    // nothing recorded: the test the ledger can settle is unmet, so the evidence says no
    assert_eq!(
        verdict_everywhere(&f),
        json!({ "state": "unsatisfied", "reasons": [
            { "criterion": "the-case-passes", "evidence": "test", "state": "not_run" }] })
    );

    // a passing run, recorded, satisfies the verdict while the milestone is still open
    let id = TestId::of("test/cases/00_x.sh").unwrap();
    let source = std::fs::read(f.path(&id.source())).unwrap();
    let mut ledger = Ledger::empty();
    ledger.merge([Execution {
        test: id.as_string(),
        runner: Runner::Suite,
        source: id.source(),
        outcome: Outcome::Pass,
        seconds: 1,
        commit: f.git(&["rev-parse", "HEAD"]).trim().to_string(),
        working_tree: "clean".into(),
        digest: digest_of(&source),
        at: "2026-10-05T00:00:00Z".into(),
        origin: Origin::Local,
        command: id.reproduce(),
        run: None,
        cases: vec![],
        detail: None,
    }]);
    ledger.save(&f.root()).unwrap();
    f.commit("record a passing run");
    assert_eq!(
        verdict_everywhere(&f),
        json!({ "state": "satisfied", "reasons": [] })
    );

    // a command criterion the ledger cannot settle leaves the evidence unable to answer
    f.write(
        ".ai/repo/project/intents/fixture-intent.yaml",
        &common::INTENT.replace(
            "    ref: test/cases/00_x.sh\n",
            "    ref: test/cases/00_x.sh
  - id: it-ships
    criterion: The fixture ships
    evidence: command
    ref: just ship
",
        ),
    );
    f.commit("a command criterion");
    assert_eq!(
        verdict_everywhere(&f),
        json!({ "state": "unknown", "reasons": [
            { "criterion": "it-ships", "evidence": "command", "state": "not_derivable" }] })
    );
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
        run: None,
        cases: vec![],
        detail: None,
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

/// A test criterion and a claim criterion over the same case, judged at every step through
/// the built executable: both read what the evidence module calls current, and nothing else.
#[test]
fn a_criterion_is_met_only_by_evidence_current_at_the_working_tree_and_follows_it_both_ways() {
    let f = Fixture::new();
    let intent = ".ai/repo/project/intents/fixture-intent.yaml";
    f.write(
        intent,
        &common::INTENT.replace(
            "    ref: test/cases/00_x.sh\n",
            "    ref: test/cases/00_x.sh
  - id: the-claim-holds
    criterion: The claim the case proves holds
    evidence: claim
    ref: policy-parse
",
        ),
    );
    f.commit("a claim criterion beside the test criterion");
    let declared = std::fs::read(f.path(intent)).unwrap();

    // (state, met) of both criteria, which must always agree: one judgement, two routes
    let judged = |f: &Fixture| -> (String, bool) {
        let (code, v) = cli_json(f, &["intent", "show", "fixture-intent"]);
        assert_eq!(code, 0, "{v:#}");
        let of = |id: &str| {
            let c = v["satisfaction"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["id"] == id)
                .unwrap()
                .clone();
            (c["state"].as_str().unwrap().to_string(), c["met"] == true)
        };
        let (test, claim) = (of("the-case-passes"), of("the-claim-holds"));
        assert_eq!(test, claim, "the test and the claim route disagree");
        test
    };
    let current = || ("current".to_string(), true);
    let not_met = |state: &str| (state.to_string(), false);
    // the verdict a met criterion rests on, through both routes: never one tick for two answers
    let proof = |f: &Fixture| -> String {
        let (_, v) = cli_json(f, &["intent", "show", "fixture-intent"]);
        let proofs: Vec<String> = v["satisfaction"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["proof"].as_str().unwrap_or("absent").to_string())
            .collect();
        assert_eq!(
            proofs[0], proofs[1],
            "the test and the claim route disagree"
        );
        proofs[0].clone()
    };

    let id = TestId::of("test/cases/00_x.sh").unwrap();
    // a run of the case as it is in the tree now, at HEAD, on a tree the recorder saw as `tree`
    let run = |outcome: Outcome, tree: &str| Execution {
        test: id.as_string(),
        runner: Runner::Suite,
        source: id.source(),
        outcome,
        seconds: 1,
        commit: f.git(&["rev-parse", "HEAD"]).trim().to_string(),
        working_tree: tree.into(),
        digest: digest_of(&std::fs::read(f.path(&id.source())).unwrap()),
        at: "2026-10-04T00:00:00Z".into(),
        origin: Origin::Local,
        command: id.reproduce(),
        run: None,
        cases: vec![],
        detail: None,
    };
    let record = |e: Execution| {
        let mut ledger = Ledger::empty();
        ledger.merge([e]);
        ledger.save(&f.root()).unwrap();
        f.commit("record a run");
    };

    assert_eq!(judged(&f), not_met("not_run"));

    // a clean pass at HEAD meets both
    record(run(Outcome::Pass, "clean"));
    assert_eq!(judged(&f), current());
    assert_eq!(proof(&f), "proven", "a pass at this revision");

    // a change the criterion does not name leaves it met: currency is the inputs, not HEAD;
    // and it says so, rather than wearing the tick of a proof at this revision
    f.write("README.md", "# Fixture\n\nRead AGENTS.md, then this.\n");
    f.commit("an unrelated change");
    assert_eq!(judged(&f), current());
    assert_eq!(
        proof(&f),
        "inputs_unchanged",
        "a pass whose named inputs did not move"
    );
    let (_, out, _) = run_in(&f.root(), &["intent", "show", "fixture-intent"], "");
    assert!(
        out.contains("current (inputs_unchanged)"),
        "the terminal names the verdict a met criterion rests on:\n{out}"
    );

    // the code under test changes in the working tree: the pass is of something else now,
    // though the test file hashes exactly as it did
    f.write(
        "lib/a.sh",
        "#!/usr/bin/env bash\n# the one library file\necho b\n",
    );
    assert_eq!(
        judged(&f),
        not_met("stale"),
        "an uncommitted change to the code"
    );
    f.commit("the code under test changes");
    assert_eq!(
        judged(&f),
        not_met("stale"),
        "a committed change to the code"
    );

    // a pass recorded on a dirty tree, whose test digest matches and whose named inputs did
    // not move, is still not current: its commit does not describe what ran
    f.write(
        "docs/notes.md",
        "pending, not committed when the case ran\n",
    );
    record(run(Outcome::Pass, "dirty"));
    assert_eq!(judged(&f), not_met("stale"), "a dirty-tree run");

    // repaired: a clean pass of the code as it is now meets it again
    record(run(Outcome::Pass, "clean"));
    assert_eq!(judged(&f), current(), "repaired");

    // regression: the latest run fails, and the criterion is unmet again
    record(run(Outcome::Fail, "clean"));
    assert_eq!(judged(&f), not_met("failing"), "regressed");

    // the case itself changes: the last pass no longer measured it
    record(run(Outcome::Pass, "clean"));
    assert_eq!(judged(&f), current());
    f.write(
        "test/cases/00_x.sh",
        "# majordomus-covers: none\n. \"$ROOT/test/lib.sh\"\ntrue\ntrue\n",
    );
    f.commit("the case changes");
    assert_eq!(judged(&f), not_met("stale"), "a changed case");
    record(run(Outcome::Pass, "clean"));
    assert_eq!(judged(&f), current(), "the changed case, run");

    // every state above was derived: the intent was never written
    assert_eq!(
        std::fs::read(f.path(intent)).unwrap(),
        declared,
        "the intent record changed"
    );
}

/// A test that proves no claim declares nothing about the code under test: its own file is not
/// that code. Its pass is current only while nothing has changed since the run, so a change to
/// the code alone, with the test file byte-identical, un-meets the criterion.
#[test]
fn a_test_that_names_no_code_under_test_is_current_only_at_the_commit_it_ran_on() {
    let f = Fixture::new();
    let case = "test/cases/01_y.sh";
    f.write(
        case,
        "# majordomus-covers: none\n# claims: none\n. \"$ROOT/test/lib.sh\"\ntrue\n",
    );
    let intent = ".ai/repo/project/intents/fixture-intent.yaml";
    f.write(
        intent,
        &common::INTENT.replace(
            "    ref: test/cases/00_x.sh\n",
            &format!("    ref: {case}\n"),
        ),
    );
    f.commit("a criterion over a case that proves no claim");

    let id = TestId::of(case).unwrap();
    let record_clean_pass = |f: &Fixture| {
        let mut ledger = Ledger::empty();
        ledger.merge([Execution {
            test: id.as_string(),
            runner: Runner::Suite,
            source: id.source(),
            outcome: Outcome::Pass,
            seconds: 1,
            commit: f.git(&["rev-parse", "HEAD"]).trim().to_string(),
            working_tree: "clean".into(),
            digest: digest_of(&std::fs::read(f.path(&id.source())).unwrap()),
            at: "2026-10-04T00:00:00Z".into(),
            origin: Origin::Local,
            command: id.reproduce(),
            run: None,
            cases: vec![],
            detail: None,
        }]);
        ledger.save(&f.root()).unwrap();
        f.commit("record a run");
    };
    let judged = |f: &Fixture| -> (String, bool, String) {
        let (code, v) = cli_json(f, &["intent", "show", "fixture-intent"]);
        assert_eq!(code, 0, "{v:#}");
        let c = &v["satisfaction"][0];
        assert_eq!(c["ref"], case);
        (
            c["state"].as_str().unwrap().to_string(),
            c["met"] == true,
            c["proof"].as_str().unwrap_or("absent").to_string(),
        )
    };
    let is = |state: &str, met: bool, proof: &str| (state.to_string(), met, proof.to_string());

    record_clean_pass(&f);
    assert_eq!(
        judged(&f),
        is("current", true, "proven"),
        "a pass at this revision"
    );

    // the code under test breaks, the case's file untouched: nothing the repository declares
    // rules the change out, so the pass is not current evidence of the code as it is now
    f.write(
        "lib/a.sh",
        "#!/usr/bin/env bash\n# the one library file\nexit 1\n",
    );
    assert_eq!(
        judged(&f),
        is("stale", false, "stale"),
        "an uncommitted change to the code"
    );
    f.commit("the code under test breaks");
    assert_eq!(
        judged(&f),
        is("stale", false, "stale"),
        "a committed change to the code"
    );

    // a run of the code as it is now meets it again
    record_clean_pass(&f);
    assert_eq!(
        judged(&f),
        is("current", true, "proven"),
        "rerun at the checkout"
    );

    // the issue serving the criterion declares `lib` as the code under test, so a change
    // outside it does not move the pass: it stays met, and says it rests on unchanged inputs
    // rather than on a run at this revision
    f.write("README.md", "# Fixture\n\nA later edit.\n");
    f.commit("a change outside the declared code under test");
    assert_eq!(
        judged(&f),
        is("current", true, "inputs_unchanged"),
        "a change outside the scope the serving issue declares"
    );

    // with nothing declared — no issue serves the criterion, and the case proves no claim —
    // the test's own file is not the code under test, and any later change is one the pass
    // cannot rule out
    f.write(
        ".ai/repo/project/issues/I0001.yaml",
        &common::ISSUE.replace("serves:\n  - fixture-intent#the-case-passes\n", ""),
    );
    f.commit("the issue no longer serves the criterion");
    record_clean_pass(&f);
    assert_eq!(
        judged(&f),
        is("current", true, "proven"),
        "a pass at this revision, with nothing declared"
    );
    f.write("README.md", "# Fixture\n\nAnother edit.\n");
    f.commit("a later change");
    assert_eq!(
        judged(&f),
        is("stale", false, "stale"),
        "a change it names nothing against"
    );
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

/// Record a critique of the fixture's intent with no finding, so work serving it may proceed.
fn critiqued(f: &Fixture) {
    f.write(
        ".ai/repo/project/critiques/fixture-intent.yaml",
        "intent: fixture-intent\nreviewed_at: HEAD\nreviewed_by: the test\nfindings: []\n",
    );
    f.commit("the plan is critiqued");
}

/// Which intent the work on an issue serves, or the link that is missing: the claim
/// `intent-preflight-names-the-intent` of docs/CLAIMS.yaml.
#[test]
fn preflight_names_the_intent_the_work_serves_or_why_it_may_not_proceed() {
    let f = Fixture::new();
    // the fixture's issue serves its intent, whose plan nobody has critiqued yet
    let (code, v) = cli_json(&f, &["intent", "preflight", "--issue", "I0001"]);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["verdict"], "refused");
    assert_eq!(v["refusals"][0]["cause"], "intent_not_critiqued");
    assert_eq!(v["intents"][0]["id"], "fixture-intent");

    critiqued(&f);
    let (code, v) = cli_json(&f, &["intent", "preflight", "--issue", "I0001"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["verdict"], "serves");
    assert_eq!(v["issues"][0]["verdict"], "serves");
    assert_eq!(v["matches"][0]["intent"], "fixture-intent");
    assert_eq!(v["matches"][0]["criteria"], json!(["the-case-passes"]));
    assert_eq!(v["governance"], json!(["rule:project.alpha"]));
    let held = &v["intents"][0];
    assert_eq!(held["criteria"][0]["id"], "the-case-passes");
    assert_eq!(held["criteria"][0]["state"], "not_run");
    assert_eq!(
        held["invariants"][0],
        "The fixture stays a valid repository"
    );
    assert_eq!(held["critique"]["open_blocking"], json!([]));
    let mcp = tool(
        &f,
        "majordomus_intent_preflight",
        json!({ "issue": "I0001" }),
    );
    assert_eq!(v, mcp, "the command line and MCP disagree");
    let served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/intents/preflight?issue=I0001");
    assert_eq!(status, 200);
    assert_eq!(v, http, "the command line and HTTP disagree");

    let (code, v) = cli_json(&f, &["intent", "preflight", "--issue", "I9999"]);
    assert_eq!(code, 10);
    assert_eq!(v["refusals"][0]["cause"], "unknown_issue");
    assert!(v["refusal"].as_str().unwrap().contains("I9999"));

    // the issue still serves the intent once it is gone: a link to nothing
    f.remove(".ai/repo/project/intents/fixture-intent.yaml");
    f.remove(".ai/repo/project/critiques/fixture-intent.yaml");
    f.commit("no intent");
    let (code, v) = cli_json(&f, &["intent", "preflight", "--issue", "I0001"]);
    assert_eq!(code, 10);
    assert_eq!(v["refusals"][0]["cause"], "serves_unknown_criterion");

    // and once it serves nothing either, it is maintenance under a milestone of no intent,
    // which `intent validate` accepts and the preflight therefore does too
    let issue = std::fs::read_to_string(f.path(".ai/repo/project/issues/I0001.yaml")).unwrap();
    f.write(
        ".ai/repo/project/issues/I0001.yaml",
        &issue.replace("serves:\n  - fixture-intent#the-case-passes\n", ""),
    );
    f.commit("maintenance");
    let (code, out, _) = run_in(&f.root(), &["intent", "validate"], "");
    assert_eq!(code, 0, "{out}");
    let (code, v) = cli_json(&f, &["intent", "preflight", "--issue", "I0001"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["verdict"], "maintenance");
    assert_eq!(v["intents"], json!([]));
    assert_eq!(v["refusals"], json!([]));

    let (code, _, err) = run_in(&f.root(), &["intent", "preflight"], "");
    assert_ne!(code, 0, "a preflight of nothing is refused");
    assert!(err.contains("name the issue"), "{err}");
}

#[test]
fn preflight_refuses_work_while_a_blocking_critique_finding_is_open() {
    let f = Fixture::new();
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
    let (code, v) = cli_json(&f, &["intent", "preflight", "--path", "lib/a.sh"]);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["refusals"][0]["cause"], "open_blocking_finding");
    assert_eq!(v["refusals"][0]["issue"], "I0001");
    assert_eq!(
        v["intents"][0]["critique"]["open_blocking"][0]["id"],
        "thin"
    );

    f.write(
        ".ai/repo/project/critiques/fixture-intent.yaml",
        &critique("      state: planned\n      issue: I0001"),
    );
    f.commit("plan the blocker");
    let (code, v) = cli_json(&f, &["intent", "preflight", "--path", "lib/a.sh"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["verdict"], "serves");
}

/// The four verbs of `majordomus intent`, each over the fixture's one intent and one issue.
const VERBS: [&[&str]; 4] = [
    &["intent", "list"],
    &["intent", "show", "fixture-intent"],
    &["intent", "validate"],
    &["intent", "preflight", "--issue", "I0001"],
];

/// Run the executable with its stdout a pipe whose reading end is already closed, so that
/// writing the answer fails; the exit code and stderr.
fn run_into_a_closed_pipe(f: &Fixture, args: &[&str]) -> (i32, String) {
    // `std::io::pipe` is newer than the crate's rust-version. The writing end of a pipe whose
    // only reader has exited is the stdin std made for a process that is gone: std opens it
    // close-on-exec, so no process another test spawns meanwhile inherits either end.
    let mut gone = common::majordomus()
        .arg("--version")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let writer = gone.stdin.take().unwrap();
    assert!(gone.wait().unwrap().success());
    let child = common::majordomus()
        .args(args)
        .current_dir(f.root())
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .stdin(Stdio::null())
        .stdout(Stdio::from(writer))
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stderr).unwrap(),
    )
}

#[test]
fn an_answer_that_cannot_be_written_is_a_transport_failure_and_never_a_success() {
    let f = Fixture::new();
    critiqued(&f);
    for args in VERBS {
        // each verb answers 0 over this fixture when its answer can be written
        let (code, _, err) = run_in(&f.root(), args, "");
        assert_eq!(code, 0, "{args:?}: {err}");
        let (code, err) = run_into_a_closed_pipe(&f, args);
        assert_eq!(
            code, 13,
            "{args:?} reported success it could not deliver: {err}"
        );
        assert!(err.contains("majordomus: transport:"), "{args:?}: {err}");
    }
}

#[test]
fn a_ledger_this_executable_cannot_read_refuses_every_verb_rather_than_reading_as_not_run() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/evidence/ledger.json",
        "{ \"this is\": not a ledger",
    );
    f.commit("an unreadable ledger");
    for args in VERBS {
        let (code, out, err) = run_in(&f.root(), args, "");
        assert_eq!(
            code, 13,
            "{args:?} answered over an unreadable ledger:\n{out}"
        );
        assert!(out.is_empty(), "{args:?} printed an answer:\n{out}");
        assert!(
            err.contains("is not a ledger this version can read"),
            "{args:?}: {err}"
        );
    }
    // the repository itself loads: the refusal is the derivation's, reached only past it
    let (code, _, err) = run_in(&f.root(), &["intent", "preflight"], "");
    assert_ne!(code, 0);
    assert!(err.contains("name the issue"), "{err}");
}

#[test]
fn an_intent_the_repository_does_not_hold_is_not_found_on_the_command_line() {
    let f = Fixture::new();
    let (code, out, err) = run_in(&f.root(), &["intent", "show", "absent"], "");
    assert_eq!(
        code, 12,
        "a missing intent is the missing-artifact code:\n{out}"
    );
    assert!(err.contains("no intent 'absent'"), "{err}");
    assert!(err.contains("`intents.list`"), "{err}");
}

#[test]
fn outside_a_repository_no_intent_verb_answers() {
    let f = Fixture::plain_dir();
    for args in VERBS {
        let (code, out, _) = run_in(&f.root(), args, "");
        assert_eq!(code, 12, "{args:?} outside a repository:\n{out}");
        assert!(out.is_empty(), "{args:?}:\n{out}");
    }
}

#[test]
fn the_text_preflight_names_the_verdict_each_issue_what_the_intent_asks_and_each_refusal() {
    let f = Fixture::new();
    let (code, out, _) = run_in(&f.root(), &["intent", "preflight", "--issue", "I9999"], "");
    assert_eq!(code, 10, "{out}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines,
        [
            "verdict     refused",
            "refusal     unknown_issue  `I9999` is not an issue under .ai/repo/project/issues/",
        ],
        "a refusal before any issue names no issue and no intent"
    );

    let (code, out, _) = run_in(&f.root(), &["intent", "preflight", "--issue", "I0001"], "");
    assert_eq!(code, 10, "{out}");
    assert!(
        out.contains("refusal     intent_not_critiqued  I0001 serves intent fixture-intent"),
        "{out}"
    );

    critiqued(&f);
    let (code, out, _) = run_in(&f.root(), &["intent", "preflight", "--issue", "I0001"], "");
    assert_eq!(code, 0, "{out}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines,
        [
            "verdict     serves",
            "issue       I0001  serves  milestone fixture-milestone  \
             serves fixture-intent#the-case-passes",
            "intent      fixture-intent  planned  The fixture's outcome is true",
            "  statement   The outcome the fixture milestone reaches is true for its users.",
            "  criterion   the-case-passes  not_run",
            "  invariant   The fixture stays a valid repository",
            "  critique    reviewed at HEAD by the test; open blocking: none",
            "governance  rule:project.alpha",
        ]
    );
}

#[test]
fn governance_claims_files_and_deployments_resolve_against_the_repository_itself() {
    let f = Fixture::new();
    f.write(
        ".ai/repo/project/intents/fixture-intent.yaml",
        &common::INTENT
            .replace(
                "governance:\n  - rule:project.alpha\n",
                "governance:
  - rule:project.alpha
  - claim:policy-parse
  - claim:no-such-claim
  - file:README.md
  - file:docs/../README.md
  - file:no/such/file.md
",
            )
            .replace(
                "    ref: test/cases/00_x.sh\n",
                "    ref: test/cases/00_x.sh
  - id: it-is-deployed
    criterion: The fixture's deployment answers
    evidence: deployment
    ref: fixture-deployment
  - id: it-is-deployed-nowhere
    criterion: A deployment the repository does not declare answers
    evidence: deployment
    ref: nowhere
",
            ),
    );
    f.commit("governance and deployments");

    let (code, v) = cli_json(&f, &["intent", "validate"]);
    assert_eq!(code, 10, "{v:#}");
    let messages = |code: &str| -> Vec<String> {
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| x["code"] == code)
            .map(|x| x["message"].as_str().unwrap().to_string())
            .collect()
    };
    let governance = messages("unresolved_governance");
    for unresolved in [
        "claim:no-such-claim",
        "file:docs/../README.md",
        "file:no/such/file.md",
    ] {
        assert!(
            governance
                .iter()
                .any(|m| m.contains(&format!("`{unresolved}`"))),
            "{unresolved} is not reported: {governance:?}"
        );
    }
    for resolved in ["rule:project.alpha", "claim:policy-parse", "file:README.md"] {
        assert!(
            !governance
                .iter()
                .any(|m| m.contains(&format!("`{resolved}`"))),
            "{resolved} names something the repository holds: {governance:?}"
        );
    }
    assert_eq!(governance.len(), 3, "{governance:?}");
    let refs = messages("unresolved_evidence_ref");
    assert_eq!(refs.len(), 1, "{refs:?}");
    assert!(
        refs[0].contains("`nowhere` is not a deployment"),
        "{refs:?}"
    );

    let (_, shown) = cli_json(&f, &["intent", "show", "fixture-intent"]);
    let state = |id: &str| {
        shown["satisfaction"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .map(|c| c["state"].clone())
            .unwrap()
    };
    // a declared deployment resolves, and is still nothing this executable can derive
    assert_eq!(state("it-is-deployed"), "not_derivable");
    assert_eq!(state("it-is-deployed-nowhere"), "unresolved");
}

/// `intent draft gap` as the text a worker saves, asserted to have succeeded.
fn drafted(f: &Fixture) -> String {
    let (code, out, err) = run_in(&f.root(), &["intent", "draft", "gap", "fixture-intent"], "");
    assert_eq!(code, 0, "{out}\n{err}");
    out
}

/// Record one run of the fixture's case at HEAD, against the source in the tree, and commit
/// the ledger; the commit the run was recorded at.
fn record_at_head(f: &Fixture, outcome: Outcome) -> String {
    let id = TestId::of("test/cases/00_x.sh").unwrap();
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    let mut ledger = Ledger::empty();
    ledger.merge([Execution {
        test: id.as_string(),
        runner: Runner::Suite,
        source: id.source(),
        outcome,
        seconds: 1,
        commit: head.clone(),
        working_tree: "clean".into(),
        digest: digest_of(&std::fs::read(f.path(&id.source())).unwrap()),
        at: "2026-10-21T00:00:00Z".into(),
        origin: Origin::Local,
        command: id.reproduce(),
        run: None,
        cases: vec![],
        detail: None,
    }]);
    ledger.save(&f.root()).unwrap();
    f.commit("record a run");
    head
}

/// The evidence half of a gap is drafted from the ledger (I3246): the criterion's condition
/// follows the evidence state at HEAD by the table of `intent_draft` and cites the
/// observation of what was read; the record, saved, passes `intent validate`'s gap checks;
/// and a draft writes nothing and is the same bytes at one commit.
#[test]
fn test_the_gap_is_drafted_from_the_ledger() {
    let f = Fixture::new();
    let gap = |f: &Fixture| -> Value {
        let (code, v) = cli_json(f, &["intent", "draft", "gap", "fixture-intent"]);
        assert_eq!(code, 0, "{v:#}");
        v
    };

    // (e) nothing is written, and two drafts at one commit are the same bytes
    let before = f.snapshot();
    let one = drafted(&f);
    let two = drafted(&f);
    assert_eq!(one, two, "two drafts at one commit differ");
    assert_eq!(f.snapshot(), before, "a draft changed the repository");
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    assert!(
        one.starts_with(&format!("intent: fixture-intent\nobserved_at: {head}\n")),
        "{one}"
    );

    // (a) no execution: the criterion is missing, and cites what the ledger did not hold
    let v = gap(&f);
    assert_eq!(v["observed_at"], head);
    assert_eq!(
        v["recorded_by"],
        format!("majordomus {}", majordomus_cli::VERSION)
    );
    assert_eq!(v["risks"], json!([]));
    assert_eq!(v["work"], json!([]));
    let observation = &v["observations"][0];
    assert_eq!(observation["id"], "the-case-passes-evidence");
    assert_eq!(observation["source"], "test:test/cases/00_x.sh");
    assert!(
        observation["statement"]
            .as_str()
            .unwrap()
            .contains("no execution"),
        "{observation}"
    );
    assert_eq!(
        v["conditions"],
        json!([{
            "criterion": "the-case-passes",
            "state": "missing",
            "because": "the evidence is not_run: no recorded run counts for it",
            "observations": ["the-case-passes-evidence"],
        }])
    );

    // (b) a passing run at HEAD: satisfied, and the observation names that execution
    let ran_at = record_at_head(&f, Outcome::Pass);
    let v = gap(&f);
    assert_eq!(v["conditions"][0]["state"], "satisfied", "{v:#}");
    let statement = v["observations"][0]["statement"].as_str().unwrap();
    assert_eq!(
        statement,
        format!("ledger: suite:00_x passed at {ran_at} (2026-10-21T00:00:00Z, runner suite)")
    );

    // (c) a failing run is missing work, and a contradiction once the serving issue is DONE
    let failed_at = record_at_head(&f, Outcome::Fail);
    let v = gap(&f);
    assert_eq!(v["conditions"][0]["state"], "missing", "{v:#}");
    assert!(v["observations"][0]["statement"]
        .as_str()
        .unwrap()
        .contains(&format!("failed at {failed_at}")));
    for (transition, to) in [("start", "ACTIVE"), ("done", "DONE")] {
        if transition == "done" {
            let rel = ".ai/repo/project/issues/I0001.yaml";
            let record = std::fs::read_to_string(f.path(rel)).unwrap();
            f.write(
                rel,
                &format!(
                    "{record}evidence:\n  - covers: proof\n    type: test\n    command: \"bash \
                     test/cases/00_x.sh\"\n    result: \"ok\"\n    commit: abc\n    recorded_at: \
                     2026-09-15T00:00:00Z\n"
                ),
            );
        }
        let moved = tool(
            &f,
            "majordomus_plan_transition",
            json!({ "issue": "I0001", "transition": transition }),
        );
        assert_eq!(moved["to"], to, "{moved}");
    }
    f.commit("the serving issue is DONE");
    let v = gap(&f);
    let condition = &v["conditions"][0];
    assert_eq!(condition["state"], "conflicting", "{v:#}");
    assert!(
        condition["because"].as_str().unwrap().contains("I0001"),
        "{condition}"
    );
    assert_eq!(
        condition["observations"],
        json!(["the-case-passes-evidence"])
    );

    // (d) the draft, saved and committed, passes the gap checks and the index unchanged
    let record = ".ai/repo/project/gaps/fixture-intent.yaml";
    f.write(record, &drafted(&f));
    f.commit("the drafted gap");
    let (_, validation) = cli_json(&f, &["intent", "validate"]);
    let gap_findings: Vec<&Value> = validation["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| {
            let code = x["code"].as_str().unwrap_or("");
            ["gap_", "observation_", "condition_"]
                .iter()
                .any(|p| code.starts_with(p))
        })
        .collect();
    assert!(gap_findings.is_empty(), "{gap_findings:#?}");
    let (_, inspected, _) = common::inspect(&f.root(), &[]);
    let about: Vec<Value> = common::diagnostics(&inspected)
        .into_iter()
        .filter(|d| d["path"] == record)
        .collect();
    assert!(about.is_empty(), "{about:#?}");

    // (e) again at the new commit; and a tree that is not its commit is the one risk
    let before = f.snapshot();
    assert_eq!(drafted(&f), drafted(&f));
    assert_eq!(f.snapshot(), before, "a draft changed the repository");
    f.write("README.md", "# Fixture\n\nAn edit nobody committed.\n");
    let v = gap(&f);
    assert_eq!(v["risks"].as_array().unwrap().len(), 1, "{v:#}");
    assert_eq!(
        v["observed_at"],
        f.git(&["rev-parse", "HEAD"]).trim(),
        "still the commit, with the difference named as a risk"
    );
}

/// The draft is one capability: the command line, HTTP and MCP answer the same value, and an
/// intent the repository does not hold is not found on every surface.
#[test]
fn the_command_line_http_and_mcp_draft_the_same_gap() {
    let f = Fixture::new();
    let (code, cli) = cli_json(&f, &["intent", "draft", "gap", "fixture-intent"]);
    assert_eq!(code, 0, "{cli:#}");
    let mcp = tool(
        &f,
        "majordomus_intent_draft_gap",
        json!({ "intent": "fixture-intent" }),
    );
    let served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/intents/draft/gap?intent=fixture-intent");
    assert_eq!(status, 200, "{http:#}");
    assert_eq!(cli, http, "the command line and HTTP disagree");
    assert_eq!(cli, mcp, "the command line and MCP disagree");

    let (status, _) = served.get("/api/v1/intents/draft/gap?intent=absent");
    assert_eq!(status, 404);
    let (code, _, err) = run_in(&f.root(), &["intent", "draft", "gap", "absent"], "");
    assert_eq!(code, 12, "{err}");
    assert!(err.contains("no intent 'absent'"), "{err}");
}
