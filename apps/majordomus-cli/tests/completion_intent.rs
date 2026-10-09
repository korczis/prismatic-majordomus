//! What a finish reads about the work a task serves (ADR 0115), through the built
//! executable: the completion answer carries the criterion states `intents.binding` answers,
//! the same on the command line, HTTP and MCP; a binding that reaches nothing is unknown and
//! never a pass; and an advisory policy reports the verdict it withholds.

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

fn json_of(f: &Fixture, args: &[&str]) -> Value {
    let mut argv = args.to_vec();
    argv.extend(["--format", "json"]);
    let (_, out, err) = run_in(&f.root(), &argv, "");
    serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("{args:?} did not print JSON ({e}):\n{out}\n{err}"))
}

/// The fixture with `intent.completion` set and an active task that names `issue`.
fn task(mode: &str, issue: &str) -> Fixture {
    let f = Fixture::new();
    f.write(
        ".ai/repo/policy.yaml",
        &format!("{}\nintent:\n  completion: {mode}\n", common::POLICY),
    );
    f.commit("the policy asks what a finish serves");
    let head = f.git(&["rev-parse", "HEAD"]).trim().to_string();
    f.write(
        ".ai/local/state/current.yaml",
        &format!(
            "id: t-1\ntask: \"Do the thing\"\nprofile: implementation\nowner: \"tester\"\n\
             scope:\n  - lib\nstarted_at: 2026-01-01T00:00:00Z\noutcome: active\nhead: {head}\n\
             working_tree: clean\nissue: {issue}\n"
        ),
    );
    f
}

/// The two questions about what the task serves, by id, from one completion answer.
fn asked(completion: &Value) -> Vec<Value> {
    ["criteria-served", "guards-hold"]
        .iter()
        .map(|id| {
            completion["questions"]
                .as_array()
                .unwrap_or_else(|| panic!("no questions: {completion}"))
                .iter()
                .find(|q| q["id"] == *id)
                .unwrap_or_else(|| panic!("no question {id}: {completion}"))
                .clone()
        })
        .collect()
}

/// Criterion `the-answer-is-the-engines`: the same two answers on every surface, in the
/// engine's own state for the criterion the task's issue serves.
#[test]
fn the_completion_answer_is_the_bindings_on_every_surface() {
    let f = task("required", "I0001");
    let binding = json_of(&f, &["intent", "binding", "--issue", "I0001"]);
    let state = binding["intents"][0]["criteria"][0]["state"]
        .as_str()
        .unwrap_or_else(|| panic!("the binding reached no criterion: {binding}"))
        .to_string();
    assert_eq!(state, "not_run", "nothing ran in the fixture: {binding}");

    let cli = json_of(&f, &["run", "gates.completion", "--input", "{}", "--quiet"]);
    let cli = asked(&cli["output"]);
    assert_eq!(cli[0]["status"], "queued", "never run is owed: {}", cli[0]);
    assert_eq!(cli[0]["source"], "intents.binding (ADR 0115)");
    let evidence = cli[0]["evidence"].as_str().unwrap();
    assert!(
        evidence.contains(&format!(
            "fixture-intent#the-case-passes (test test/cases/00_x.sh) is {state}"
        )),
        "{evidence}"
    );
    assert!(!evidence.contains("satisfied"), "{evidence}");
    assert!(
        cli[0].get("withheld").is_none(),
        "required withholds nothing"
    );
    assert_eq!(
        cli[1]["status"], "exempt",
        "the fixture's intent declares no guard"
    );

    let mcp = asked(&tool(&f, "majordomus_completion", json!({})));
    assert_eq!(cli, mcp, "the command line and MCP disagree");
    let served = Served::start(&f.root(), &[]);
    let (status, http) = served.get("/api/v1/gates/completion");
    assert_eq!(status, 200);
    assert_eq!(cli, asked(&http), "the command line and HTTP disagree");
}

/// A binding refused before it reaches an intent is unknown, and an advisory policy reports
/// the verdict it does not hold.
#[test]
fn a_binding_that_reaches_nothing_is_unknown_and_advisory_says_what_it_withholds() {
    let refused = task("required", "I9999");
    let answer = json_of(
        &refused,
        &["run", "gates.completion", "--input", "{}", "--quiet"],
    );
    for q in asked(&answer["output"]) {
        assert_eq!(q["status"], "unknown", "{q}");
        assert!(
            q["evidence"]
                .as_str()
                .unwrap()
                .starts_with("the binding was refused: "),
            "{q}"
        );
    }
    assert!(
        answer["output"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f.as_str().unwrap().contains("unknown rather than passing")),
        "{answer}"
    );

    let advisory = task("advisory", "I0001");
    let answer = json_of(
        &advisory,
        &["run", "gates.completion", "--input", "{}", "--quiet"],
    );
    let q = &asked(&answer["output"])[0];
    assert_eq!(q["status"], "exempt", "{q}");
    assert_eq!(q["withheld"], "queued", "{q}");
    assert!(
        q["evidence"].as_str().unwrap().starts_with(
            "reported and not held (intent.completion: advisory); it would be queued: "
        ),
        "{q}"
    );
    // and an advisory policy does not turn a binding that reached nothing into an exemption
    let both = task("advisory", "I9999");
    let answer = json_of(
        &both,
        &["run", "gates.completion", "--input", "{}", "--quiet"],
    );
    assert_eq!(asked(&answer["output"])[0]["status"], "unknown");
}
