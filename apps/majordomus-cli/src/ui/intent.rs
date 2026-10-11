//! One intent in full, for the Roadmap's inspector (`/ui/roadmap?intent=intent-bound-work`):
//! what it says, the invariants it holds the work to, the milestones and issues that serve
//! it, and each criterion of its satisfaction with the evidence that would prove it and
//! whether it is met.
//!
//! Read from `intents.list` and `plan.issues`, both answering in milliseconds. Every milestone
//! leads to its inspector and every issue to its own: from what the work is for to the work.

use serde_json::Value;

use crate::cockpit::html::{el, El};
use crate::cockpit::view::word_badge;

use super::source::Answer;
use super::view::{list, nothing, panel, state, table, text, unanswered};

/// An intent id, as the repository writes them (`intent-bound-work`): letters, digits, `-`
/// and `_`, and nothing a link could carry anything else in.
pub fn is_intent_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// The intent a `serves` entry names: an entry is the intent (`intent-planning`) or one of
/// its criteria (`intent-planning#criteria-are-covered`).
pub fn intent_of(serves: &str) -> &str {
    serves.split('#').next().unwrap_or(serves)
}

/// The intent a query names, when it names one.
pub fn picked(query: &[(String, String)]) -> Option<String> {
    query
        .iter()
        .find(|(k, _)| k == "intent")
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| is_intent_id(v))
}

/// Where an intent's inspector is.
pub fn href(id: &str) -> String {
    format!("{}?intent={id}#intent", super::roadmap::ROUTE)
}

fn chip(href: String, label: String) -> El {
    el("a")
        .class("mj-ui-chip mj-ui-chip--link")
        .attr("href", href)
        .child(el("code").class("mj-ui-chip-value").text(label))
}

fn strings(v: &Value, k: &str) -> Vec<String> {
    v.get(k)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The inspector of intent `id`.
pub fn inspector(intents: &Answer, issues: &Answer, id: &str) -> El {
    let body = unanswered(intents)
        .or_else(|| unanswered(issues))
        .unwrap_or_else(|| {
            let Some(i) = list(intents, "/intents")
                .into_iter()
                .find(|i| text(i, "id") == id)
            else {
                return nothing(&format!("{id} is not an intent of this repository."));
            };
            let milestones = i
                .get("milestones")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let serving: Vec<Value> = list(issues, "/issues")
                .into_iter()
                .filter(|x| strings(x, "serves").iter().any(|s| intent_of(s) == id))
                .collect();
            let criteria = i
                .get("satisfaction")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let met = criteria
                .iter()
                .filter(|c| c.get("met").and_then(Value::as_bool) == Some(true))
                .count();
            let rows = criteria
                .iter()
                .map(|c| {
                    let reproduce = text(c, "reproduce");
                    el("tr")
                        .child(
                            el("td")
                                .child(el("span").class("mj-ui-mono mj-ui-dim").text(text(c, "id")))
                                .child(
                                    el("span")
                                        .class("mj-ui-cell-title")
                                        .text(text(c, "criterion")),
                                ),
                        )
                        .child(el("td").class("mj-ui-mono").text(text(c, "evidence")))
                        .child(el("td").child(state(c, "state")))
                        .child(el("td").child(word_badge(
                            if c.get("met").and_then(Value::as_bool) == Some(true) {
                                "met"
                            } else {
                                "unmet"
                            },
                        )))
                        .child(el("td").child(if reproduce.is_empty() {
                            el("span").class("mj-ui-dim").text("none")
                        } else {
                            el("code").class("mj-ui-mono").text(reproduce)
                        }))
                })
                .collect::<Vec<_>>();
            el("section")
                .class("mj-ui-detail")
                .attr("id", "intent")
                .attr("aria-label", format!("Intent {id}"))
                .child(
                    el("div")
                        .class("mj-ui-detail-head")
                        .child(el("p").class("mj-ui-mono mj-ui-dim").text(id.to_string()))
                        .child(el("h3").class("mj-ui-detail-title").text(text(&i, "title")))
                        .child(
                            el("p")
                                .class("mj-ui-badges")
                                .child(state(&i, "stage"))
                                .child(state(&i, "verdict"))
                                .child(
                                    el("span")
                                        .class("mj-ui-mono mj-ui-dim")
                                        .text(format!("{met} of {} criteria met", criteria.len())),
                                )
                                .child(
                                    el("a")
                                        .class("mj-ui-more")
                                        .attr("href", super::roadmap::ROUTE)
                                        .text("Close"),
                                ),
                        ),
                )
                .child(el("p").class("mj-ui-lead").text(text(&i, "statement")))
                .child(
                    el("h4")
                        .class("mj-ui-panel-title")
                        .text("What it holds the work to"),
                )
                .child({
                    let inv = strings(&i, "invariants");
                    if inv.is_empty() {
                        nothing("It declares no invariant.")
                    } else {
                        el("ul")
                            .class("mj-ui-list")
                            .children(inv.into_iter().map(|t| el("li").text(t)))
                    }
                })
                .child(
                    el("h4")
                        .class("mj-ui-panel-title")
                        .text("Milestones that serve it"),
                )
                .child(if milestones.is_empty() {
                    nothing("No milestone serves it yet.")
                } else {
                    el("p")
                        .class("mj-ui-badges")
                        .children(milestones.iter().map(|m| {
                            let mid = text(m, "id");
                            el("span")
                                .child(chip(super::milestone::href(&mid), mid))
                                .child(state(m, "status"))
                        }))
                })
                .child(
                    el("h4")
                        .class("mj-ui-panel-title")
                        .text("Issues that serve it"),
                )
                .child(if serving.is_empty() {
                    nothing("No issue of the plan names it.")
                } else {
                    el("p")
                        .class("mj-ui-badges")
                        .children(serving.iter().map(|x| {
                            let iid = text(x, "id");
                            chip(format!("{}?issue={iid}#issue", super::prs::ROUTE), iid)
                        }))
                })
                .child(
                    el("h4")
                        .class("mj-ui-panel-title")
                        .text("When it is satisfied"),
                )
                .child(table(
                    &["Criterion", "Evidence", "State", "Met", "Reproduce"],
                    rows,
                    "It declares no criterion.",
                ))
        });
    panel(
        "Intent",
        "What the work is for, what it holds the work to, and whether the evidence says it is met.",
        body,
        &[intents, issues],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::source::{Exposure, Source};
    use std::time::Duration;

    fn answer(capability: &'static str, value: Value) -> Answer {
        Answer {
            source: Source::of(capability),
            route: None,
            exposure: Exposure::default(),
            value: Ok(value),
            took: Duration::from_millis(1),
        }
    }

    fn data() -> (Answer, Answer) {
        (
            answer(
                "intents.list",
                serde_json::json!({ "intents": [{
                    "id": "intent-a", "title": "A is done", "statement": "Work is bound.", "stage": "planned",
                    "verdict": "unmet", "invariants": ["one rule"],
                    "milestones": [{ "id": "M7", "status": "BLOCKED" }],
                    "satisfaction": [
                        { "id": "c1", "criterion": "it binds", "evidence": "test", "state": "not_run", "met": false, "reproduce": "cargo test --test x" },
                        { "id": "c2", "criterion": "it says why", "evidence": "doc", "state": "passed", "met": true, "reproduce": "" }
                    ]
                }]}),
            ),
            answer(
                "plan.issues",
                serde_json::json!({ "issues": [
                { "id": "I0009", "serves": ["intent-a#c1"] }, { "id": "I0010", "serves": ["other"] } ]}),
            ),
        )
    }

    #[test]
    fn an_intent_shows_its_criteria_and_leads_to_what_serves_it() {
        let (intents, issues) = data();
        let html = inspector(&intents, &issues, "intent-a").render();
        for want in [
            "id=\"intent\"",
            "1 of 2 criteria met",
            "cargo test --test x",
            "/ui/roadmap?milestone=M7#milestone",
            "/ui/prs?issue=I0009#issue",
            "one rule",
        ] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        assert!(
            !html.contains("I0010"),
            "an issue serving another intent is not its own"
        );
    }

    #[test]
    fn an_unknown_intent_says_so_and_a_hostile_one_is_never_picked() {
        let (intents, issues) = data();
        assert!(inspector(&intents, &issues, "nope")
            .render()
            .contains("nope is not an intent"));
        let q = |v: &str| vec![("intent".to_string(), v.to_string())];
        assert_eq!(picked(&q("intent-a")), Some("intent-a".into()));
        assert_eq!(picked(&q("a\" onclick=\"x")), None);
    }
}
