//! Satisfaction through the built executable (ADR 0113): a judged criterion carries the run
//! that decided it on every surface; an optional criterion holds nothing back; and a failing
//! guard keeps an intent unsatisfied whatever its criteria say.

// claims: intent-evaluation-explained

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

fn json_of(f: &Fixture, args: &[&str]) -> (i32, Value) {
    let mut argv = args.to_vec();
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(&f.root(), &argv, "");
    let v = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("{args:?} did not print JSON ({e}):\n{out}\n{err}"));
    (code, v)
}

fn shown(f: &Fixture) -> Value {
    json_of(f, &["intent", "show", "fixture-intent"]).1
}

/// Record one run of `test` at HEAD over a clean tree, and commit the ledger.
fn record(f: &Fixture, test: &str, outcome: Outcome) {
    let id = TestId::of(test).unwrap();
    let source = std::fs::read(f.path(&id.source())).unwrap();
    let mut ledger = Ledger::load(&f.root()).unwrap_or_else(|_| Ledger::empty());
    ledger.merge([Execution {
        test: id.as_string(),
        runner: Runner::Suite,
        source: id.source(),
        outcome,
        seconds: 1,
        commit: f.git(&["rev-parse", "HEAD"]).trim().to_string(),
        working_tree: "clean".into(),
        digest: digest_of(&source),
        at: "2026-10-08T00:00:00Z".into(),
        origin: Origin::Local,
        command: id.reproduce(),
        run: None,
    }]);
    ledger.save(&f.root()).unwrap();
    f.commit("record a run");
}

/// The fixture's intent with `extra` appended under its satisfaction list, and `tail`
/// appended to the record.
fn intent_with(f: &Fixture, extra: &str, tail: &str) {
    f.write(
        ".ai/repo/project/intents/fixture-intent.yaml",
        &(common::INTENT.replace(
            "    ref: test/cases/00_x.sh\n",
            &format!("    ref: test/cases/00_x.sh\n{extra}"),
        ) + tail),
    );
    f.commit("the intent is edited");
}

/// Criterion `evaluation-is-explained`: a criterion nothing ran for carries no evaluation;
/// a judged one names the run; a stale one names what changed; the surfaces agree.
#[test]
fn an_evaluation_names_the_run_and_the_inputs() {
    let f = Fixture::new();
    let c = &shown(&f)["satisfaction"][0];
    assert_eq!(c["state"], "not_run");
    assert!(c.get("evaluation").is_none(), "nothing ran: {c}");

    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    record(&f, "test/cases/00_x.sh", Outcome::Pass);
    let view = shown(&f);
    let c = &view["satisfaction"][0];
    assert_eq!(c["state"], "current", "{c}");
    let e = &c["evaluation"];
    assert_eq!(e["commit"], head.as_str());
    assert_eq!(e["working_tree"], "clean");
    assert_eq!(e["outcome"], "pass");
    assert_eq!(e["at"], "2026-10-08T00:00:00Z");
    assert!(e.get("changed").is_none(), "nothing changed since: {e}");

    // every surface answers the evaluation the command line printed
    let mcp = tool(
        &f,
        "majordomus_intent_record",
        json!({ "id": "fixture-intent" }),
    );
    assert_eq!(view, mcp, "the command line and MCP disagree");
    let served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/intents/record?id=fixture-intent");
    assert_eq!(status, 200);
    assert_eq!(view, http, "the command line and HTTP disagree");
    // and explain says it in a sentence
    let (_, explained) = json_of(&f, &["intent", "explain", "fixture-intent"]);
    assert!(
        explained["because"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap().contains(&format!(
                "judged by the run recorded at {head} (clean tree, pass"
            ))),
        "{explained}"
    );

    // the code the issue serving the criterion owns changes: stale, and it says what changed
    f.write("lib/a.sh", "echo changed\n");
    f.commit("the code under test changes");
    let c = &shown(&f)["satisfaction"][0];
    assert_eq!(c["state"], "stale", "{c}");
    assert_eq!(
        c["evaluation"]["commit"],
        head.as_str(),
        "still the same run"
    );
    assert_eq!(c["evaluation"]["changed"], json!(["lib/a.sh"]), "{c}");

    // run again at the new commit, it is current again, by a new run
    record(&f, "test/cases/00_x.sh", Outcome::Pass);
    let c = &shown(&f)["satisfaction"][0];
    assert_eq!(c["state"], "current", "{c}");
    assert_ne!(c["evaluation"]["commit"], head.as_str());
    // asked twice, the answer is the same answer
    assert_eq!(shown(&f), shown(&f));
}

/// Criterion `optional-criterion-does-not-block`: an unmet optional criterion holds the
/// verdict back no more than it holds work back, and an intent that requires nothing is
/// refused.
#[test]
fn an_optional_criterion_blocks_nothing() {
    let f = Fixture::new();
    intent_with(
        &f,
        "  - id: nice-to-have\n    criterion: It would be nice\n    evidence: test\n    ref: test/cases/00_x.sh\n    optional: true\n",
        "",
    );
    // nobody serves the optional criterion: a warning, never a refusal
    let (code, validation) = json_of(&f, &["intent", "validate"]);
    assert_eq!(code, 0, "{validation}");
    let uncovered = validation["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["code"] == "optional_criterion_uncovered")
        .unwrap_or_else(|| panic!("no optional_criterion_uncovered: {validation}"));
    assert_eq!(uncovered["level"], "WARN");
    assert_eq!(uncovered["subject"], "fixture-intent#nice-to-have");

    let view = shown(&f);
    assert_eq!(view["optional"], 1);
    assert_eq!(view["met"], 0);
    assert_eq!(view["satisfaction"][1]["optional"], true);
    assert_eq!(
        view["verdict"],
        json!({ "state": "unsatisfied", "reasons": [
            { "criterion": "the-case-passes", "evidence": "test", "state": "not_run" }] }),
        "only the required criterion is a reason"
    );

    // the required criterion alone decides: its run makes the verdict satisfied. Both
    // criteria name the same test here, so make the optional one name a test nobody ran.
    f.write("test/cases/01_never.sh", ". \"$ROOT/test/lib.sh\"\ntrue\n");
    intent_with(
        &f,
        "  - id: nice-to-have\n    criterion: It would be nice\n    evidence: test\n    ref: test/cases/01_never.sh\n    optional: true\n",
        "",
    );
    record(&f, "test/cases/00_x.sh", Outcome::Pass);
    let view = shown(&f);
    assert_eq!(view["satisfaction"][1]["state"], "not_run");
    assert_eq!(
        view["verdict"],
        json!({ "state": "satisfied", "reasons": [] })
    );
    assert_eq!(
        (view["met"].clone(), view["optional"].clone()),
        (json!(1), json!(1))
    );

    // every criterion optional: the intent requires nothing, which is refused
    f.write(
        ".ai/repo/project/intents/fixture-intent.yaml",
        &common::INTENT.replace(
            "    ref: test/cases/00_x.sh\n",
            "    ref: test/cases/00_x.sh\n    optional: true\n",
        ),
    );
    f.commit("nothing is required");
    let (code, validation) = json_of(&f, &["intent", "validate"]);
    assert_eq!(code, 10, "{validation}");
    assert!(
        validation["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["code"] == "intent_without_required_criterion" && x["level"] == "FAIL"),
        "{validation}"
    );
    assert_eq!(shown(&f)["verdict"]["state"], "unknown");
}

/// Criterion `guard-is-judged`, through the executable: a failing guard keeps the verdict
/// unsatisfied although the criterion is met; a guard nobody ran violates nothing; and a
/// guard is part of the plan a review stamped.
#[test]
fn a_failing_guard_keeps_the_verdict_unsatisfied() {
    let f = Fixture::new();
    f.write("test/cases/01_guard.sh", ". \"$ROOT/test/lib.sh\"\ntrue\n");
    f.commit("the guard's test");
    let (_, before) = json_of(&f, &["intent", "oppose", "fixture-intent"]);
    intent_with(
        &f,
        "",
        "guards:\n  - id: stays-valid\n    invariant: The fixture stays a valid repository\n    evidence: test\n    ref: test/cases/01_guard.sh\n",
    );
    assert_eq!(json_of(&f, &["intent", "validate"]).0, 0);
    // a guard is part of what a review judges
    let (_, after) = json_of(&f, &["intent", "oppose", "fixture-intent"]);
    assert_ne!(after["reviewed_plan"], before["reviewed_plan"]);

    record(&f, "test/cases/00_x.sh", Outcome::Pass);
    let view = shown(&f);
    // nobody ran the guard: not judged, and the met criterion decides
    assert_eq!(view["guards"][0]["state"], "not_run");
    assert_eq!(view["guards"][0]["violated"], false);
    assert_eq!(
        view["verdict"],
        json!({ "state": "satisfied", "reasons": [] })
    );

    // it runs and fails: violated, and the intent is not satisfied whatever its criterion says
    record(&f, "test/cases/01_guard.sh", Outcome::Fail);
    let view = shown(&f);
    assert_eq!(view["satisfaction"][0]["met"], true);
    assert_eq!(view["guards"][0]["violated"], true);
    assert_eq!(view["guards"][0]["standing"], "violated");
    assert_eq!(view["guards"][0]["evaluation"]["outcome"], "fail");
    assert_eq!(
        view["verdict"],
        json!({ "state": "unsatisfied", "reasons": [],
                "guards": [{ "guard": "stays-valid", "evidence": "test", "state": "failing" }] })
    );
    let mcp = tool(
        &f,
        "majordomus_intent_record",
        json!({ "id": "fixture-intent" }),
    );
    assert_eq!(view, mcp, "the command line and MCP disagree");
    let (_, out, _) = run_in(&f.root(), &["intent", "show", "fixture-intent"], "");
    assert!(
        out.contains("  violated    stays-valid  test failing"),
        "{out}"
    );
    let (_, explained) = json_of(&f, &["intent", "explain", "fixture-intent"]);
    assert!(
        explained["because"].as_array().unwrap().iter().any(|l| l
            .as_str()
            .unwrap()
            .contains("guard `stays-valid` is violated")),
        "{explained}"
    );

    // it passes again: the guard holds and the intent is satisfied again
    record(&f, "test/cases/01_guard.sh", Outcome::Pass);
    let view = shown(&f);
    assert_eq!(view["guards"][0]["violated"], false);
    assert_eq!(view["verdict"]["state"], "satisfied");

    // a guard may not take a criterion's id
    intent_with(
        &f,
        "",
        "guards:\n  - id: the-case-passes\n    invariant: A clash\n    evidence: test\n    ref: test/cases/01_guard.sh\n",
    );
    let (code, validation) = json_of(&f, &["intent", "validate"]);
    assert_eq!(code, 10, "{validation}");
    assert!(
        validation["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["code"] == "duplicate_guard"),
        "{validation}"
    );
}
