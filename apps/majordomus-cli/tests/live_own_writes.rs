//! A write this process made is a move of the repository, on every path that can make one.
//!
//! `plan.transition` stamps an issue file: a tracked file changes and no git control file
//! moves, so the stamp `Live` watches does not see it. The next read was answered from the
//! generation built before the write, and `plan.record` reported the status from before the
//! move. The executor now counts every successful repository mutation, whichever transport
//! or execution made it, and a generation built before a count moved is stale.

mod common;

use common::{Fixture, Served};
use serde_json::Value;

fn status_of(served: &Served, issue: &str) -> String {
    let (status, answer) = served.get(&format!("/api/v1/plan/record?id={issue}"));
    assert_eq!(status, 200, "{answer}");
    answer["issue"]["status"]
        .as_str()
        .unwrap_or_else(|| panic!("plan.record carries no status: {answer}"))
        .to_string()
}

#[test]
fn a_move_made_as_an_execution_is_read_back_by_the_next_request() {
    let f = Fixture::new();
    let served = Served::start(&f.root(), &[]);
    let before = status_of(&served, "I0001");

    // the execution path: a worker thread runs the capability, not the route
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

    // the write moved no git control file, and the answer is still not the one from before it
    let after = status_of(&served, "I0001");
    assert_ne!(
        after, before,
        "plan.record still reads the status from before the move ({before})"
    );
}
