//! The binding through the built executable (ADR 0111): what a piece of work serves, asked by
//! issue, by intent and by paths, answered alike on the command line, HTTP and MCP; an
//! exemption only as a class the policy declares, with a reason; paths that reach two intents
//! refused as ambiguous; and two pins, of which the plan's moves only when the plan does.

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

fn binding(f: &Fixture, args: &[&str]) -> (i32, Value) {
    let mut argv = vec!["intent", "binding"];
    argv.extend(args);
    argv.extend(["--format", "json"]);
    let (code, out, err) = run_in(&f.root(), &argv, "");
    let v = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("{args:?} did not print JSON ({e}):\n{out}\n{err}"));
    (code, v)
}

fn critique(f: &Fixture, finding: &str) {
    f.write(
        ".ai/repo/project/critiques/fixture-intent.yaml",
        &format!(
            "intent: fixture-intent
reviewed_at: HEAD
reviewed_by: the test
findings:
  - id: thin
    class: insufficient_work
    subject: fixture-intent#the-case-passes
    finding: {finding}
    blocking: true
    resolution:
      state: planned
      issue: I0001
"
        ),
    );
    f.commit("the plan is critiqued");
}

/// The policy with an `intent:` block declaring one exemption class.
fn declare_exemption(f: &Fixture) {
    let mut policy = std::fs::read_to_string(f.path(".ai/repo/policy.yaml")).unwrap();
    policy.push_str(
        "\nintent:\n  binding: advisory\n  exemptions:\n    - id: emergency\n      description: Restoring a broken trunk\n",
    );
    f.write(".ai/repo/policy.yaml", &policy);
    f.commit("an exemption class");
}

/// A second intent with its own milestone and an issue whose scope is `scope`.
fn second_intent(f: &Fixture, scope: &str) {
    f.write(
        ".ai/repo/project/milestones/other-milestone.yaml",
        "id: other-milestone
title: The other outcome
slug: other-milestone
order: 1
priority: p1
problem: \"Another problem.\"
outcome: \"Another outcome.\"
acceptance_criteria:
  - It is reached
validation:
  - \"true\"
evidence_required: []
",
    );
    f.write(
        ".ai/repo/project/intents/other-intent.yaml",
        "id: other-intent
title: The other outcome is true
statement: \"Another thing is true.\"
invariants:
  - Nothing else breaks
milestones:
  - other-milestone
satisfaction:
  - id: it-holds
    criterion: The other thing holds
    evidence: test
    ref: test/cases/00_x.sh
",
    );
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &format!(
            "id: I0002
milestone: other-milestone
title: The other piece of work
slug: issue-I0002
priority: p1
profile: implementation
objective: \"Do the other piece of work.\"
scope:
  - {scope}
serves:
  - other-intent#it-holds
acceptance_criteria:
  - The work is done
validation:
  - \"true\"
evidence_required:
  - proof
"
        ),
    );
    f.write(
        ".ai/repo/project/critiques/other-intent.yaml",
        "intent: other-intent\nreviewed_at: HEAD\nreviewed_by: the test\nfindings: []\n",
    );
    f.commit("a second intent");
}

/// Criterion `binding-is-resolved` of intent `intent-bound-work`: an issue, an intent and
/// paths each resolve to the same intent and criteria with the same pins, every surface
/// answers the same value, and the plan revision moves when the plan does and at no other
/// time.
#[test]
fn binding_resolves_issue_intent_and_paths() {
    let f = Fixture::new();
    // nobody critiqued the plan yet: the preflight's refusal is the binding's refusal
    let (code, v) = binding(&f, &["--issue", "I0001"]);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["standing"], "refused");
    assert_eq!(v["refusals"][0]["cause"], "intent_not_critiqued");

    critique(&f, "One case may not be enough");
    let (code, by_issue) = binding(&f, &["--issue", "I0001"]);
    assert_eq!(code, 0, "{by_issue}");
    assert_eq!(by_issue["standing"], "bound");
    assert_eq!(by_issue["named"]["issue"], "I0001");
    assert_eq!(by_issue["intents"][0]["id"], "fixture-intent");
    assert_eq!(
        by_issue["intents"][0]["criteria"][0]["id"],
        "the-case-passes"
    );
    let plan = by_issue["plan_revision"].as_str().unwrap().to_string();
    let evidence = by_issue["evidence_standing"].as_str().unwrap().to_string();
    assert_eq!(plan.len(), 64, "a SHA-256, in hex");
    assert_eq!(evidence.len(), 64);

    // the same work named three ways is the same binding
    let (code, by_intent) = binding(&f, &["--intent", "fixture-intent"]);
    assert_eq!(code, 0, "{by_intent}");
    assert_eq!(by_intent["standing"], "bound");
    assert_eq!(by_intent["plan_revision"], plan.as_str());
    let (code, by_paths) = binding(&f, &["--path", "lib/a.sh"]);
    assert_eq!(code, 0, "{by_paths}");
    assert_eq!(by_paths["standing"], "bound");
    assert_eq!(by_paths["plan_revision"], plan.as_str());
    assert_eq!(by_paths["evidence_standing"], evidence.as_str());

    // every surface answers the value the command line printed
    let mcp = tool(&f, "majordomus_intent_binding", json!({ "issue": "I0001" }));
    assert_eq!(by_issue, mcp, "the command line and MCP disagree");
    let served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/intents/binding?issue=I0001");
    assert_eq!(status, 200);
    assert_eq!(by_issue, http, "the command line and HTTP disagree");

    // a path the issue's scope does not cover is said, and refuses nothing
    let (code, v) = binding(&f, &["--issue", "I0001", "--path", "docs/CLI.md"]);
    assert_eq!(code, 0, "{v}");
    assert!(
        v["notes"][0].as_str().unwrap().contains("docs/CLI.md"),
        "{v}"
    );
    assert_eq!(v["plan_revision"], plan.as_str(), "a note is not the plan");

    // an ordinary commit moves neither pin
    f.write("README.md", "# Fixture\n\nRead AGENTS.md first.\n");
    f.commit("unrelated");
    let (_, v) = binding(&f, &["--issue", "I0001"]);
    assert_eq!(v["plan_revision"], plan.as_str());
    assert_eq!(v["evidence_standing"], evidence.as_str());

    // the critique is edited: the plan the work was reviewed against is not the one pinned
    critique(&f, "One case is not enough");
    let (_, v) = binding(&f, &["--issue", "I0001"]);
    assert_ne!(v["plan_revision"], plan.as_str(), "the critique changed");
    assert_eq!(v["evidence_standing"], evidence.as_str());
    critique(&f, "One case may not be enough");
    let (_, v) = binding(&f, &["--issue", "I0001"]);
    assert_eq!(v["plan_revision"], plan.as_str(), "the hash is of content");

    // the criterion is rewritten
    let intent =
        std::fs::read_to_string(f.path(".ai/repo/project/intents/fixture-intent.yaml")).unwrap();
    f.write(
        ".ai/repo/project/intents/fixture-intent.yaml",
        &intent.replace(
            "The fixture's own case passes",
            "The fixture's own case passes everywhere",
        ),
    );
    f.commit("the criterion is rewritten");
    let (_, v) = binding(&f, &["--issue", "I0001"]);
    assert_ne!(v["plan_revision"], plan.as_str(), "the criterion changed");

    // what cannot be bound says which link is missing
    let (code, v) = binding(&f, &["--intent", "no-such-intent"]);
    assert_eq!(code, 10);
    assert_eq!(v["refusals"][0]["cause"], "unknown_intent");
    let (code, v) = binding(&f, &["--issue", "I9999"]);
    assert_eq!(code, 10);
    assert_eq!(v["refusals"][0]["cause"], "unknown_issue");
    let (code, v) = binding(&f, &[]);
    assert_eq!(code, 10);
    assert_eq!(v["refusals"][0]["cause"], "nothing_named");
    assert_eq!(v["plan_revision"], "", "nothing reached, nothing pinned");
}

/// The other half of `binding-is-resolved`: paths alone that reach two intents are refused,
/// and naming the issue is the way through.
#[test]
fn ambiguous_paths_are_refused() {
    let f = Fixture::new();
    critique(&f, "One case may not be enough");
    second_intent(&f, "lib");
    let (code, v) = binding(&f, &["--path", "lib/a.sh"]);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["standing"], "refused");
    let causes: Vec<&str> = v["refusals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["cause"].as_str().unwrap())
        .collect();
    assert_eq!(causes, ["ambiguous_intent"], "{v}");
    let said = v["refusal"].as_str().unwrap();
    assert!(
        said.contains("fixture-intent") && said.contains("other-intent"),
        "{said}"
    );

    // the issue settles it, either way
    let (code, v) = binding(&f, &["--issue", "I0002", "--path", "lib/a.sh"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["intents"][0]["id"], "other-intent");
    assert_eq!(v["intents"].as_array().unwrap().len(), 1);
    // and an issue beside an intent it does not serve is a contradiction, named
    let (code, v) = binding(&f, &["--issue", "I0002", "--intent", "fixture-intent"]);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["refusals"][0]["cause"], "issue_outside_intent");

    // paths that reach one intent are not ambiguous because a second exists
    let issue = std::fs::read_to_string(f.path(".ai/repo/project/issues/I0002.yaml")).unwrap();
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        &issue.replace("  - lib\n", "  - docs\n"),
    );
    f.commit("the second issue moves away");
    let (code, v) = binding(&f, &["--path", "lib/a.sh"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["standing"], "bound");
}

/// Criterion `exemption-is-typed`: an exemption is a class the policy declares, given with a
/// reason, and never beside named work; with no class declared there is none to give.
#[test]
fn an_exemption_is_a_declared_class_with_a_reason() {
    let f = Fixture::new();
    // the fixture's policy declares no class: nothing can be given
    let (code, v) = binding(
        &f,
        &["--exempt", "emergency", "--because", "the trunk is red"],
    );
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["refusals"][0]["cause"], "unknown_exemption");
    assert!(
        v["refusal"].as_str().unwrap().contains("declares none"),
        "{v}"
    );

    declare_exemption(&f);
    let (code, v) = binding(
        &f,
        &["--exempt", "emergency", "--because", "the trunk is red"],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["standing"], "exempt");
    assert_eq!(v["exemption"]["class"], "emergency");
    assert_eq!(v["exemption"]["because"], "the trunk is red");
    assert_eq!(v["exemption"]["description"], "Restoring a broken trunk");
    assert_eq!(v["intents"], json!([]), "an exempt task serves no intent");
    assert_eq!(v["plan_revision"], "");
    let mcp = tool(
        &f,
        "majordomus_intent_binding",
        json!({ "exemption": "emergency", "because": "the trunk is red" }),
    );
    assert_eq!(v, mcp, "the command line and MCP disagree");

    let (code, v) = binding(&f, &["--exempt", "emergency"]);
    assert_eq!(code, 10);
    assert_eq!(v["refusals"][0]["cause"], "exemption_without_reason");
    let (code, v) = binding(&f, &["--exempt", "emergency", "--because", "   "]);
    assert_eq!(code, 10, "a blank reason is no reason: {v}");
    assert_eq!(v["refusals"][0]["cause"], "exemption_without_reason");
    let (code, v) = binding(&f, &["--exempt", "whim", "--because", "I want to"]);
    assert_eq!(code, 10);
    assert_eq!(v["refusals"][0]["cause"], "unknown_exemption");
    assert!(v["refusal"].as_str().unwrap().contains("emergency"), "{v}");
    let (code, v) = binding(
        &f,
        &[
            "--issue",
            "I0001",
            "--exempt",
            "emergency",
            "--because",
            "both",
        ],
    );
    assert_eq!(code, 10);
    assert_eq!(v["refusals"][0]["cause"], "exemption_names_work");

    // work nobody exempted and no issue covers is refused: absence is not an exemption
    let (code, v) = binding(&f, &["--path", "docs/CLI.md"]);
    assert_eq!(code, 10, "{v}");
    assert_eq!(v["refusals"][0]["cause"], "no_issue_covers_paths");
}

/// The binding and the realization join agree about one task: the issue it named at start is
/// a declared link, whatever its title says (critique finding `realization-and-binding-disagree`).
#[test]
fn realization_and_binding_agree() {
    let f = Fixture::new();
    critique(&f, "One case may not be enough");
    // a task whose title cites no issue, started the way `start --issue I0001` records it
    f.write(
        ".ai/local/state/ledger.jsonl",
        "{\"at\":\"2026-10-07T10:00:00Z\",\"event\":\"task.started\",\"task_id\":\"t-9\",\"profile\":\"implementation\",\"owner\":\"t\",\"scope\":\"docs\",\"requires\":\"\",\"issue\":\"I0001\",\"intent\":\"\",\"exemption\":\"\",\"because\":\"\",\"binding\":\"bound\",\"plan_revision\":\"x\"}\n",
    );
    f.write(
        ".ai/local/state/current.yaml",
        "id: t-9\ntask: \"tidy the documents\"\nprofile: implementation\nscope:\n  - docs\nissue: I0001\nbinding: bound\nstarted_at: 2026-10-07T10:00:00Z\noutcome: active\n",
    );
    let (code, out, err) = run_in(
        &f.root(),
        &["intent", "realization", "--format", "json"],
        "",
    );
    assert_eq!(code, 0, "{out}\n{err}");
    let v: Value = serde_json::from_str(&out).unwrap();
    let work = v["work"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["work"]["id"] == "t-9")
        .unwrap_or_else(|| panic!("the task is not in the join: {v}"));
    let link = &work["links"][0];
    assert_eq!(link["issue"], "I0001", "{work}");
    assert_eq!(link["provenance"], "declared", "{work}");
    assert_eq!(link["intent"], "fixture-intent", "{work}");
    let (_, bound) = binding(&f, &["--issue", "I0001"]);
    assert_eq!(bound["intents"][0]["id"], link["intent"]);
}
