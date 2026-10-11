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
pub fn screen(ctx: &Context, query: &[(String, String)]) -> View {
    // the selected pull request travels to the queue panel, which draws its detail
    let selected = query
        .iter()
        .find(|(k, _)| k == "pr")
        .and_then(|(_, v)| v.parse::<u64>().ok());
    let panel_src = match selected {
        Some(n) => format!("{PULL_REQUESTS_PANEL}?pr={n}"),
        None => PULL_REQUESTS_PANEL.to_string(),
    };
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
        .attr("data-panel-src", panel_src.clone())
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
                    .attr("href", panel_src)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;
    use serde_json::json;
    use std::time::Duration;

    fn answer(source: Source, value: serde_json::Value) -> super::super::source::Answer {
        super::super::source::Answer {
            source,
            route: None,
            exposure: Default::default(),
            value: Ok(value),
            took: Duration::ZERO,
        }
    }

    /// With no pull request selected, the queue placeholder asks for the whole queue.
    #[test]
    fn without_a_selection_the_queue_panel_asks_for_the_whole_queue() {
        let repo = SyntheticRepository::planned();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &[]).main.render();
        assert!(
            html.contains(&format!("data-panel-src=\"{PULL_REQUESTS_PANEL}\"")),
            "{html}"
        );
        // a value that is not a number selects nothing
        let html = screen(&ctx, &[("pr".into(), "seven".into())]).main.render();
        assert!(html.contains(&format!("data-panel-src=\"{PULL_REQUESTS_PANEL}\"")));
    }

    /// An issue table lists the issues of its status alone, each with its milestone and
    /// its priority, and says so when there is none.
    #[test]
    fn an_issue_table_lists_the_issues_of_its_status_alone() {
        let issues = answer(
            ISSUES,
            json!({ "issues": [
                { "id": "I0001", "title": "Ready work", "milestone": "m-first", "priority": "p1", "status": "READY" },
                { "id": "I0002", "title": "Blocked work", "milestone": "m-first", "priority": "p2", "status": "BLOCKED" },
                { "id": "I0003", "title": "Active work", "milestone": "m-second", "priority": "p0", "status": "ACTIVE" }
            ] }),
        );
        let html = issue_table(&issues, "READY", "none ready").render();
        assert!(html.contains("I0001") && html.contains("Ready work") && html.contains("m-first"));
        assert!(html.contains("p1"), "{html}");
        assert!(!html.contains("I0002") && !html.contains("I0003"), "{html}");
        let html = issue_table(&issues, "ACTIVE", "none active").render();
        assert!(
            html.contains("I0003") && html.contains("m-second"),
            "{html}"
        );
        let html = issue_table(&issues, "DONE", "Nothing is done.").render();
        assert!(html.contains("Nothing is done."), "{html}");
    }
}
