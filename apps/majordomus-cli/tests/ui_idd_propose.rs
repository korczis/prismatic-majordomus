//! An intent is proposed from the intent workspace (I2807), over a real socket.
//!
//! The workspace renders a proposal form wired by the module that wires its moves
//! (`moves.js`): `intents.propose` started as an execution through `executions.start`, the
//! routes and the capability named by the page from `capabilities.describe`. The record the
//! execution writes is the proposal, and the workspace opens it. The capability, not the page,
//! refuses an incomplete proposal; the server refuses one from another origin.
//!
//! They prove the criterion `the-owner-directs-work-from-the-ui#an-intent-is-proposed-from-the-workspace`.

mod common;

use std::time::{Duration, Instant};

use common::{Fixture, Served};
use serde_json::{json, Value};

/// Start `intents.propose` as the form does, from this server's own origin, and wait —
/// bounded — for the execution to end.
fn proposed(s: &Served, input: Value) -> Value {
    let origin = format!("http://{}", s.address);
    let (status, _, body) = s.request_with(
        "POST",
        "/api/v1/executions/start",
        Some(&json!({ "capability": "intents.propose", "input": input }).to_string()),
        &[("Origin", origin.as_str())],
    );
    assert_eq!(status, 200, "the proposal was not started: {body}");
    let id = serde_json::from_str::<Value>(&body).expect("JSON")["id"]
        .as_str()
        .expect("an execution id")
        .to_string();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let (_, execution) = s.get(&format!("/api/v1/executions/get?id={id}"));
        if matches!(
            execution["state"].as_str(),
            Some("succeeded" | "failed" | "cancelled")
        ) {
            return execution;
        }
        assert!(
            Instant::now() < deadline,
            "the proposal never ended: {execution}"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn attribute(html: &str, name: &str) -> String {
    let start = html
        .find(&format!("{name}=\""))
        .unwrap_or_else(|| panic!("the page names {name}"))
        + name.len()
        + 2;
    let end = start + html[start..].find('"').expect("the attribute closes");
    html[start..end].replace("&amp;", "&")
}

#[test]
fn an_intent_is_proposed_from_the_workspace_and_the_workspace_opens_it() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, _, page) = s.request("GET", "/ui/idd", None);
    assert_eq!(status, 200, "{page}");
    assert!(
        page.contains("data-mj-propose"),
        "the workspace offers no proposal: {page}"
    );
    assert_eq!(attribute(&page, "data-mj-capability"), "intents.propose");
    assert_eq!(
        attribute(&page, "data-mj-start"),
        "/api/v1/executions/start"
    );
    assert_eq!(attribute(&page, "data-mj-follow"), "/api/v1/executions/get");
    assert_eq!(attribute(&page, "data-mj-open"), "/ui/idd?intent=");
    assert!(
        page.contains("lists the proposal once the record is staged"),
        "{page}"
    );
    let button = page
        .split("<button")
        .find(|tag| tag.contains("data-mj-propose-submit"))
        .and_then(|tag| tag.split('>').next())
        .expect("the proposal's button");
    assert!(
        button.contains("disabled"),
        "a page whose script did not load offers nothing it cannot do: <button{button}>"
    );
    // the module the page names wires the form
    let lib = attribute(&page, "data-moves-lib");
    let (status, _, module) = s.request("GET", &lib, None);
    assert_eq!(status, 200);
    assert!(
        module.contains("export function installProposal"),
        "{module}"
    );
    assert!(
        module.contains("[data-mj-propose]"),
        "installAll wires proposal forms"
    );

    let execution = proposed(
        &s,
        json!({
            "statement": "A reader of the workspace can start the work from what it is for.",
            "title": "Work starts from the workspace",
            "id": "work-starts-from-the-workspace",
        }),
    );
    assert_eq!(execution["state"], "succeeded", "{execution}");
    assert_eq!(
        execution["output"]["id"], "work-starts-from-the-workspace",
        "{execution}"
    );
    let record = f.path(".ai/repo/project/intents/work-starts-from-the-workspace.yaml");
    let written = std::fs::read_to_string(&record).expect("the proposal's record");
    assert!(
        written.contains("A reader of the workspace can start the work"),
        "{written}"
    );

    // the index reads the records git tracks: the untracked proposal is not listed yet, and
    // the page does not pretend otherwise
    let (_, _, page) = s.request("GET", "/ui/idd?intent=work-starts-from-the-workspace", None);
    assert!(
        page.contains("is not an intent of this repository"),
        "{page}"
    );

    // staged, the workspace the form links to opens it once the server has read it again
    let staged = std::process::Command::new("git")
        .args([
            "add",
            ".ai/repo/project/intents/work-starts-from-the-workspace.yaml",
        ])
        .current_dir(f.root())
        .status()
        .expect("git add");
    assert!(staged.success());
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let (status, _, page) =
            s.request("GET", "/ui/idd?intent=work-starts-from-the-workspace", None);
        assert_eq!(status, 200);
        if page.contains("Work starts from the workspace")
            && !page.contains("is not an intent of this repository")
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the workspace never opened the proposal: {page}"
        );
        std::thread::sleep(Duration::from_millis(300));
    }
}

#[test]
fn an_incomplete_proposal_is_refused_by_the_capability_and_writes_nothing() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let before: Vec<_> = std::fs::read_dir(f.path(".ai/repo/project/intents"))
        .expect("the intents")
        .map(|e| e.expect("an entry").file_name())
        .collect();

    let execution = proposed(&s, json!({ "statement": "   " }));
    assert_eq!(execution["state"], "failed", "{execution}");
    assert!(
        execution["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("an intent needs a statement")),
        "the refusal names why: {execution}"
    );
    let after: Vec<_> = std::fs::read_dir(f.path(".ai/repo/project/intents"))
        .expect("the intents")
        .map(|e| e.expect("an entry").file_name())
        .collect();
    assert_eq!(before, after, "a refused proposal wrote a record");

    // an existing intent is never overwritten
    let execution = proposed(
        &s,
        json!({ "statement": "Something else entirely.", "id": "fixture-intent" }),
    );
    assert_eq!(execution["state"], "failed", "{execution}");
    let fixture = std::fs::read_to_string(f.path(".ai/repo/project/intents/fixture-intent.yaml"))
        .expect("the fixture intent");
    assert_eq!(
        fixture,
        common::INTENT,
        "a proposal overwrote an existing intent"
    );
}

#[test]
fn a_proposal_from_another_origin_is_refused_by_the_server() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let (status, _, body) = s.request_with(
        "POST",
        "/api/v1/executions/start",
        Some(
            &json!({ "capability": "intents.propose", "input": { "statement": "From elsewhere." } })
                .to_string(),
        ),
        &[("Origin", "http://evil.example")],
    );
    assert_eq!(status, 403, "{body}");
    assert!(
        !f.path(".ai/repo/project/intents/from-elsewhere.yaml")
            .exists(),
        "a refused proposal wrote a record"
    );
}
