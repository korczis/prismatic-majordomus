//! One rule in full, for the Governance screen's inspector (`/ui/governance?rule=<id>`): what
//! it states, how it is enforced, every test that proves it with the state of its proof, the
//! gates that run them, what is still missing to prove it, and the rules it depends on and
//! that depend on it, each a link to its own inspector.
//!
//! Read from `rules.show`, the capability that joins a rule to its proof; a rule the index
//! does not hold is its refusal, said as such.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::source::{ask_input, Answer};
use super::view::{
    code_link, none, object, object_or, panel, state, table, text, unanswered, value,
};

/// A rule id as the index writes them (`project.worktree-topology`, `majordomus.adr-integrity@1`):
/// letters, digits, `.`, `-`, `_` and `@`, and nothing a link could carry anything else in.
pub fn is_rule_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'@'))
}

/// The rule a query names, when it names one.
pub fn picked(query: &[(String, String)]) -> Option<String> {
    query
        .iter()
        .find(|(k, _)| k == "rule")
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| is_rule_id(v))
}

/// Where a rule's inspector is.
pub fn href(id: &str) -> String {
    format!("{}?rule={id}#rule", super::governance::ROUTE)
}

/// The id of a rule as the report or the index gives it: the rule object's `id`, or the
/// string itself when the report gives one.
pub fn id_of(rule: &Value) -> String {
    match rule {
        Value::String(s) => s.clone(),
        other => text(other, "id"),
    }
}

/// Ask `rules.show` for `id`.
pub fn ask(ctx: &Context, id: &str) -> Answer {
    ask_input(ctx, "rules.show", serde_json::json!({ "rule": id }))
}

fn chips(ids: Vec<String>) -> El {
    if ids.is_empty() {
        return el("span").class("mj-ui-dim").text("none");
    }
    el("span")
        .class("mj-ui-badges")
        .children(ids.into_iter().map(|id| {
            // a dependency is written with its version (`x@1`); the inspector takes either
            el("a")
                .class("mj-ui-chip mj-ui-chip--link")
                .attr("href", href(&id))
                .child(el("code").class("mj-ui-chip-value").text(id))
        }))
}

fn strings(v: &Value, k: &str) -> Vec<String> {
    v.get(k)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    x.as_str()
                        .map(str::to_string)
                        .or_else(|| x.get("id").and_then(Value::as_str).map(str::to_string))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The inspector of the rule `answer` is about.
pub fn inspector(answer: &Answer, id: &str) -> El {
    let body = unanswered(answer).unwrap_or_else(|| {
        // `unanswered` has spoken for an answer that failed; this one answered
        let a = answer.ok().cloned().unwrap_or(Value::Null);
        let proof = a.get("proof").cloned().unwrap_or(Value::Null);
        let rule = proof.get("rule").cloned().unwrap_or(Value::Null);
        let enforcement = rule.get("enforcement").cloned().unwrap_or(Value::Null);
        let fact = |label: &str, value: El| {
            el("div")
                .class("mj-ui-fact")
                .child(el("dt").text(label))
                .child(el("dd").child(value))
        };
        let tests = proof
            .get("tests")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|t| {
                let path = text(t, "path");
                // a test the report found on disk is an object of the index: its technical
                // view; one it did not find is its path, said as text
                let test = if path.is_empty() {
                    none()
                } else if t.get("present").and_then(Value::as_bool) == Some(true) {
                    object(&format!("majordomus://test/{path}"), path)
                } else {
                    el("span")
                        .class("mj-ui-mono")
                        .text(format!("{path} (not found)"))
                };
                el("tr")
                    .child(el("td").child(test))
                    .child(el("td").child(state(t, "state")))
                    .child(el("td").class("mj-ui-dim").text(value(t, "meaning")))
            })
            .collect::<Vec<_>>();
        let missing = text(&a, "missing");
        el("section")
            .class("mj-ui-detail")
            .attr("id", "rule")
            .attr("aria-label", format!("Rule {id}"))
            .child(
                el("div")
                    .class("mj-ui-detail-head")
                    .child(
                        el("p")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(value(&rule, "identity")),
                    )
                    .child(
                        el("h3")
                            .class("mj-ui-detail-title")
                            .text(value(&rule, "title")),
                    )
                    .child(
                        el("p")
                            .class("mj-ui-badges")
                            .child(state(&rule, "class"))
                            .child(state(&rule, "status"))
                            .child(state(&proof, "state"))
                            .child(
                                el("a")
                                    .class("mj-ui-more")
                                    .attr("href", super::governance::ROUTE)
                                    .text("Close"),
                            ),
                    ),
            )
            .child(super::view::lead(&text(&rule, "statement")))
            .child(el("p").class("mj-ui-line").text(text(&proof, "meaning")))
            .child(
                el("dl")
                    .class("mj-ui-facts")
                    .child(fact(
                        "Enforced",
                        el("span").class("mj-ui-mono").text(format!(
                            "{} · {}",
                            Some(text(&enforcement, "mode"))
                                .filter(|s| !s.is_empty())
                                .unwrap_or_else(|| "not said".into()),
                            Some(text(&enforcement, "validator"))
                                .filter(|s| !s.is_empty())
                                .unwrap_or_else(|| "no validator".into())
                        )),
                    ))
                    .child(fact("Gates", {
                        // each gate leads to its row in the CI model
                        let g = strings(&proof, "gates");
                        if g.is_empty() {
                            none()
                        } else {
                            el("span").class("mj-ui-badges").children(
                                g.into_iter()
                                    .map(|id| code_link(super::delivery::gate_href(&id), id)),
                            )
                        }
                    }))
                    .child(fact("Depends on", chips(strings(&a, "depends_on"))))
                    .child(fact("Required by", chips(strings(&a, "required_by"))))
                    .child(fact("Declared in", {
                        // the rule's own record, which the index holds: rules.show found it
                        object_or(&text(&rule, "uri"), &text(&rule, "path"))
                    })),
            )
            .child(el("h4").class("mj-ui-panel-title").text("What proves it"))
            .child(table(
                &["Test", "State", "What that means"],
                tests,
                "It names no test.",
            ))
            .when(!missing.is_empty(), |s| {
                s.child(el("h4").class("mj-ui-panel-title").text("To prove it"))
                    .child(el("code").class("mj-ui-mono").text(missing))
            })
    });
    panel(
        "Rule",
        "One rule of the repository, with what proves it and what is still missing.",
        body,
        &[answer],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::source::{Exposure, Source};
    use std::time::Duration;

    fn answer(value: Result<Value, String>) -> Answer {
        Answer {
            source: Source::of("rules.show"),
            route: Some("GET /api/v1/rules/rule".into()),
            exposure: Exposure::default(),
            value,
            took: Duration::from_millis(1),
            standing: crate::ui::source::Standing::NOW,
        }
    }

    #[test]
    fn a_rule_shows_its_proof_and_leads_to_the_rules_around_it() {
        let a = answer(Ok(serde_json::json!({
            "proof": {
                "rule": { "id": "project.a", "identity": "project.a@1", "title": "A holds", "statement": "A is true.",
                          "class": "blocking", "status": "active", "path": ".ai/repo/rules/project/a.v1.md",
                          "uri": "majordomus://rule/project.a@1",
                          "enforcement": { "mode": "dispatched", "validator": "adr" } },
                "tests": [{ "path": "test/cases/96_a.sh", "present": true, "state": "not_run", "meaning": "never run" },
                          { "path": "test/cases/97_gone.sh", "present": false, "state": "missing" }],
                "gates": ["shell-suite"], "state": "not_run", "meaning": "No run is recorded."
            },
            "depends_on": ["project.b@1"], "required_by": [{ "id": "project.c" }],
            "missing": "Run and record: bash test/run.sh 96_a"
        })));
        let html = inspector(&a, "project.a").render();
        for want in [
            "id=\"rule\"",
            "A is true.",
            "test/cases/96_a.sh",
            "shell-suite",
            "/ui/governance?rule=project.b@1#rule",
            "/ui/governance?rule=project.c#rule",
            "bash test/run.sh 96_a",
            // every entity it names is a link: the gate to its row in the CI model, the test
            // and the declaration to their technical views
            "href=\"/ui/delivery#gate-shell-suite\"",
            "href=\"/cockpit/object?uri=majordomus%3A%2F%2Ftest%2Ftest%2Fcases%2F96_a.sh\"",
            "href=\"/cockpit/object?uri=majordomus%3A%2F%2Frule%2Fproject.a%401\"",
            // a test the report did not find is never a link to a page that would answer 404
            "test/cases/97_gone.sh (not found)",
        ] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        assert!(!html.contains("97_gone.sh\""), "{html}");
    }

    /// A rule its index says little about still opens: no dependency either way, no
    /// enforcement named, no gate, each said rather than left blank.
    #[test]
    fn a_rule_that_says_little_says_what_it_lacks() {
        let html = inspector(
            &answer(Ok(serde_json::json!({
                "proof": { "rule": { "id": "project.z", "title": "Z" }, "tests": [], "state": "not_run" }
            }))),
            "project.z",
        )
        .render();
        for want in ["not said", "no validator", "It names no test.", ">none<"] {
            assert!(html.contains(want), "no {want}: {html}");
        }
    }

    #[test]
    fn an_unknown_rule_is_its_refusal_and_a_hostile_id_is_never_asked() {
        let refused = inspector(&answer(Err("not found: no rule nope".into())), "nope").render();
        assert!(refused.contains("no rule nope") && !refused.contains("id=\"rule\""));
        let q = |v: &str| vec![("rule".to_string(), v.to_string())];
        assert_eq!(
            picked(&q("majordomus.adr-integrity@1")),
            Some("majordomus.adr-integrity@1".into())
        );
        assert_eq!(picked(&q("x\" onclick=\"y")), None);
        assert_eq!(id_of(&serde_json::json!({ "id": "p.q" })), "p.q");
        assert_eq!(id_of(&serde_json::json!("p.r")), "p.r");
    }
}
