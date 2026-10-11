//! Roadmap: the milestones in the plan's own order, the intents they serve, and the issues
//! they hold. Read from `plan.roadmap`, `plan.status` and `intents.list`.
//!
//! Milestones carry no dates in this repository's plan, so the roadmap is ordered by the
//! plan's dependency graph and drawn as a sequence, never as a calendar: a timeline with
//! invented dates would be a claim nobody made.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::el;

use super::dashboard::{issues_panel, milestone_row};
use super::shell::View;
use super::source::{ask_all, Source};
use super::view::{
    graph, hero, kpi, kpi_count, list, panel, state, status_tone, table, text, unanswered, wide,
    GraphNode, Layout,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/roadmap";

const ROADMAP: Source = Source::of("plan.roadmap");
const PLAN: Source = Source::of("plan.status");
const INTENTS: Source = Source::of("intents.list");

const ISSUES: Source = Source::of("plan.issues");

/// The Roadmap screen; `query` may pick one milestone to inspect (`?milestone=M004`).
pub fn screen(ctx: &Context, query: &[(String, String)]) -> View {
    let picked = super::milestone::picked(query);
    let intent = super::intent::picked(query);
    // the issues are asked only when a milestone or an intent is inspected, their one reader
    let mut sources = vec![ROADMAP, PLAN, INTENTS];
    if picked.is_some() || intent.is_some() {
        sources.push(ISSUES);
    }
    let answers = ask_all(ctx, &sources);
    let (roadmap, plan, intents) = (&answers[&ROADMAP], &answers[&PLAN], &answers[&INTENTS]);

    let top = hero(
        "target",
        "violet",
        "Roadmap",
        "From intent to impact, in the order the plan's dependencies allow.",
        "Every milestone of the plan with the issues it holds, and the intents the work serves.",
        vec![
            kpi("Now", &roadmap.read("/now")),
            kpi("Next", &roadmap.read("/next")),
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
        grid = grid.child(wide(super::milestone::inspector(roadmap, issues, id)));
    }
    let grid = grid
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
