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
