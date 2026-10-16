//! The intent workspace, `/ui/idd`, over a real socket (I2780).
//!
//! The workspace is a projection of the plan's capabilities. These tests hold what the
//! owner relies on when directing the work from it:
//! - the intent, its milestone and its issues are drawn from the repository's own records,
//!   each value citing the capability it was read from;
//! - an issue added to the plan appears, in dependency order and as an edge of the graph,
//!   with no edit but its own record;
//! - an intent the link names that is not recorded is refused by name;
//! - the screen is read with GET, and a move is the capability's, never the screen's.
//!
//! They prove the criterion `the-owner-directs-work-from-the-ui#workspace-is-derived`.

mod common;

use common::{Fixture, Served};

const SECOND: &str = "id: I0002
milestone: fixture-milestone
title: The piece that waits for the first
slug: issue-I0002
priority: p1
profile: implementation
objective: \"Do the piece that waits for the first.\"
scope:
  - docs
depends_on:
  - I0001
serves:
  - fixture-intent#the-case-passes
acceptance_criteria:
  - The second piece is done
validation:
  - \"true\"
evidence_required:
  - proof
";

#[test]
fn the_workspace_draws_the_intent_and_what_serves_it_from_the_plan() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, _, page) = s.request("GET", "/ui/idd", None);
    assert_eq!(status, 200, "{page}");
    assert!(page.contains("<title>Intent workspace · majordomus ui</title>"));
    // the one intent is the one opened, and the rule that opened it is printed
    assert!(
        page.contains("href=\"/ui/idd?intent=fixture-intent\" aria-current=\"page\""),
        "{page}"
    );
    assert!(page.contains("A link that names no intent opens the first one"));
    // its milestone leads to the milestone's inspector, its issue to the issue's
    assert!(page.contains("/ui/roadmap?milestone=fixture-milestone#milestone"));
    assert!(page.contains("/ui/prs?issue=I0001#issue"));
    // every panel cites what it was read from
    for capability in [
        "intents.list",
        "plan.roadmap",
        "plan.issues",
        "plan.waves",
        "plan.next",
    ] {
        assert!(
            page.contains(&format!("/cockpit/capabilities/{capability}")),
            "the workspace does not cite {capability}"
        );
    }
    // the status the screen draws is the one the plan answers over the API
    let (status, issues) = s.get("/api/v1/plan/issues");
    assert_eq!(status, 200);
    let word = issues["issues"][0]["status"]
        .as_str()
        .expect("a status")
        .to_lowercase();
    let row = page
        .split("id=\"issue-I0001\"")
        .nth(1)
        .and_then(|rest| rest.split("</tr>").next())
        .expect("the issue's row");
    assert!(row.contains(&word), "{word} is not drawn in {row}");
}

#[test]
fn an_issue_added_to_the_plan_appears_in_order_and_in_the_graph_with_no_registration() {
    let f = Fixture::new();
    f.write(".ai/repo/project/issues/I0002.yaml", SECOND);
    f.commit("plan: a second issue");
    let s = Served::start(&f.root(), &[]);

    let (status, _, page) = s.request("GET", "/ui/idd?intent=fixture-intent", None);
    assert_eq!(status, 200, "{page}");
    let first = page.find("id=\"issue-I0001\"").expect("I0001 is drawn");
    let second = page.find("id=\"issue-I0002\"").expect("I0002 is drawn");
    assert!(first < second, "a dependency is listed before what waits for it");
    // the edge the record declares is the graph's, as the plan's dependency
    assert!(
        page.contains("&quot;source&quot;:&quot;I0001&quot;,&quot;target&quot;:&quot;I0002&quot;"),
        "the graph carries the dependency: {page}"
    );
    // and what blocks the second is the first, as a link
    let row = page
        .split("id=\"issue-I0002\"")
        .nth(1)
        .and_then(|rest| rest.split("</tr>").next())
        .expect("the second issue's row");
    assert!(row.contains("/ui/prs?issue=I0001#issue"), "{row}");
}

#[test]
fn an_intent_that_is_not_recorded_is_refused_by_name() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let (status, _, page) = s.request("GET", "/ui/idd?intent=no-such-intent", None);
    assert_eq!(status, 200);
    assert!(
        page.contains("no-such-intent is not an intent of this repository"),
        "{page}"
    );
    assert!(!page.contains("aria-current=\"page\" "), "nothing is opened in its place");
}

#[test]
fn the_workspace_is_read_and_changes_nothing() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);
    let before = f.snapshot();
    let (status, _, _) = s.request("POST", "/ui/idd", Some("{}"));
    assert_eq!(status, 405, "a move is a capability's route, not the screen's");
    let (status, _, _) = s.request("GET", "/ui/idd", None);
    assert_eq!(status, 200);
    assert_eq!(f.snapshot(), before, "reading the workspace wrote the repository");
}
