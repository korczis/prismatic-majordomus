//! An issue is moved from the intent workspace (I2781), over a real socket.
//!
//! The workspace renders, for a picked issue, the form the Cockpit's issue page renders, and
//! the page names the one module that wires it (`moves.js`, beside the Cockpit's assets). A
//! move is `plan.transition` run as an execution through `executions.start` — exactly what
//! the module posts — and the screen read again shows the status the record now derives.
//! The server, not the page, refuses an illegal move with its reason and a request from
//! another origin.
//!
//! They prove the criterion `the-owner-directs-work-from-the-ui#next-move-is-reachable`.

mod common;

use std::time::{Duration, Instant};

use common::{Fixture, Served};
use serde_json::{json, Value};

/// Start `plan.transition` as the move form does, from this server's own origin, and wait —
/// bounded — for the execution to end.
fn moved(s: &Served, issue: &str, transition: &str) -> Value {
    let origin = format!("http://{}", s.address);
    let (status, _, body) = s.request_with(
        "POST",
        "/api/v1/executions/start",
        Some(
            &json!({ "capability": "plan.transition", "input": { "issue": issue, "transition": transition } })
                .to_string(),
        ),
        &[("Origin", origin.as_str())],
    );
    assert_eq!(status, 200, "the move was not started: {body}");
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
            "the move never ended: {execution}"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn row(page: &str, issue: &str) -> String {
    page.split(&format!("id=\"issue-{issue}\""))
        .nth(1)
        .and_then(|rest| rest.split("</tr>").next())
        .expect("the issue's row")
        .to_string()
}

#[test]
fn an_issue_is_started_from_the_workspace_and_the_screen_reads_it_active() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let target = "/ui/idd?intent=fixture-intent&issue=I0001";

    let (status, _, page) = s.request("GET", target, None);
    assert_eq!(status, 200, "{page}");
    assert!(page.contains("data-mj-transition=\"I0001\""), "{page}");
    assert!(
        page.contains("data-mj-start=\"/api/v1/executions/start\""),
        "{page}"
    );
    // the page names the module that wires the form, and it is served
    let lib = page
        .split("data-moves-lib=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("the page names the moves module")
        .replace("&amp;", "&");
    assert!(lib.starts_with("/cockpit/assets/moves.js"), "{lib}");
    let (status, _, module) = s.request("GET", &lib, None);
    assert_eq!(status, 200);
    assert!(module.contains("export function installAll"), "{module}");
    assert!(row(&page, "I0001").contains("ready"), "{page}");

    let execution = moved(&s, "I0001", "start");
    assert_eq!(execution["state"], "succeeded", "{execution}");
    assert_eq!(execution["output"]["to"], "ACTIVE", "{execution}");

    let (_, _, page) = s.request("GET", target, None);
    assert!(
        row(&page, "I0001").contains("active"),
        "the workspace still reads the issue as before the move: {}",
        row(&page, "I0001")
    );
}

#[test]
fn a_move_the_plan_does_not_allow_is_refused_with_its_reason_and_changes_nothing() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let before =
        std::fs::read_to_string(f.path(".ai/repo/project/issues/I0001.yaml")).expect("the issue");
    let execution = moved(&s, "I0001", "done");
    assert_eq!(execution["state"], "failed", "{execution}");
    assert_eq!(execution["error"]["code"], "refused", "{execution}");
    assert!(
        execution["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("I0001 is READY")),
        "the refusal names why: {execution}"
    );
    assert_eq!(
        std::fs::read_to_string(f.path(".ai/repo/project/issues/I0001.yaml")).expect("the issue"),
        before,
        "a refused move wrote the record"
    );
}

#[test]
fn a_move_from_another_origin_is_refused_by_the_server() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let (status, _, body) = s.request_with(
        "POST",
        "/api/v1/executions/start",
        Some(
            &json!({ "capability": "plan.transition", "input": { "issue": "I0001", "transition": "start" } })
                .to_string(),
        ),
        &[("Origin", "http://evil.example")],
    );
    assert_eq!(status, 403, "{body}");
}
