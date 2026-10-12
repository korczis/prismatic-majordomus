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
use super::view::{more, nothing, object_if, panel, state, table, text, unanswered, value};

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

/// How many of the ledger's runs of a claim's test the inspector lists, newest first.
const RUNS: usize = 10;

/// Seconds as a run recorded them, or `unknown` when it recorded none.
fn seconds(run: &Value) -> String {
    run.get("seconds")
        .filter(|s| !s.is_null())
        .map_or_else(|| "unknown".into(), Value::to_string)
}

/// The first ten characters of a run's commit, or `unknown` when it recorded none.
fn commit(run: &Value) -> String {
    let c = text(run, "commit");
    match c.get(..10) {
        Some(short) => short.to_string(),
        None if c.is_empty() => "unknown".into(),
        None => c,
    }
}

/// The inspector of the claim `answer` is about. The document that says it, the code that
/// implements it and the test that settles it lead to their technical views when the index
/// holds them (`ctx` asks it).
pub fn inspector(ctx: &Context, answer: &Answer, id: &str) -> El {
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
                    seconds(&run),
                    commit(&run),
                    value(&run, "working_tree")
                )))
        };
        let runs = c
            .get("executions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let history = runs
            .iter()
            .rev()
            .take(RUNS)
            .map(|e| {
                el("tr")
                    .child(el("td").child(word_badge(&text(e, "outcome"))))
                    .child(el("td").class("mj-ui-mono").text(seconds(e)))
                    .child(el("td").class("mj-ui-mono").text(commit(e)))
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(value(e, "working_tree")),
                    )
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(value(e, "at").replace('T', " ")),
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
                    .child(fact("Said in", {
                        let source = text(&c, "source");
                        object_if(ctx, &format!("majordomus://document/{source}"), &source)
                    }))
                    .child(fact("Implemented by", {
                        let code = text(&c, "implementation");
                        object_if(ctx, &format!("majordomus://implementation/{code}"), &code)
                    }))
                    .child(fact("Settled by", {
                        let test = text(&c, "test_path");
                        object_if(ctx, &format!("majordomus://test/{test}"), &test)
                    }))
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
            .children(more(RUNS.min(runs.len()), runs.len(), None))
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
    use crate::synthetic::SyntheticRepository;
    use crate::ui::source::{Exposure, Source};
    use std::time::Duration;

    fn repo() -> SyntheticRepository {
        SyntheticRepository::small().expect("a synthetic repository")
    }

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
        let repo = repo();
        let ctx = repo.context().expect("a context");
        let html = inspector(
            &ctx,
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
            &ctx,
            &answer(Ok(
                serde_json::json!({ "id": "c-3", "claim": "Timed nowhere",
                "execution": { "outcome": "pass", "commit": "0123456789abcdef" } }),
            )),
            "c-3",
        )
        .render();
        assert!(html.contains("0123456789"), "{html}");
        assert!(
            html.contains(" unknown s · 0123456789 · none"),
            "a run without its seconds or its tree says so: {html}"
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
        let repo = repo();
        let ctx = repo.context().expect("a context");
        let html = inspector(&ctx, &a, "c-1").render();
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

    /// What the claim names leads to its technical view when the index holds it, and is
    /// text when it does not, so no link answers 404; more runs than it lists are counted.
    #[test]
    fn what_a_claim_names_is_a_link_when_the_index_holds_it() {
        let repo = repo();
        let ctx = repo.context().expect("a context");
        let runs: Vec<Value> = (0..12)
            .map(|i| serde_json::json!({ "outcome": "pass", "seconds": i, "commit": "abcdefabcdef", "at": format!("2026-10-{:02}T00:00:00Z", i + 1) }))
            .collect();
        let html = inspector(
            &ctx,
            &answer(Ok(serde_json::json!({
                "id": "c-4", "claim": "Linked", "source": "docs/DOC_0.md",
                "test_path": "test/cases/no_such_case.sh", "executions": runs
            }))),
            "c-4",
        )
        .render();
        assert!(
            html.contains("href=\"/cockpit/object?uri=majordomus%3A%2F%2Fdocument%2Fdocs%2FDOC_0.md\""),
            "the document the index holds is a link: {html}"
        );
        assert!(
            html.contains("test/cases/no_such_case.sh") && !html.contains("no_such_case.sh\""),
            "a test the index does not hold is text: {html}"
        );
        assert!(html.contains("The first 10 of 12; 2 more"), "{html}");
    }

    #[test]
    fn a_claim_the_matrix_does_not_declare_is_said_as_its_refusal_and_a_hostile_id_never_asked() {
        let repo = repo();
        let ctx = repo.context().expect("a context");
        let refused =
            inspector(&ctx, &answer(Err("not found: no claim nope".into())), "nope").render();
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
