//! The consistency gate of the Dashboard Suite (ADR 0088), over a real socket.
//!
//! A dashboard has no data of its own: every card is one fact read out of another
//! capability's answer, and it says which — the capability, the input, the RFC 6901
//! pointer and the measure. These tests hold that sentence for every card the Overview
//! serves, and name none of them: they ask each card's source the card's own question over
//! the source's own route, read the pointer, and compare. A card that computed its own
//! number, read a field other than the one it names, or reported a pass for a source that
//! could not answer fails here, whichever card it is.
//!
//! The other half is the Cockpit: the overview page must carry each card's value and link
//! to each card's route, so the page and the route are one answer rendered twice.

mod common;

use common::{Fixture, Served};
use serde_json::Value;

/// The measure a card declares, applied to its source's answer. Written here a second
/// time on purpose: this is the oracle, and an oracle that called the implementation it
/// judges would agree with any mistake in it.
fn read(answer: &Value, pointer: &str, measure: &str) -> Value {
    let at = answer.pointer(pointer);
    match measure {
        "exact" => at.cloned().unwrap_or(Value::Null),
        "count" => match at {
            None | Some(Value::Null) => Value::from(0),
            Some(Value::Array(a)) => Value::from(a.len()),
            Some(Value::Object(o)) => Value::from(o.len()),
            Some(other) => other.clone(),
        },
        other => panic!("a card declares a measure nobody defined: {other}"),
    }
}

/// Every card of an overview, flattened, in order.
fn cards(overview: &Value) -> Vec<Value> {
    overview["questions"]
        .as_array()
        .expect("questions")
        .iter()
        .flat_map(|q| q["cards"].as_array().expect("cards").clone())
        .collect()
}

/// The route a capability declares, asked of the registry through its own route.
fn route_of(s: &Served, capability: &str) -> String {
    let (status, describe) = s.get(&format!("/api/v1/capability?id={capability}"));
    assert_eq!(status, 200, "{capability} is not a capability: {describe}");
    describe["exposure"]["http"]["path"]
        .as_str()
        .unwrap_or_else(|| panic!("{capability} declares no HTTP route"))
        .to_string()
}

/// A card's input as the query string of its source's GET route.
fn query(input: &Value) -> String {
    let pairs: Vec<String> = input
        .as_object()
        .expect("a card's input is an object")
        .iter()
        .map(|(k, v)| {
            let v = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            format!("{k}={v}")
        })
        .collect();
    if pairs.is_empty() {
        String::new()
    } else {
        format!("?{}", pairs.join("&"))
    }
}

/// What the Cockpit's element builder writes for a text in an attribute.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[test]
fn every_card_is_its_sources_answer_at_its_pointer() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, overview) = s.get("/api/v1/dashboard/overview");
    assert_eq!(status, 200, "{overview}");
    let ids: Vec<&str> = overview["questions"]
        .as_array()
        .expect("questions")
        .iter()
        .map(|q| q["id"].as_str().expect("a question id"))
        .collect();
    assert_eq!(ids, ["healthy", "changed", "broken", "action"]);

    let all = cards(&overview);
    assert!(!all.is_empty(), "the overview answers with no card");
    let mut seen = std::collections::BTreeSet::new();
    for card in &all {
        let id = card["id"].as_str().expect("a card id");
        assert!(seen.insert(id.to_string()), "{id} appears twice");
        let source = &card["source"];
        let capability = source["capability"].as_str().expect("a source capability");
        let pointer = source["pointer"].as_str().expect("a source pointer");
        let measure = source["measure"].as_str().expect("a source measure");
        assert!(
            pointer.starts_with('/'),
            "{id}: {pointer} is not a JSON pointer"
        );
        assert!(
            card["route"]
                .as_str()
                .is_some_and(|r| r.starts_with("/cockpit")),
            "{id} drills into no Cockpit page: {}",
            card["route"]
        );

        let target = format!("{}{}", route_of(&s, capability), query(&source["input"]));
        let (answered, answer) = s.get(&target);
        if answered == 200 {
            assert_eq!(
                read(&answer, pointer, measure),
                card["value"],
                "{id} is not {capability} at {pointer} ({measure}): the card computed a \
                 value of its own"
            );
        } else {
            // a source that cannot answer is unknown on the card, with nothing in its place
            assert_eq!(
                card["status"], "unknown",
                "{id}: {capability} did not answer ({answered}) and the card says {}",
                card["status"]
            );
            assert!(card["value"].is_null(), "{id} shows a value nobody gave it");
        }
    }

    // a question is as good as its worst card, and the overview as its worst question
    let rank = |w: &Value| match w.as_str() {
        Some("ok") => 0,
        Some("warn") => 1,
        Some("fail") => 2,
        _ => 3,
    };
    for q in overview["questions"].as_array().expect("questions") {
        let worst = q["cards"]
            .as_array()
            .expect("cards")
            .iter()
            .map(|c| rank(&c["status"]))
            .max()
            .unwrap_or(0);
        assert_eq!(rank(&q["status"]), worst, "{}", q["id"]);
    }
}

#[test]
fn the_cockpit_overview_carries_every_card_and_asks_first() {
    let f = Fixture::new();
    let s = Served::start(&f.root(), &[]);

    let (status, overview) = s.get("/api/v1/dashboard/overview");
    assert_eq!(status, 200);
    let (status, _, page) = s.request("GET", "/cockpit", None);
    assert_eq!(status, 200);

    // first on the page, before anything the page lays out of its own
    let questions = page
        .find("data-question=\"healthy\"")
        .expect("the overview page carries no four questions");
    let statistics = page
        .find("mj-stats")
        .expect("the overview keeps its statistics");
    assert!(questions < statistics, "the four questions are not first");

    for card in cards(&overview) {
        let id = card["id"].as_str().expect("a card id");
        let at = page
            .find(&format!("data-card=\"{}\"", escape(id)))
            .unwrap_or_else(|| panic!("the page does not carry {id}"));
        let item = &page[at..at + page[at..].find("</li>").expect("a closed item")];
        let value = escape(&card["value"].to_string());
        assert!(
            item.contains(&format!("data-value=\"{value}\"")),
            "the page shows {id} with a value other than the route's {value}: {item}"
        );
        let route = escape(card["route"].as_str().expect("a route"));
        assert!(
            item.contains(&format!("href=\"{route}\"")),
            "{id} does not link to {route}: {item}"
        );
        let capability = card["source"]["capability"].as_str().expect("a source");
        assert!(item.contains(capability), "{id} does not name {capability}");
    }
}

/// An issue of `milestone`, written as the fixture's own issue is, with `extra` appended.
fn issue_of(id: &str, milestone: &str, extra: &str) -> String {
    format!(
        "{}{extra}",
        common::ISSUE
            .replace("id: I0001", &format!("id: {id}"))
            .replace(
                "milestone: fixture-milestone",
                &format!("milestone: {milestone}")
            )
            .replace("slug: issue-I0001", &format!("slug: issue-{id}"))
    )
}

/// A milestone record in the plan's order at `order`.
fn milestone_of(id: &str, order: u32) -> String {
    format!(
        "id: {id}\ntitle: The {id} milestone\nslug: {id}\norder: {order}\npriority: p1\n\
         problem: \"A problem.\"\noutcome: \"An outcome.\"\nacceptance_criteria:\n  - It is \
         reached\nvalidation:\n  - \"true\"\nevidence_required:\n  - proof\n"
    )
}

/// The cells of one milestone's row on a page: from its `id` to the end of the row.
fn row<'a>(page: &'a str, milestone: &str) -> &'a str {
    let at = page
        .find(&format!("id=\"milestone-{milestone}\""))
        .unwrap_or_else(|| panic!("no row of {milestone}: {page}"));
    &page[at..at + page[at..].find("</tr>").expect("a closed row")]
}

/// Every milestone meter of the Overview, the Roadmap and the milestone inspector is DONE out
/// of `counts.required`, the denominator `plan.roadmap` derives the milestone's status from;
/// a milestone of no issue says so, and one of only cancelled issues requires none (I3440).
#[test]
fn milestone_meter_counts_required() {
    let f = Fixture::new();
    // a done stamp counts only when sealed by its transition, as `plan done` writes it
    let at = "2026-10-19T00:00:00Z";
    let seal = majordomus_cli::plan::seal(majordomus_cli::plan::Transition::Done, "I0101", at);
    let done = format!(
        "completed_at: {at}\ncompleted_event: \"{seal}\"\nevidence:\n  - covers: proof\n    \
         type: test\n    command: \"true\"\n    result: \"ok\"\n    commit: abc\n    \
         recorded_at: {at}\n"
    );
    let cancelled = "cancelled: true\n";
    f.write(
        ".ai/repo/project/milestones/m-mixed.yaml",
        &milestone_of("m-mixed", 1),
    );
    f.write(
        ".ai/repo/project/milestones/m-empty.yaml",
        &milestone_of("m-empty", 2),
    );
    f.write(
        ".ai/repo/project/milestones/m-dropped.yaml",
        &milestone_of("m-dropped", 3),
    );
    for (id, milestone, extra) in [
        ("I0101", "m-mixed", done.as_str()),
        ("I0102", "m-mixed", cancelled),
        ("I0103", "m-mixed", cancelled),
        ("I0104", "m-mixed", ""),
        ("I0105", "m-mixed", ""),
        ("I0106", "m-dropped", cancelled),
        ("I0107", "m-dropped", cancelled),
    ] {
        f.write(
            &format!(".ai/repo/project/issues/{id}.yaml"),
            &issue_of(id, milestone, extra),
        );
    }
    f.commit("plan: a milestone with cancelled work, one empty, one all cancelled");
    let mut s = Served::start(&f.root(), &[]);

    let (status, roadmap) = s.get("/api/v1/plan/roadmap");
    assert_eq!(status, 200, "{roadmap}");
    let counts = |id: &str| {
        roadmap["milestones"]
            .as_array()
            .and_then(|all| all.iter().find(|m| m["id"] == id))
            .map(|m| m["counts"].clone())
            .unwrap_or_else(|| panic!("plan.roadmap has no {id}: {roadmap}"))
    };
    let mixed = counts("m-mixed");
    assert_eq!(mixed["total"], 5, "{mixed}");
    assert_eq!(mixed["required"], 3, "{mixed}");
    assert_eq!(mixed["by_status"]["DONE"], 1, "{mixed}");
    assert_eq!(counts("m-empty")["total"], 0);
    assert_eq!(counts("m-dropped")["required"], 0);

    let (_, _, overview) = s.request("GET", "/ui", None);
    let (_, _, screen) = s.request("GET", "/ui/roadmap", None);
    let (_, _, inspector) = s.request("GET", "/ui/roadmap?milestone=m-mixed", None);
    s.stop();

    for (surface, page) in [("Overview", &overview), ("Roadmap", &screen)] {
        let mixed = row(page, "m-mixed");
        assert!(
            mixed.contains("1 of 3") && !mixed.contains("1 of 5"),
            "{surface}: {mixed}"
        );
        let empty = row(page, "m-empty");
        assert!(
            empty.contains("no issues")
                && !empty.contains("0 of 0")
                && !empty.contains("mj-ui-meter-track"),
            "{surface}: {empty}"
        );
        let dropped = row(page, "m-dropped");
        assert!(
            dropped.contains("no issue required")
                && !dropped.contains("0 of")
                && !dropped.contains("mj-ui-meter-track"),
            "{surface}: {dropped}"
        );
        let whole = row(page, "fixture-milestone");
        assert!(whole.contains("0 of 1"), "{surface}: {whole}");
    }
    let at = inspector
        .find("id=\"milestone\"")
        .expect("the inspector is drawn");
    assert!(
        inspector[at..].contains("1 of 3") && !inspector[at..].contains("1 of 5"),
        "{inspector}"
    );
}
