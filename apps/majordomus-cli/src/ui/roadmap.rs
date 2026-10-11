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

/// The Roadmap screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[ROADMAP, PLAN, INTENTS]);
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
                        href: format!("#milestone-{id}"),
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
                        el("td")
                            .child(el("span").class("mj-ui-mono mj-ui-dim").text(text(i, "id")))
                            .child(el("span").class("mj-ui-cell-title").text(text(i, "title"))),
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

    let grid = el("div")
        .class("mj-ui-grid")
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
