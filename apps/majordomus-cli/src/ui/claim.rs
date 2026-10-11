//! One claim in full, for the Evidence screen's inspector (`/ui/evidence?claim=<id>`): what it
//! says and where, the state its proof is in and what that state means, the run behind it,
//! what changed since, how to see it again, every run the ledger holds for it, and the other
//! claims the same test proves, each a link to its own inspector.
//!
//! Read from `evidence.claim`, the one capability that joins a claim to its execution; a claim
//! the matrix does not declare is its refusal, said as such, never an empty proof.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};
use crate::cockpit::view::word_badge;

use super::source::{ask_input, Answer};
use super::view::{nothing, panel, state, table, text, unanswered};

/// A claim id as the matrix spells them (`a-finding-names-how-to-see-it-again`): letters,
/// digits, `-`, `_` and `.`, and nothing a link could carry anything else in.
pub fn is_claim_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// The claim a query names, when it names one.
pub fn picked(query: &[(String, String)]) -> Option<String> {
    query
        .iter()
        .find(|(k, _)| k == "claim")
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| is_claim_id(v))
}

/// Where a claim's inspector is.
pub fn href(id: &str) -> String {
    format!("{}?claim={id}#claim", super::evidence::ROUTE)
}

/// Ask `evidence.claim` for `id`.
pub fn ask(ctx: &Context, id: &str) -> Answer {
    ask_input(ctx, "evidence.claim", serde_json::json!({ "claim": id }))
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

fn mono(t: String) -> El {
    el("span").class("mj-ui-mono").text(t)
}

/// The inspector of the claim `answer` is about.
pub fn inspector(answer: &Answer, id: &str) -> El {
    let body = unanswered(answer).unwrap_or_else(|| {
        // `unanswered` has spoken for an answer that failed; this one answered
        let c = answer.ok().cloned().unwrap_or(Value::Null);
        let fact = |label: &str, value: El| {
            el("div")
                .class("mj-ui-fact")
                .child(el("dt").text(label))
                .child(el("dd").child(value))
        };
        let run = c.get("execution").cloned().unwrap_or(Value::Null);
        let last = if run.is_null() {
            el("span").class("mj-ui-dim").text("never run")
        } else {
            el("span")
                .child(word_badge(&text(&run, "outcome")))
                .child(mono(format!(
                    " {} s · {} · {}",
                    run.get("seconds")
                        .map(Value::to_string)
                        .unwrap_or_else(|| "?".into()),
                    text(&run, "commit").get(..10).unwrap_or_default(),
                    text(&run, "working_tree")
                )))
        };
        let history = c
            .get("executions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .rev()
            .take(10)
            .map(|e| {
                el("tr")
                    .child(el("td").child(word_badge(&text(e, "outcome"))))
                    .child(
                        el("td")
                            .class("mj-ui-mono")
                            .text(e.get("seconds").map(Value::to_string).unwrap_or_default()),
                    )
                    .child(
                        el("td")
                            .class("mj-ui-mono")
                            .text(text(e, "commit").get(..10).unwrap_or_default().to_string()),
                    )
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(text(e, "working_tree")),
                    )
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(text(e, "at").replace('T', " ")),
                    )
            })
            .collect::<Vec<_>>();
        let changed = strings(&c, "changed");
        let also = strings(&c, "also_proves");
        let reproduce = text(&c, "reproduce");
        el("section")
            .class("mj-ui-detail")
            .attr("id", "claim")
            .attr("aria-label", format!("Claim {id}"))
            .child(
                el("div")
                    .class("mj-ui-detail-head")
                    .child(el("p").class("mj-ui-mono mj-ui-dim").text(id.to_string()))
                    .child(el("h3").class("mj-ui-detail-title").text(text(&c, "claim")))
                    .child(
                        el("p")
                            .class("mj-ui-badges")
                            .child(state(&c, "state"))
                            .child(state(&c, "status"))
                            .child(
                                el("a")
                                    .class("mj-ui-more")
                                    .attr("href", super::evidence::ROUTE)
                                    .text("Close"),
                            ),
                    ),
            )
            .child(el("p").class("mj-ui-lead").text(text(&c, "meaning")))
            .child(
                el("dl")
                    .class("mj-ui-facts")
                    .child(fact("Said in", mono(text(&c, "source"))))
                    .child(fact("Implemented by", mono(text(&c, "implementation"))))
                    .child(fact("Settled by", mono(text(&c, "test_path"))))
                    .child(fact("Last run", last))
                    .child(fact(
                        "Reproduce",
                        if reproduce.is_empty() {
                            el("span").class("mj-ui-dim").text("none")
                        } else {
                            el("code").class("mj-ui-mono").text(reproduce)
                        },
                    )),
            )
            .child(
                el("h4")
                    .class("mj-ui-panel-title")
                    .text("Changed since it was proven"),
            )
            .child(if changed.is_empty() {
                nothing("Nothing it names has changed since its proof.")
            } else {
                el("ul")
                    .class("mj-ui-list")
                    .children(changed.into_iter().map(|p| el("li").child(mono(p))))
            })
            .child(
                el("h4")
                    .class("mj-ui-panel-title")
                    .text("Also proved by the same test"),
            )
            .child(if also.is_empty() {
                nothing("The test proves this claim alone.")
            } else {
                el("p").class("mj-ui-badges").children(also.iter().map(|a| {
                    el("a")
                        .class("mj-ui-chip mj-ui-chip--link")
                        .attr("href", href(a))
                        .child(el("code").class("mj-ui-chip-value").text(a.clone()))
                }))
            })
            .child(
                el("h4")
                    .class("mj-ui-panel-title")
                    .text("Runs the ledger holds"),
            )
            .child(table(
                &["Outcome", "Seconds", "Commit", "Tree", "At"],
                history,
                "The ledger holds no run of its test.",
            ))
    });
    panel(
        "Claim",
        "One claim of the product, with what proves it and how to see it again.",
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
            source: Source::of("evidence.claim"),
            route: Some("GET /api/v1/evidence/claim".into()),
            exposure: Exposure::default(),
            value,
            took: Duration::from_millis(1),
        }
    }

    /// A claim that never ran, names no reproduction, changed nothing since and shares its
    /// test with no other claim says each of those, rather than drawing an empty field.
    #[test]
    fn a_claim_never_run_says_what_it_lacks() {
        let html = inspector(
            &answer(Ok(
                serde_json::json!({ "id": "c-2", "claim": "Alone", "state": "not_run" }),
            )),
            "c-2",
        )
        .render();
        for want in [
            "never run",
            ">none<",
            "Nothing it names has changed since its proof.",
            "The test proves this claim alone.",
        ] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        let html = inspector(
            &answer(Ok(
                serde_json::json!({ "id": "c-3", "claim": "Timed nowhere",
                "execution": { "outcome": "pass", "commit": "0123456789abcdef" } }),
            )),
            "c-3",
        )
        .render();
        assert!(html.contains("0123456789"), "{html}");
        assert!(
            html.contains("?"),
            "a run without its seconds says so: {html}"
        );
    }

    #[test]
    fn a_claim_shows_its_proof_what_changed_and_the_claims_its_test_also_proves() {
        let a = answer(Ok(serde_json::json!({
            "id": "c-1", "claim": "A finding names how to see it again", "status": "guaranteed",
            "source": "docs/DOCTRINE.md", "implementation": "scripts/x", "test_path": "test/cases/138.sh",
            "state": "stale", "meaning": "The proof is older than what it is about.",
            "execution": { "outcome": "pass", "seconds": 6, "commit": "a04b65c9110a74dc", "working_tree": "dirty" },
            "changed": ["docs/DOCTRINE.md"], "reproduce": "bash test/run.sh 138",
            "executions": [{ "outcome": "fail", "seconds": 5, "commit": "1111111111aa", "working_tree": "clean", "at": "2026-10-01T00:00:00Z" },
                           { "outcome": "pass", "seconds": 6, "commit": "a04b65c9110a", "working_tree": "dirty", "at": "2026-10-02T00:00:00Z" }],
            "also_proves": ["authored-files-are-english"]
        })));
        let html = inspector(&a, "c-1").render();
        for want in [
            "id=\"claim\"",
            "The proof is older than what it is about.",
            "bash test/run.sh 138",
            "docs/DOCTRINE.md",
            "/ui/evidence?claim=authored-files-are-english#claim",
            "a04b65c911",
        ] {
            assert!(html.contains(want), "no {want}: {html}");
        }
        match (html.find("2026-10-02"), html.find("2026-10-01")) {
            (Some(newer), Some(older)) => assert!(newer < older, "the newest run first"),
            other => panic!("both runs are listed: {other:?}"),
        }
    }

    #[test]
    fn a_claim_the_matrix_does_not_declare_is_said_as_its_refusal_and_a_hostile_id_never_asked() {
        let refused = inspector(&answer(Err("not found: no claim nope".into())), "nope").render();
        assert!(refused.contains("not found: no claim nope"), "{refused}");
        assert!(!refused.contains("id=\"claim\""));
        let q = |v: &str| vec![("claim".to_string(), v.to_string())];
        assert_eq!(
            picked(&q("a-finding.names_it")),
            Some("a-finding.names_it".into())
        );
        assert_eq!(picked(&q("x\" onclick=\"y")), None);
    }
}
