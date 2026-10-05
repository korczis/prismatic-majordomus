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
        run: None,
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
    let mut gone = Command::new(BIN)
        .arg("--version")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let writer = gone.stdin.take().unwrap();
    assert!(gone.wait().unwrap().success());
    let child = Command::new(BIN)
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
fn the_text_preflight_names_the_verdict_and_the_refusal_or_the_issues_it_followed() {
    let f = Fixture::new();
    let (code, out, _) = run_in(&f.root(), &["intent", "preflight", "--issue", "I9999"], "");
    assert_eq!(code, 10, "{out}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "verdict     refused");
    assert_eq!(
        lines[1],
        "refusal     `I9999` is not an issue under .ai/repo/project/issues/"
    );
    assert_eq!(
        lines.len(),
        2,
        "a refusal names no issue and no intent:\n{out}"
    );

    let (code, out, _) = run_in(&f.root(), &["intent", "preflight", "--issue", "I0001"], "");
    assert_eq!(code, 0, "{out}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines,
        [
            "verdict     serves",
            "issues      I0001",
            "intent      fixture-intent  planned  via milestone fixture-milestone and issue I0001",
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
