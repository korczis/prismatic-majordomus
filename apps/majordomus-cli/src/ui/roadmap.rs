//! Roadmap: the milestones in the plan's own order, the intents they serve, and the issues
//! they hold. Read from `plan.roadmap`, `plan.status` and `intents.list`, and from
//! `plan.flight` for where the work is (ADR 0166, decision 4):
//!
//! - **Now** is the milestones with work in flight (an open pull request, a live mesh claim,
//!   an ACTIVE issue), never merely the first open one in the plan's order, and **Next** is
//!   what the graph allows once that work lands.
//! - Every issue in flight, and every issue of an inspected milestone, shows its declared
//!   status beside its inferred stage, linked to the objects that prove it: the pull
//!   request, the merge commit, the release tag, the published `latest.json`. A source that
//!   was not read says so, with its reason.
//! - The latest release is read from the releases when the screen is drawn — the records,
//!   the tags and the published `latest.json` — so a snapshot never shows a release older
//!   than the executable that drew it.
//!
//! Milestones carry no dates in this repository's plan, so the roadmap is ordered by the
//! plan's dependency graph and drawn as a sequence, never as a calendar: a timeline with
//! invented dates would be a claim nobody made.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};
use crate::cockpit::view::word_badge;

use super::dashboard::{issues_panel, milestone_row};
use super::shell::View;
use super::source::{ask_all, ask_input, Answer, Source};
use super::view::{
    graph, hero, kpi, kpi_count, list, nothing, panel, shown, state, status_tone, table, text,
    unanswered, value, wide, GraphNode, Layout,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/roadmap";

const ROADMAP: Source = Source::of("plan.roadmap");
const PLAN: Source = Source::of("plan.status");
const INTENTS: Source = Source::of("intents.list");

const ISSUES: Source = Source::of("plan.issues");

/// Where the work is, the next milestone and the latest release: live, read when the
/// screen is drawn.
const FLIGHT: Source = Source::of("plan.flight");

/// The Roadmap screen; `query` may pick one milestone to inspect (`?milestone=M004`).
pub fn screen(ctx: &Context, query: &[(String, String)]) -> View {
    let picked = super::milestone::picked(query);
    let intent = super::intent::picked(query);
    // the issues are asked only when a milestone or an intent is inspected, their one reader
    let mut sources = vec![ROADMAP, PLAN, INTENTS];
    if picked.is_some() || intent.is_some() {
        sources.push(ISSUES);
    }
    // a milestone that is inspected is asked of `plan.flight` with every one of its issues;
    // otherwise where the work is is asked beside the other sources, at once
    let inspecting = picked.is_some();
    if !inspecting {
        sources.push(FLIGHT);
    }
    let mut answers = ask_all(ctx, &sources);
    if let Some(id) = &picked {
        let declared = answers[&ROADMAP]
            .ok()
            .and_then(|v| v.pointer("/milestones"))
            .and_then(Value::as_array)
            .is_some_and(|ms| ms.iter().any(|m| text(m, "id") == *id));
        let input = if declared {
            serde_json::json!({ "milestone": id })
        } else {
            serde_json::json!({})
        };
        answers.insert(FLIGHT, ask_input(ctx, FLIGHT.capability, input));
    }
    let (roadmap, plan, intents) = (&answers[&ROADMAP], &answers[&PLAN], &answers[&INTENTS]);
    let flight = &answers[&FLIGHT];

    let top = hero(
        "target",
        "violet",
        "Roadmap",
        "From intent to impact, in the order the plan's dependencies allow.",
        "Every milestone of the plan with the issues it holds, and the intents the work serves.",
        vec![
            kpi_count("Now: milestones in flight", &flight.read("/now")),
            kpi("Next", &flight.read("/next")),
            kpi("Latest release", &flight.read("/release/latest")),
            kpi_count("Milestones", &roadmap.read("/milestones")),
            kpi_count("Intents", &intents.read("/intents")),
        ],
    );

    let sequence = unanswered(roadmap).unwrap_or_else(|| {
        let rows = list(roadmap, "/milestones")
            .iter()
            .map(milestone_row)
            .collect::<Vec<_>>();
        table(
            &["Milestone", "State", "Issues done"],
            rows,
            "The plan holds no milestone.",
        )
    });

    let dependencies = unanswered(roadmap).unwrap_or_else(|| {
        let rows = list(roadmap, "/milestones")
            .iter()
            .filter(|m| {
                m.get("depends_on")
                    .and_then(Value::as_array)
                    .is_some_and(|d| !d.is_empty())
            })
            .map(|m| {
                let on = |key: &str| {
                    m.get(key)
                        .and_then(Value::as_array)
                        .map(|d| {
                            d.iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default()
                };
                el("tr")
                    .child(el("td").class("mj-ui-mono").text(text(m, "id")))
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(on("depends_on")),
                    )
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(on("blocked_by")),
                    )
            })
            .collect::<Vec<_>>();
        table(
            &["Milestone", "Depends on", "Blocked by"],
            rows,
            "No milestone depends on another.",
        )
    });

    // the same edges as a graph: every milestone a node in the tone of its status, every
    // dependency the plan records an edge, the table above the fallback without a script
    let dependencies = match unanswered(roadmap) {
        Some(failed) => failed,
        None => {
            let milestones = list(roadmap, "/milestones");
            let nodes: Vec<GraphNode> = milestones
                .iter()
                .map(|m| {
                    let id = text(m, "id");
                    GraphNode {
                        label: id.clone(),
                        tone: status_tone(&text(m, "status")),
                        href: super::milestone::href(&id),
                        kind: "node",
                        id,
                    }
                })
                .collect();
            let edges: Vec<(String, String)> = milestones
                .iter()
                .flat_map(|m| {
                    let to = text(m, "id");
                    m.get("depends_on")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(move |d| d.as_str().map(|from| (from.to_string(), to.clone())))
                })
                .collect();
            graph(
                "The milestones of the plan and the dependencies between them",
                Layout::Directed,
                &nodes,
                &edges,
                dependencies,
            )
        }
    };

    let intents_body = unanswered(intents).unwrap_or_else(|| {
        let rows = list(intents, "/intents")
            .iter()
            .map(|i| {
                el("tr")
                    .child(
                        el("td").child(
                            el("a")
                                .class("mj-ui-row-link")
                                .attr("href", super::intent::href(&text(i, "id")))
                                .child(el("span").class("mj-ui-mono mj-ui-dim").text(text(i, "id")))
                                .child(el("span").class("mj-ui-cell-title").text(text(i, "title"))),
                        ),
                    )
                    .child(el("td").child(state(i, "stage")))
                    .child(el("td").child(state(i, "verdict")))
            })
            .collect::<Vec<_>>();
        table(
            &["Intent", "Stage", "Verdict"],
            rows,
            "No intent is recorded.",
        )
    });

    let mut grid = el("div").class("mj-ui-grid");
    if let (Some(id), Some(issues)) = (&intent, answers.get(&ISSUES)) {
        grid = grid.child(wide(super::intent::inspector(intents, issues, id)));
    }
    if let (Some(id), Some(issues)) = (&picked, answers.get(&ISSUES)) {
        // the milestone's own record, for the prose no plan summary carries
        let record = super::source::ask_input(
            ctx,
            "objects.get",
            serde_json::json!({ "uri": format!("majordomus://milestone/{id}") }),
        );
        grid = grid.child(wide(super::milestone::inspector_with(
            roadmap, issues, &record, id,
        )));
        grid = grid.child(wide(delivery_panel(flight, id)));
    }
    let grid = grid
        .child(wide(now_panel(flight)))
        .child(wide(panel(
            "Milestones",
            "Every milestone, in the plan's order; the plan declares no dates, so none are drawn.",
            sequence,
            &[roadmap],
        )))
        .child(panel(
            "Intents",
            "What the work is for, and whether the evidence says it is met.",
            intents_body,
            &[intents],
        ))
        .child(issues_panel(plan))
        .child(release_panel(flight))
        .child(wide(panel(
            "Dependencies",
            "Which milestone waits for which, as the plan records it.",
            dependencies,
            &[roadmap],
        )));

    View::new(
        "Roadmap",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

/// The milestones with work in flight, each with the issues that put it there: why, the
/// declared status beside the inferred stage, and what proves the stage.
fn now_panel(flight: &Answer) -> El {
    let body = unanswered(flight).unwrap_or_else(|| {
        let milestones = list(flight, "/milestones");
        let mut body = el("div").child(el("p").class("mj-ui-line").text(format!(
            "Now {} · next {}",
            joined(&list(flight, "/now"), "nothing in flight"),
            shown(&flight.read("/next"))
        )));
        if milestones.is_empty() {
            body = body.child(nothing(
                "No work is in flight: no open pull request, live mesh claim or ACTIVE issue \
                 names an open issue of an open milestone.",
            ));
        }
        for m in &milestones {
            let id = text(m, "id");
            body = body
                .child(
                    el("h3").class("mj-ui-detail-title").child(
                        el("a")
                            .class("mj-ui-row-link")
                            .attr("href", super::milestone::href(&id))
                            .child(el("span").class("mj-ui-mono mj-ui-dim").text(id.clone()))
                            .child(el("span").class("mj-ui-cell-title").text(text(m, "title"))),
                    ),
                )
                .child(issue_table(m, true));
        }
        body.children(unread_sources(flight))
    });
    panel(
        "Now: where the work is",
        "The milestones holding an issue with an open pull request, a live mesh claim or an \
         ACTIVE status, in the plan's order; next is the first one the graph allows after them.",
        body,
        &[flight],
    )
}

/// Every issue of the inspected milestone, its declared status beside its inferred stage.
fn delivery_panel(flight: &Answer, id: &str) -> El {
    let body = unanswered(flight).unwrap_or_else(|| {
        match flight.ok().and_then(|v| v.pointer("/inspected")) {
            Some(m) => el("div")
                .child(issue_table(m, false))
                .children(unread_sources(flight)),
            None => nothing(&format!("{id} is not a milestone of this plan.")),
        }
    });
    panel(
        &format!("Delivery of {id}"),
        "Each issue's declared status beside how far its work has travelled, and what proves \
         it.",
        body,
        &[flight],
    )
}

/// A table of a milestone's issues: declared, inferred, why in flight (when `why`), proof.
fn issue_table(m: &Value, why: bool) -> El {
    let issues = m
        .get("issues")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let rows = issues
        .iter()
        .map(|i| {
            let id = text(i, "issue");
            let unread = strings(i, "unread");
            let stage =
                el("td")
                    .child(word_badge(&value(i, "stage")))
                    .when(!unread.is_empty(), |td| {
                        td.child(
                            el("span")
                                .class("mj-ui-dim")
                                .text(format!(" unread: {}", unread.join(", "))),
                        )
                    });
            let row = el("tr")
                .child(
                    el("td").child(
                        el("a")
                            .class("mj-ui-row-link")
                            .attr("href", format!("{}?issue={id}#issue", super::prs::ROUTE))
                            .child(el("span").class("mj-ui-mono mj-ui-dim").text(id.clone()))
                            .child(el("span").class("mj-ui-cell-title").text(text(i, "title"))),
                    ),
                )
                .child(el("td").child(state(i, "status")))
                .child(stage);
            let row = if why {
                row.child(
                    el("td")
                        .class("mj-ui-mono mj-ui-dim")
                        .text(strings(i, "in_flight").join(", ")),
                )
            } else {
                row
            };
            row.child(el("td").child(proof_list(i)))
        })
        .collect::<Vec<_>>();
    let headers: &[&str] = if why {
        &["Issue", "Declared", "Inferred", "In flight by", "Proof"]
    } else {
        &["Issue", "Declared", "Inferred", "Proof"]
    };
    table(headers, rows, "The milestone holds no issue.")
}

/// The objects that prove an issue's stage: a link where the forge has the object, the name
/// alone where only this clone (or the mesh) does.
fn proof_list(i: &Value) -> El {
    let proofs = i
        .get("proofs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if proofs.is_empty() {
        return el("span").class("mj-ui-dim").text("nothing names it");
    }
    let mut out = el("span");
    for (n, p) in proofs.iter().enumerate() {
        if n > 0 {
            out = out.text(" · ");
        }
        let label = format!("{} {}", text(p, "kind").replace('_', " "), text(p, "label"));
        out = match (text(p, "kind").as_str(), text(p, "href")) {
            ("claim", _) => out.child(super::view::link(super::mesh::ROUTE, label)),
            (_, href) if !href.is_empty() => out.child(
                el("a")
                    .class("mj-ui-link")
                    .attr("href", href)
                    .attr("rel", "noopener noreferrer")
                    .text(label),
            ),
            _ => out.child(el("span").class("mj-ui-mono").text(label)),
        };
    }
    out
}

/// The latest release, and what each source of it answered.
fn release_panel(flight: &Answer) -> El {
    let body = unanswered(flight).unwrap_or_else(|| {
        let rows = list(flight, "/release/readings")
            .iter()
            .map(|r| {
                let detail = text(r, "detail");
                let href = text(r, "href");
                el("tr")
                    .child(el("td").class("mj-ui-mono").text(text(r, "source")))
                    .child(el("td").child(state(r, "state")))
                    .child(el("td").class("mj-ui-mono").text(value(r, "version")))
                    .child(el("td").child(if href.is_empty() {
                        el("span").class("mj-ui-dim").text(detail)
                    } else {
                        el("a")
                            .class("mj-ui-link mj-ui-mono")
                            .attr("href", href)
                            .attr("rel", "noopener noreferrer")
                            .text(detail)
                    }))
            })
            .collect::<Vec<_>>();
        // an answered flight always says whether this executable is tagged
        let tagged = match flight.read("/release/executable_tagged").text().as_deref() {
            Some("true") => "is tagged",
            _ => "is not tagged",
        };
        el("div")
            .child(el("p").class("mj-ui-line").text(format!(
                "Latest release {} (from {}) · this executable {} {tagged}",
                shown(&flight.read("/release/latest")),
                shown(&flight.read("/release/source")),
                shown(&flight.read("/release/executable")),
            )))
            .child(table(
                &["Source", "State", "Version", "Read from"],
                rows,
                "No source of releases was asked.",
            ))
    });
    panel(
        "Latest release",
        "The highest stable version the release records, the tags and the published \
         latest.json name, read when this screen was drawn.",
        body,
        &[flight],
    )
}

/// The sources `plan.flight` could not read, each with why: an unread source is said, so
/// "nothing in flight" is never mistaken for "nothing was looked at".
fn unread_sources(flight: &Answer) -> Option<El> {
    let unread: Vec<Value> = list(flight, "/sources")
        .into_iter()
        .filter(|s| text(s, "state") == "unread")
        .collect();
    (!unread.is_empty()).then(|| {
        let mut ul = el("ul").class("mj-ui-prose-list");
        for s in &unread {
            ul = ul.child(
                el("li")
                    .child(word_badge("unread"))
                    .text(" ")
                    .child(el("code").class("mj-mono").text(text(s, "name")))
                    .text(format!(": {}", text(s, "detail"))),
            );
        }
        ul
    })
}

/// A list of strings joined, or `empty` when there are none.
fn joined(items: &[Value], empty: &str) -> String {
    let words: Vec<&str> = items.iter().filter_map(Value::as_str).collect();
    if words.is_empty() {
        empty.to_string()
    } else {
        words.join(", ")
    }
}

fn strings(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `?intent=` opens the intent's inspector on the Roadmap screen itself, asking the plan's
    /// issues for what serves it.
    #[test]
    fn the_screen_opens_an_intent_it_is_asked_for() {
        let repo = crate::synthetic::SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &[("intent".to_string(), "intent-nope".to_string())])
            .main
            .render();
        assert!(html.contains("intent-nope is not an intent"), "{html}");
    }
    use crate::capability::{builtin, CapabilityRegistry};
    use crate::synthetic::SyntheticRepository;

    /// A context over `repo` whose registry holds every built-in module except those named:
    /// a screen asked there meets a capability that cannot answer, as it would on a server
    /// whose registry lost one.
    fn context_without(repo: &SyntheticRepository, absent: &[&str]) -> std::sync::Arc<Context> {
        let index = repo.index().expect("an index");
        let modules = builtin::modules()
            .into_iter()
            .filter(|m| !absent.contains(&m.id.as_str()))
            .collect();
        let registry = CapabilityRegistry::builder()
            .with_modules(modules)
            .with_index(&index)
            .build()
            .expect("a registry");
        std::sync::Arc::new(Context::new(
            std::sync::Arc::new(index),
            std::sync::Arc::new(registry),
        ))
    }

    /// `planned` with one intent that serves its first milestone.
    fn planned_with_an_intent() -> SyntheticRepository {
        let repo = SyntheticRepository::planned();
        let sources = repo.root().join(".ai/repo/knowledge/sources.yaml");
        let mut text = std::fs::read_to_string(&sources).expect("the sources");
        text.push_str(
            "  - id: intent\n    kind: intent\n    discovery: vcs\n    \
             pathspec: ':(glob).ai/repo/project/intents/*.yaml'\n    required: false\n",
        );
        std::fs::write(&sources, text).expect("the sources");
        let intent = repo.root().join(".ai/repo/project/intents/i-first.yaml");
        std::fs::create_dir_all(intent.parent().expect("a parent")).expect("a directory");
        std::fs::write(
            &intent,
            "id: i-first\ntitle: The first intent\nstatement: \"The first milestone is real.\"\n\
             invariants:\n  - It stays real\nmilestones:\n  - m-first\nsatisfaction:\n  - id: it-holds\n    \
             criterion: It holds\n    evidence: test\n    ref: tests/first.rs\n",
        )
        .expect("an intent");
        repo
    }

    /// Every dependency the plan records is a row and an edge, every milestone a node led to
    /// its row, and every intent a row with its stage and verdict.
    #[test]
    fn the_dependencies_are_a_table_and_a_graph_and_the_intents_are_listed() {
        let repo = planned_with_an_intent();
        let ctx = repo.context().expect("a context");
        let view = screen(&ctx, &[]);
        let html = view.main.render();
        assert_eq!(view.status, 200);
        // m-second depends on m-first: its row in the table, its edge in the graph
        assert!(html.contains(">m-second<"), "{html}");
        assert!(
            !html.contains("No milestone depends on another."),
            "a recorded dependency is a row: {html}"
        );
        assert!(
            html.contains("/ui/roadmap?milestone=m-first#milestone"),
            "a node opens its milestone's inspector"
        );
        assert!(html.contains("data-graph"), "{html}");
        assert!(
            html.contains("i-first") && html.contains("The first intent"),
            "{html}"
        );
        assert!(!html.contains("No intent is recorded."), "{html}");
    }

    /// A roadmap that cannot be asked says so in every panel that reads it, and draws no
    /// sequence, no table and no graph of its own.
    #[test]
    fn a_roadmap_that_cannot_answer_draws_no_sequence() {
        let repo = SyntheticRepository::planned();
        let ctx = context_without(&repo, &["plan", "intents"]);
        let html = screen(&ctx, &[]).main.render();
        assert!(html.contains("plan.roadmap could not answer"), "{html}");
        assert!(!html.contains("#milestone-m-first"), "{html}");
        assert!(
            !html.contains("data-graph"),
            "no graph is drawn from nothing: {html}"
        );
    }

    /// A plan with no dependency and no intent says so rather than drawing empty tables.
    #[test]
    fn a_plan_without_dependencies_or_intents_says_so() {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &[]).main.render();
        assert!(html.contains("No milestone depends on another."), "{html}");
        assert!(html.contains("No intent is recorded."), "{html}");
    }
}
