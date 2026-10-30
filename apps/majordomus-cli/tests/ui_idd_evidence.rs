//! The intent workspace shows why an intent stands where it stands and every move made on its
//! issues (I2789), over a real socket.
//!
//! The standing is `intent_realization.explain`, which takes seconds, so the screen draws a
//! placeholder naming its own route and the script fills it; that route answers the intent's
//! unmet criteria, each with its evidence state, the command that reproduces it and the issues
//! serving it, exactly as the capability answers them. The moves are `executions.list`'s
//! `plan.transition` runs on the intent's issues: one on another intent's issue is not shown.
//!
//! They prove the criterion `the-owner-directs-work-from-the-ui#the-workspace-shows-why-and-what-moved`.

mod common;

use std::time::{Duration, Instant};

use common::{Fixture, Served};
use serde_json::{json, Value};

const STANDING: &str = "/ui/panels/standing?intent=fixture-intent";

/// Start `plan.transition` as the move form does and wait, bounded, for it to end.
fn moved(s: &Served, issue: &str, transition: &str) -> String {
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
            return id;
        }
        assert!(
            Instant::now() < deadline,
            "the move never ended: {execution}"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// The panel as a reader sees it: its markup set aside, the text kept in order.
fn visible(html: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in html.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            c if !inside => out.push(c),
            _ => {}
        }
    }
    out
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[test]
fn the_screen_names_the_standing_route_and_the_route_answers_what_the_capability_derives() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, _, page) = s.request("GET", "/ui/idd?intent=fixture-intent", None);
    assert_eq!(status, 200, "{page}");
    assert!(
        page.contains(&format!("data-panel-src=\"{}\"", escaped(STANDING))),
        "the first paint leaves the standing to its own route: {page}"
    );

    let (status, explain) = s.get("/api/v1/intents/explain?id=fixture-intent");
    assert_eq!(status, 200, "{explain}");
    let unmet = explain["realization"]["unmet"]
        .as_array()
        .expect("the unmet criteria");
    assert!(
        !unmet.is_empty(),
        "the fixture's criterion has no evidence yet: {explain}"
    );

    let (status, _, panel) = s.request("GET", STANDING, None);
    assert_eq!(status, 200, "{panel}");
    assert!(
        panel.contains("intent_realization.explain"),
        "the panel names its source"
    );
    for criterion in unmet {
        for key in ["id", "evidence", "reproduce"] {
            let value = criterion[key].as_str().expect("a field of the criterion");
            assert!(
                panel.contains(&escaped(value)),
                "the panel does not show the criterion's {key} `{value}` as the capability answers it"
            );
        }
        for issue in criterion["issues"].as_array().into_iter().flatten() {
            let issue = issue.as_str().expect("an issue id");
            assert!(
                panel.contains(issue),
                "the panel does not name {issue}, which serves it"
            );
        }
    }
    // a sentence reads whole once the markup is set aside: the issue ids in it are links (I2900)
    let read = visible(&panel);
    for line in explain["because"].as_array().expect("the sentences") {
        let line = line.as_str().expect("a sentence");
        // the sentence is drawn as prose (I3000): a code span keeps its words and loses its
        // backticks, so the sentence reads whole with its markup set aside, and every span is code
        let plain = line.replace("**", "").replace('`', "");
        assert!(
            read.contains(&escaped(&plain)),
            "the panel leaves out: {line}"
        );
        for (n, span) in line.split('`').enumerate() {
            if n % 2 == 1 && !span.is_empty() {
                assert!(
                    panel.contains(&format!(">{}</code>", escaped(span))),
                    "`{span}` in the sentence is not drawn as code: {line}"
                );
            }
        }
        for id in line
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| {
                w.len() > 1 && w.starts_with('I') && w[1..].bytes().all(|b| b.is_ascii_digit())
            })
        {
            assert!(
                panel.contains(&format!("issue={id}")),
                "the sentence names {id} and does not lead to it: {line}"
            );
        }
    }

    // the fragment is the panel alone, which is what the script puts in the placeholder
    let (status, _, fragment) = s.request("GET", &format!("{STANDING}&fragment"), None);
    assert_eq!(status, 200);
    assert!(fragment.trim_start().starts_with('<'), "{fragment}");
    assert!(
        !fragment.contains("<!doctype"),
        "a fragment carries no frame"
    );

    // a route that names no intent says what it needs, and one naming nothing an id could be
    // is not passed on
    let (status, _, empty) = s.request("GET", "/ui/panels/standing", None);
    assert_eq!(status, 200);
    assert!(empty.contains("Name an intent"), "{empty}");
    let (status, _, hostile) = s.request(
        "GET",
        "/ui/panels/standing?intent=x%22%3E%3Cscript%3E",
        None,
    );
    assert_eq!(status, 200);
    assert!(
        !hostile.contains("x\"><script>") && !hostile.contains("x%22"),
        "an intent no id could be is not written into the page: {hostile}"
    );
    assert!(hostile.contains("Name an intent"), "{hostile}");
}

#[test]
fn a_move_on_the_intents_issue_is_listed_and_a_move_on_another_intents_issue_is_not() {
    let f = Fixture::new();
    // a second milestone and an issue in it that serves no intent
    f.write(
        ".ai/repo/project/milestones/other-milestone.yaml",
        "id: other-milestone\ntitle: Another outcome\nslug: other-milestone\norder: 1\npriority: p2\nproblem: \"Another problem.\"\noutcome: \"Another outcome.\"\nacceptance_criteria:\n  - It is reached\nvalidation:\n  - \"true\"\nevidence_required:\n  - proof\n",
    );
    f.write(
        ".ai/repo/project/issues/I0002.yaml",
        "id: I0002\nmilestone: other-milestone\ntitle: Work for another outcome\nslug: issue-I0002\npriority: p2\nprofile: implementation\nobjective: \"Do other work.\"\nscope:\n  - other\nacceptance_criteria:\n  - It is done\nvalidation:\n  - \"true\"\nevidence_required:\n  - proof\n",
    );
    let s = Served::start(&f.root(), &[]);

    let (_, _, before) = s.request("GET", "/ui/idd?intent=fixture-intent", None);
    assert!(
        before.contains(&escaped(
            "No move was made on this intent's issues on this server"
        )),
        "{before}"
    );

    let ours = moved(&s, "I0001", "start");
    let theirs = moved(&s, "I0002", "start");

    let (status, _, page) = s.request("GET", "/ui/idd?intent=fixture-intent", None);
    assert_eq!(status, 200, "{page}");
    assert!(
        page.contains(&format!("id=\"move-{ours}\"")),
        "the move on I0001 is not listed"
    );
    assert!(
        page.contains(&format!("href=\"/cockpit/executions/{ours}\"")),
        "the move does not lead to its execution"
    );
    assert!(
        !page.contains(&theirs),
        "a move on another intent's issue is listed on this intent's workspace"
    );
    let (status, _, execution) = s.request("GET", &format!("/cockpit/executions/{ours}"), None);
    assert_eq!(
        status, 200,
        "the execution the row leads to is a page: {execution}"
    );
}
