//! The opposition through the built executable (ADR 0112): derived with no advisor and
//! answered alike on every surface; a recorded finding held to its class, its subject and
//! its resolution; a stamp that names the plan reviewed and moves only when the plan does.

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::{run_in, Fixture, Served, BIN};
use serde_json::{json, Value};

/// One MCP tool call over stdio: whether it was an error, and its structured content.
fn tool(f: &Fixture, name: &str, arguments: Value) -> (bool, Value) {
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
    (
        last["result"]["isError"] == true,
        last["result"]["structuredContent"].clone(),
    )
}

fn json_of(f: &Fixture, args: &[&str]) -> (i32, Value) {
    let mut argv = args.to_vec();
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(&f.root(), &argv, "");
    let v = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("{args:?} did not print JSON ({e}):\n{out}\n{err}"));
    (code, v)
}

fn oppose(f: &Fixture) -> (i32, Value) {
    json_of(f, &["intent", "oppose", "fixture-intent"])
}

const CRITIQUE: &str = ".ai/repo/project/critiques/fixture-intent.yaml";

/// A critique with one blocking finding in the given resolution, unstamped.
fn critique(f: &Fixture, resolution: &str) {
    f.write(
        CRITIQUE,
        &format!(
            "intent: fixture-intent
reviewed_at: HEAD
reviewed_by: the test
findings:
  - id: thin
    class: insufficient_work
    subject: fixture-intent#the-case-passes
    finding: \"One case: it may not be enough\"
    source: a second session
    blocking: true
    resolution:
{resolution}
"
        ),
    );
    f.commit("a critique");
}

fn stamp(f: &Fixture) -> Value {
    let (code, v) = json_of(
        f,
        &["intent", "stamp", "fixture-intent", "--by", "the test"],
    );
    assert_eq!(code, 0, "{v}");
    f.commit("the review is stamped");
    v
}

fn require_opposition(f: &Fixture) {
    let mut policy = std::fs::read_to_string(f.path(".ai/repo/policy.yaml")).unwrap();
    policy.push_str("\nintent:\n  opposition: required\n");
    f.write(".ai/repo/policy.yaml", &policy);
    f.commit("opposition is required");
}

/// Criterion `opposition-is-run` of intent `intent-opposition`: derived with no advisor, the
/// same value on the command line, HTTP and MCP, and a stamp that creates the record.
#[test]
fn the_opposition_runs_without_an_advisor() {
    let f = Fixture::new();
    let (code, v) = oppose(&f);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["intent"], "fixture-intent");
    assert_eq!(v["review"]["state"], "none");
    assert_eq!(v["disposition"], "accept");
    assert_eq!(v["recorded"], json!([]));
    assert_eq!(v["issues"][0]["id"], "I0001");
    assert_eq!(v["criteria"][0]["id"], "the-case-passes");
    assert_eq!(v["invariants"][0], "The fixture stays a valid repository");
    let revision = v["reviewed_plan"].as_str().unwrap().to_string();
    assert_eq!(revision.len(), 64);
    // the review is not among its own findings
    assert!(
        !v["structural"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["id"] == "plan_not_critiqued"),
        "{v}"
    );

    let (is_error, mcp) = tool(
        &f,
        "majordomus_intent_opposition",
        json!({ "intent": "fixture-intent" }),
    );
    assert!(!is_error);
    assert_eq!(v, mcp, "the command line and MCP disagree");
    let served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/intents/opposition?intent=fixture-intent");
    assert_eq!(status, 200);
    assert_eq!(v, http, "the command line and HTTP disagree");

    // an intent this repository does not hold is not found, never answered empty
    let (code, _, err) = run_in(&f.root(), &["intent", "oppose", "absent"], "");
    assert_eq!(code, 12, "{err}");

    // a check says what would be stamped and writes nothing
    let (code, check) = json_of(
        &f,
        &["intent", "stamp", "fixture-intent", "--check", "--by", "t"],
    );
    assert_eq!(code, 0, "{check}");
    assert_eq!(check["written"], false);
    assert_eq!(check["created"], true);
    assert_eq!(check["reviewed_revision"], revision.as_str());
    assert!(!f.path(CRITIQUE).exists(), "a check wrote the record");
    // a record this would create must say who reviewed
    let (code, _, err) = run_in(&f.root(), &["intent", "stamp", "fixture-intent"], "");
    assert_ne!(code, 0);
    assert!(err.contains("reviewed_by"), "{err}");

    let stamped = stamp(&f);
    assert_eq!(stamped["written"], true);
    assert_eq!(stamped["event"], "opposition.recorded");
    let text = std::fs::read_to_string(f.path(CRITIQUE)).unwrap();
    assert!(
        text.contains(&format!("reviewed_revision: {revision}")),
        "{text}"
    );
    assert!(text.ends_with("findings: []\n"), "{text}");
    let (code, v) = oppose(&f);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["review"]["state"], "current");
    assert_eq!(v["review"]["reviewed_revision"], revision.as_str());
    assert!(v["review"]["reviewed_with"]
        .as_str()
        .unwrap()
        .starts_with("majordomus-cli "));
    // the ledger carries the run
    let ledger = std::fs::read_to_string(f.path(".ai/local/state/ledger.jsonl")).unwrap();
    let event: Value = ledger
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|e| e["event"] == "opposition.recorded")
        .expect("no opposition.recorded in the ledger");
    assert_eq!(event["intent"], "fixture-intent");
    assert_eq!(event["reviewed_revision"], revision.as_str());
    assert_eq!(event["disposition"], "accept");
}

/// Criteria `stale-review-authorises-nothing` and `review-names-what-it-reviewed`: the
/// revision moves when what a review judges changes and at no other time; a stale review
/// refuses the binding; and a stamp leaves every line of the findings as it was.
#[test]
fn the_reviewed_plan_moves_only_with_the_plan() {
    let f = Fixture::new();
    critique(
        &f,
        "      state: planned\n      issue: I0001\n      resolved_by: the test",
    );
    let before = std::fs::read_to_string(f.path(CRITIQUE)).unwrap();
    let (_, v) = oppose(&f);
    assert_eq!(v["review"]["state"], "not_stamped");
    assert_eq!(v["disposition"], "accept_with_required_changes");
    assert_eq!(v["recorded"][0]["source"], "a second session");
    assert_eq!(v["recorded"][0]["resolved_by"], "the test");
    let revision = v["reviewed_plan"].as_str().unwrap().to_string();

    // unstamped is a warning where the policy does not require opposition, and binds
    let (code, validation) = json_of(&f, &["intent", "validate"]);
    assert_eq!(code, 0, "{validation}");
    let not_stamped = validation["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["code"] == "critique_not_stamped")
        .unwrap_or_else(|| panic!("no critique_not_stamped: {validation}"));
    assert_eq!(not_stamped["level"], "WARN");
    let (code, bound) = json_of(&f, &["intent", "binding", "--issue", "I0001"]);
    assert_eq!(code, 0, "{bound}");

    // the stamp is three lines, and the findings are byte for byte what the reviewer wrote
    stamp(&f);
    let after = std::fs::read_to_string(f.path(CRITIQUE)).unwrap();
    let findings = |s: &str| s[s.find("findings:").unwrap()..].to_string();
    assert_eq!(
        findings(&before),
        findings(&after),
        "the stamp touched a finding"
    );
    assert!(after.contains(&format!("reviewed_revision: {revision}")));
    let pinned = json_of(&f, &["intent", "binding", "--issue", "I0001"]).1["plan_revision"].clone();

    // a title, an objective and an unrelated file are not what a review judges
    let issue = std::fs::read_to_string(f.path(".ai/repo/project/issues/I0001.yaml")).unwrap();
    f.write(
        ".ai/repo/project/issues/I0001.yaml",
        &issue
            .replace(
                "The bounded piece of work",
                "The bounded piece of work, renamed",
            )
            .replace("Do the bounded piece of work.", "Do it well."),
    );
    f.commit("a title and an objective");
    let (_, v) = oppose(&f);
    assert_eq!(v["reviewed_plan"], revision.as_str());
    assert_eq!(v["review"]["state"], "current");
    // and stamping again moves neither the review nor a task's pin
    stamp(&f);
    let again = json_of(&f, &["intent", "binding", "--issue", "I0001"]).1;
    assert_eq!(
        again["plan_revision"], pinned,
        "a re-stamp moved the binding's pin"
    );
    assert_eq!(again["standing"], "bound");

    // the issue's scope is: the plan changed, and the review is of another plan
    let issue = std::fs::read_to_string(f.path(".ai/repo/project/issues/I0001.yaml")).unwrap();
    f.write(
        ".ai/repo/project/issues/I0001.yaml",
        &issue.replace("scope:\n  - lib\n", "scope:\n  - lib\n  - docs\n"),
    );
    f.commit("the issue reaches further");
    let (_, v) = oppose(&f);
    assert_ne!(v["reviewed_plan"], revision.as_str());
    assert_eq!(v["review"]["state"], "stale");
    let (code, refused) = json_of(&f, &["intent", "binding", "--issue", "I0001"]);
    assert_eq!(code, 10, "{refused}");
    assert_eq!(refused["refusals"][0]["cause"], "critique_stale");
    assert!(
        refused["refusal"]
            .as_str()
            .unwrap()
            .contains("intent stamp fixture-intent"),
        "{refused}"
    );
    let (code, validation) = json_of(&f, &["intent", "validate"]);
    assert_eq!(
        code, 0,
        "stale is a warning without the policy: {validation}"
    );

    // reviewed again and stamped, it binds
    stamp(&f);
    let (code, bound) = json_of(&f, &["intent", "binding", "--issue", "I0001"]);
    assert_eq!(code, 0, "{bound}");
    assert_eq!(oppose(&f).1["review"]["state"], "current");
}

/// Criteria `findings-are-admitted-typed` and `disposition-gates-execution`: an open
/// blocking finding rejects; a resolution into a cancelled issue is refused and the record
/// is not stamped; and where the policy requires opposition an unstamped critique fails
/// validation and refuses the binding.
#[test]
fn a_recorded_finding_and_the_policy_gate_execution() {
    let f = Fixture::new();
    critique(&f, "      state: open");
    let (code, v) = oppose(&f);
    assert_eq!(code, 10, "an open blocking finding rejects: {v}");
    assert_eq!(v["disposition"], "reject");
    assert_eq!(
        v["rejecting"],
        json!(["recorded thin fixture-intent#the-case-passes"])
    );

    // a second issue of the same criterion, cancelled: planning the finding into it resolves nothing
    let issue = std::fs::read_to_string(f.path(".ai/repo/project/issues/I0001.yaml")).unwrap();
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &(issue
            .replace("id: I0001", "id: I0002")
            .replace("issue-I0001", "issue-I0002")
            + "cancelled: true\n"),
    );
    critique(&f, "      state: planned\n      issue: I0002");
    let (code, validation) = json_of(&f, &["intent", "validate"]);
    assert_eq!(code, 10, "{validation}");
    assert!(
        validation["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["code"] == "planned_into_cancelled_issue" && x["level"] == "FAIL"),
        "{validation}"
    );
    let (code, _, err) = run_in(&f.root(), &["intent", "stamp", "fixture-intent"], "");
    assert_ne!(code, 0, "a record that does not hold was stamped");
    assert!(err.contains("planned_into_cancelled_issue"), "{err}");
    assert!(
        !std::fs::read_to_string(f.path(CRITIQUE))
            .unwrap()
            .contains("reviewed_revision"),
        "a refused stamp wrote"
    );
    // over MCP the refusal is the same refusal
    let (is_error, _) = tool(
        &f,
        "majordomus_intent_opposition_record",
        json!({ "intent": "fixture-intent" }),
    );
    assert!(is_error, "MCP stamped a record the command line refused");

    // planned into live work, the record holds; unstamped, it authorises work only where
    // the policy does not require opposition
    critique(&f, "      state: planned\n      issue: I0001");
    assert_eq!(json_of(&f, &["intent", "binding", "--issue", "I0001"]).0, 0);
    require_opposition(&f);
    let (code, validation) = json_of(&f, &["intent", "validate"]);
    assert_eq!(code, 10, "{validation}");
    let failing = |code: &str| {
        validation["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["code"] == code && x["level"] == "FAIL")
    };
    assert!(failing("critique_not_stamped"), "{validation}");
    // and a required change nobody is named as having accepted is not accepted
    assert!(failing("resolution_names_no_resolver"), "{validation}");
    critique(
        &f,
        "      state: planned\n      issue: I0001\n      resolved_by: the test",
    );
    let (code, refused) = json_of(&f, &["intent", "binding", "--issue", "I0001"]);
    assert_eq!(code, 10, "{refused}");
    assert_eq!(refused["refusals"][0]["cause"], "opposition_not_executed");

    // stamped over MCP, it validates and binds
    let (is_error, stamped) = tool(
        &f,
        "majordomus_intent_opposition_record",
        json!({ "intent": "fixture-intent" }),
    );
    assert!(!is_error, "{stamped}");
    assert_eq!(stamped["written"], true);
    f.commit("stamped over MCP");
    assert_eq!(json_of(&f, &["intent", "validate"]).0, 0);
    assert_eq!(json_of(&f, &["intent", "binding", "--issue", "I0001"]).0, 0);
}
