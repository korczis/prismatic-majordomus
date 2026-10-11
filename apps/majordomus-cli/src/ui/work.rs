//! Issues & PRs: the work of the plan and the pull requests that carry it. Read from
//! `plan.status` and `plan.issues`; the pull-request queue is the Dashboard's slow panel,
//! drawn here on its own route the same way.

use crate::capability::Context;
use crate::cockpit::html::el;

use super::dashboard::{issues_panel, PULL_REQUESTS_PANEL, ROWS};
use super::shell::View;
use super::source::{ask_all, Source};
use super::view::{hero, kpi, list, panel, state, table, text, unanswered, wide};

/// The route of the screen.
pub const ROUTE: &str = "/ui/work";

const PLAN: Source = Source::of("plan.status");
const ISSUES: Source = Source::of("plan.issues");

/// The Issues & PRs screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[PLAN, ISSUES]);
    let (plan, issues) = (&answers[&PLAN], &answers[&ISSUES]);

    let top = hero(
        "merge",
        "green",
        "Issues & Pull Requests",
        "From ready work to merged change.",
        "The issues the plan holds, which of them can start now, and what the integration lane would do with every open pull request.",
        vec![
            kpi("Issues", &plan.read("/counts/total")),
            kpi("Ready", &plan.read("/counts/by_status/READY")),
            kpi("Blocked", &plan.read("/counts/by_status/BLOCKED")),
            kpi("Done", &plan.read("/counts/by_status/DONE")),
        ],
    );

    let ready = issue_table(issues, "READY", "No issue is ready to start.");
    let active = issue_table(issues, "ACTIVE", "No issue is in progress.");

    let queue = el("section")
        .class("mj-ui-panel")
        .attr("aria-label", "Pull-request queue")
        .attr("data-panel-src", PULL_REQUESTS_PANEL)
        .attr("aria-busy", "false")
        .child(
            el("header")
                .class("mj-ui-panel-head")
                .child(
                    el("h2")
                        .class("mj-ui-panel-title")
                        .text("Pull-request queue"),
                )
                .child(
                    el("p")
                        .class("mj-ui-panel-sub")
                        .text("Asked on its own, once this screen has painted."),
                ),
        )
        .child(
            el("p").class("mj-ui-panel-body").child(
                el("a")
                    .class("mj-ui-link")
                    .attr("href", PULL_REQUESTS_PANEL)
                    .text("Open the pull-request queue"),
            ),
        );

    let grid = el("div")
        .class("mj-ui-grid")
        .child(wide(queue))
        .child(panel(
            "Ready to start",
            "Issues whose dependencies are done, in the plan's order.",
            ready,
            &[issues],
        ))
        .child(panel(
            "In progress",
            "Issues the plan records as active.",
            active,
            &[issues],
        ))
        .child(issues_panel(plan));

    View::new(
        "Issues & PRs",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

fn issue_table(
    issues: &super::source::Answer,
    status: &str,
    empty: &str,
) -> crate::cockpit::html::El {
    unanswered(issues).unwrap_or_else(|| {
        let rows = list(issues, "/issues")
            .iter()
            .filter(|i| text(i, "status") == status)
            .take(ROWS * 2)
            .map(|i| {
                el("tr")
                    .child(
                        el("td")
                            .child(el("span").class("mj-ui-mono mj-ui-dim").text(text(i, "id")))
                            .child(el("span").class("mj-ui-cell-title").text(text(i, "title"))),
                    )
                    .child(
                        el("td")
                            .class("mj-ui-mono mj-ui-dim")
                            .text(text(i, "milestone")),
                    )
                    .child(el("td").child(state(i, "priority")))
            })
            .collect::<Vec<_>>();
        table(&["Issue", "Milestone", "Priority"], rows, empty)
    })
}
