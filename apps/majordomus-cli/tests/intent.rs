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
