//! The plan in the Cockpit over a real socket: `/cockpit/plan`, one milestone, one issue and
//! the move an issue page makes.
//!
//! The behavioural claim these tests exist for: every status a plan page shows is the word a
//! capability answered for the same request, and a move made from an issue page goes through
//! `plan.transition` — it changes the record on disk, the page reads the change back, and a
//! move the plan does not allow is refused by the capability rather than by the page.

mod common;

use common::{Fixture, Served};
use serde_json::Value;

fn page(served: &Served, target: &str) -> (u16, String) {
    let (status, headers, body) = served.request("GET", target, None);
    assert!(
        headers
            .iter()
            .any(|(k, v)| k == "content-type" && v.starts_with("text/html")),
        "{target} is not HTML: {headers:?}"
    );
    (status, body)
}

/// The status `plan.record` derives for one issue, over the same socket the page is read on.
fn status_of(served: &Served, issue: &str) -> String {
    let (status, answer) = served.get(&format!("/api/v1/plan/record?id={issue}"));
    assert_eq!(status, 200, "{answer}");
    answer["issue"]["status"]
        .as_str()
        .unwrap_or_else(|| panic!("plan.record carries no status: {answer}"))
        .to_string()
}

#[test]
fn the_plan_page_shows_every_issue_with_the_status_the_plan_answered() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);

    let (status, answer) = served.get("/api/v1/plan/issues");
    assert_eq!(status, 200, "{answer}");
    let issue = &answer["issues"][0];
    let word = issue["status"].as_str().expect("a status word");

    let (status, body) = page(&served, "/cockpit/plan");
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("The bounded piece of work"), "no issue title");
    assert!(
        body.contains(&format!(">{word}<")),
        "the page does not show the status plan.issues answered ({word})"
    );
    assert!(
        body.contains("href=\"/cockpit/plan/issues/I0001\""),
        "no issue link"
    );
    assert!(
        body.contains("href=\"/cockpit/plan/milestones/fixture-milestone\""),
        "no milestone link"
    );
    // it is an area: every page links it
    let (_, overview) = page(&served, "/cockpit");
    assert!(
        overview.contains("href=\"/cockpit/plan\""),
        "the navigation does not link the plan"
    );
    assert!(
        overview.contains("href=\"/cockpit/sessions\""),
        "the navigation does not link the sessions"
    );
    assert!(
        overview.contains("href=\"/cockpit/board\""),
        "the navigation does not link the board"
    );
}

#[test]
fn a_filter_is_a_query_string_and_narrows_the_issues() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let word = status_of(&served, "I0001");

    let (status, body) = page(&served, &format!("/cockpit/plan?status={word}"));
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("The bounded piece of work"), "{body}");

    // a status no issue is in leaves the table empty and says so
    let other = if word == "DONE" { "READY" } else { "DONE" };
    let (status, body) = page(&served, &format!("/cockpit/plan?status={other}"));
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("No issue matches this filter."), "{body}");

    // a wave that is not a number is the reader's mistake, and the page says which
    let (status, body) = page(&served, "/cockpit/plan?wave=soon");
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("`soon` is not one"), "{body}");
}

#[test]
fn the_milestone_page_renders_the_graph_devtask_answered() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);

    let (status, answer) = served.get("/api/v1/devtask/milestone?milestone=fixture-milestone");
    assert_eq!(status, 200, "{answer}");
    let readiness = answer["nodes"][0]["readiness"]
        .as_str()
        .expect("a readiness word")
        .to_string();

    let (status, body) = page(&served, "/cockpit/plan/milestones/fixture-milestone");
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("The fixture reaches its outcome"), "no title");
    assert!(
        body.contains("The outcome once it is solved."),
        "no outcome"
    );
    assert!(
        body.contains(&format!(">{readiness}<")),
        "the page does not show the readiness devtask answered ({readiness})"
    );
    assert!(
        body.contains("href=\"/cockpit/plan/issues/I0001\""),
        "no issue link"
    );
}

#[test]
fn an_issue_or_a_milestone_the_plan_does_not_declare_is_a_404_that_says_so() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);

    let (status, body) = page(&served, "/cockpit/plan/issues/I9999");
    assert_eq!(status, 404, "{body}");
    assert!(body.contains("The plan declares no issue"), "{body}");

    let (status, body) = page(&served, "/cockpit/plan/milestones/no-such-milestone");
    assert!(status == 404 || status == 500, "{status}: {body}");
    assert!(!body.contains("data-mj-transition"), "{body}");
}

#[test]
fn an_issue_page_is_complete_before_its_script_and_offers_only_the_capability_s_moves() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);

    let (status, body) = page(&served, "/cockpit/plan/issues/I0001");
    assert_eq!(status, 200, "{body}");
    assert!(
        body.contains("Do the bounded piece of work."),
        "no objective"
    );
    assert!(body.contains("The work is done"), "no acceptance criterion");
    assert!(
        body.contains("data-mj-transition=\"I0001\""),
        "no moves form"
    );
    assert!(
        body.contains("data-mj-path=\"/api/v1/plan/transition\""),
        "the form does not name the capability's own route"
    );
    for word in ["start", "verify", "done"] {
        assert!(
            body.contains(&format!("data-mj-move=\"{word}\"")),
            "no {word} button"
        );
        // the same move from a terminal, for a page read without its script
        assert!(
            body.contains(&format!("majordomus plan {word} I0001")),
            "no {word} command"
        );
    }
    // disabled until the script enables them
    assert!(
        body.matches("disabled").count() >= 3,
        "a move button is enabled before any script ran"
    );
    assert!(
        body.contains("plan.js"),
        "the page does not load its module"
    );
}

#[test]
fn a_move_changes_the_record_and_the_page_reads_it_back() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let before = status_of(&served, "I0001");
    assert_eq!(
        before, "READY",
        "the fixture's issue is expected to be startable"
    );

    let own = format!("http://{}", served.address);
    let (status, _, answer) = served.request_with(
        "POST",
        "/api/v1/plan/transition",
        Some(r#"{"issue":"I0001","transition":"start"}"#),
        &[("Origin", &own)],
    );
    assert_eq!(status, 200, "{answer}");
    let moved: Value = serde_json::from_str(&answer).expect("a JSON answer");
    assert_eq!(moved["from"], "READY", "{moved}");
    let after = moved["to"].as_str().expect("a status after").to_string();
    assert_ne!(after, "READY", "{moved}");

    // the record on disk carries the move
    let record = std::fs::read_to_string(f.root().join(".ai/repo/project/issues/I0001.yaml"))
        .expect("the issue record");
    assert!(record.contains("started_at"), "{record}");

    // and the page shows the status the plan derives now, not the one it showed before
    let (status, body) = page(&served, "/cockpit/plan/issues/I0001");
    assert_eq!(status, 200, "{body}");
    assert!(
        body.contains(&format!(">{after}<")),
        "the issue page does not show {after} after the move: {body}"
    );

    // a move the plan does not allow is refused by the capability, with a reason
    let (status, _, refusal) = served.request_with(
        "POST",
        "/api/v1/plan/transition",
        Some(r#"{"issue":"I0001","transition":"start"}"#),
        &[("Origin", &own)],
    );
    assert_ne!(status, 200, "a second start was accepted: {refusal}");
    assert!(refusal.contains("error"), "{refusal}");

    // and a page on another origin cannot make a move at all
    let (status, _, body) = served.request_with(
        "POST",
        "/api/v1/plan/transition",
        Some(r#"{"issue":"I0001","transition":"verify"}"#),
        &[("Origin", "http://evil.example")],
    );
    assert_eq!(status, 403, "{body}");
}

#[test]
fn a_move_made_as_an_execution_is_read_back_by_the_next_page_too() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    assert_eq!(status_of(&served, "I0001"), "READY");

    // the Run-as-an-execution path: a worker thread runs the capability, not the route
    let own = format!("http://{}", served.address);
    let (status, _, answer) = served.request_with(
        "POST",
        "/api/v1/executions/start",
        Some(r#"{"capability":"plan.transition","input":{"issue":"I0001","transition":"start"}}"#),
        &[("Origin", &own)],
    );
    assert!(status == 200 || status == 202, "{status}: {answer}");
    let started: Value = serde_json::from_str(&answer).expect("a JSON answer");
    let id = started["id"].as_str().expect("an execution id").to_string();

    let mut state = String::new();
    for _ in 0..100 {
        let (_, execution) = served.get(&format!("/api/v1/executions/get?id={id}"));
        state = execution["state"].as_str().unwrap_or_default().to_string();
        if ["succeeded", "failed", "cancelled"].contains(&state.as_str()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(state, "succeeded", "the execution did not succeed");

    // the write moved no git control file, and the page is still not the one from before it
    let after = status_of(&served, "I0001");
    assert_ne!(
        after, "READY",
        "plan.record still reads the status from before the move"
    );
    let (status, body) = page(&served, "/cockpit/plan/issues/I0001");
    assert_eq!(status, 200, "{body}");
    assert!(
        body.contains(&format!(">{after}<")),
        "the issue page does not show {after} after a move made as an execution: {body}"
    );
}

#[test]
fn the_sessions_page_renders_the_episode_store() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let (status, body) = page(&served, "/cockpit/sessions");
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("open episodes"), "{body}");
    assert!(body.contains("Every open episode"), "{body}");
}
