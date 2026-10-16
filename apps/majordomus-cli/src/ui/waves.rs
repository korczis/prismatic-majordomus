//! Waves: what the plan lets run at the same time, and what is still open in each wave.
//! Read from `plan.waves` alone. A wave is a set of issues whose dependencies are met by
//! the waves before it; two issues of one wave that share a scope are listed apart, as the
//! plan serialises them.

use crate::capability::Context;
use crate::cockpit::html::el;

use super::dashboard::ROWS;
use super::shell::View;
use super::source::{ask_all, Answer, Source};
use super::view::{
    hero, kpi_count, list, more, nothing, panel, state, table, text, unanswered, value, wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/waves";

const WAVES: Source = Source::of("plan.waves");

/// Whether an issue is still to be done: anything the plan has neither finished nor given up.
fn is_open(issue: &serde_json::Value) -> bool {
    !matches!(text(issue, "status").as_str(), "DONE" | "CANCELLED")
}

/// The Waves screen.
pub fn screen(ctx: &Context) -> View {
    let answers = ask_all(ctx, &[WAVES]);
    let waves = &answers[&WAVES];

    let top = hero(
        "list",
        "violet",
        "Waves",
        "What may run at the same time.",
        "The plan's issues in the order its dependencies allow: every issue of a wave can start once the waves before it are done, and what is still open in each is listed here.",
        vec![
            kpi_count("Waves", &waves.read("/waves")),
            kpi_count("Scope conflicts", &waves.read("/serialised_by_scope")),
        ],
    );

    let mut grid = el("div").class("mj-ui-grid");
    match unanswered(waves) {
        Some(reason) => grid = grid.child(wide(panel("Waves", "plan.waves", reason, &[waves]))),
        None => {
            for wave in list(waves, "/waves") {
                grid = grid.child(wave_panel(&wave, waves));
            }
            grid = grid.child(conflicts_panel(waves));
        }
    }

    View::new(
        "Waves",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

fn wave_panel(wave: &serde_json::Value, answer: &Answer) -> crate::cockpit::html::El {
    let number = wave
        .get("wave")
        .and_then(serde_json::Value::as_u64)
        .map(|n| n.to_string())
        .unwrap_or_else(|| "unknown".into());
    let issues: Vec<serde_json::Value> = wave
        .get("issues")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let open: Vec<&serde_json::Value> = issues.iter().filter(|i| is_open(i)).collect();
    let rows = open
        .iter()
        .take(ROWS)
        .map(|i| {
            let (id, milestone) = (text(i, "id"), text(i, "milestone"));
            el("tr")
                .child(
                    el("td").child(
                        el("a")
                            .class("mj-ui-row-link")
                            .attr("href", format!("{}?issue={id}#issue", super::prs::ROUTE))
                            .child(
                                el("span")
                                    .class("mj-ui-mono mj-ui-dim")
                                    .text(value(i, "id")),
                            )
                            .child(el("span").class("mj-ui-cell-title").text(value(i, "title"))),
                    ),
                )
                .child(el("td").child(if milestone.is_empty() {
                    super::view::none()
                } else {
                    super::view::code_link(super::milestone::href(&milestone), milestone)
                }))
                .child(el("td").child(state(i, "status")))
                .child(el("td").child(state(i, "priority")))
        })
        .collect::<Vec<_>>();
    let body = el("div")
        .child(table(
            &["Issue", "Milestone", "Status", "Priority"],
            rows,
            "Every issue of this wave is done or cancelled.",
        ))
        .children(more(ROWS.min(open.len()), open.len(), None));
    panel(
        &format!("Wave {number}"),
        &format!(
            "{} issue(s) in the wave, {} still open.",
            issues.len(),
            open.len()
        ),
        body,
        &[answer],
    )
}

fn conflicts_panel(answer: &Answer) -> crate::cockpit::html::El {
    let found = list(answer, "/serialised_by_scope");
    let rows = found
        .iter()
        .take(ROWS)
        .map(|f| {
            el("tr")
                .child(el("td").child(el("span").class("mj-ui-mono").text(value(f, "subject"))))
                .child(el("td").child(state(f, "level")))
                .child(el("td").text(value(f, "message")))
        })
        .collect::<Vec<_>>();
    let body = if found.is_empty() {
        nothing("No two issues of a wave share a scope.")
    } else {
        el("div")
            .child(table(&["Issue", "Level", "Why"], rows, ""))
            .children(more(ROWS.min(found.len()), found.len(), None))
    };
    wide(panel(
        "Serialised by scope",
        "Issues of one wave that the plan will not let run together, because they write the same paths.",
        body,
        &[answer],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;
    use serde_json::json;
    use std::time::Duration;

    fn answer(value: serde_json::Value) -> Answer {
        Answer {
            source: WAVES,
            route: None,
            exposure: Default::default(),
            value: Ok(value),
            took: Duration::ZERO,
            standing: crate::ui::source::Standing::NOW,
        }
    }

    /// A wave lists what is still open and says how many of its issues that is; a done or
    /// cancelled issue is counted in the wave's size and not drawn as work.
    #[test]
    fn a_wave_lists_what_is_open_and_not_what_is_done() {
        let a = answer(json!({ "waves": [] }));
        let wave = json!({ "wave": 2, "issues": [
            { "id": "I0001", "title": "Open work", "milestone": "M1", "status": "READY", "priority": "p1" },
            { "id": "I0002", "title": "Finished work", "milestone": "M1", "status": "DONE", "priority": "p1" },
            { "id": "I0003", "title": "Dropped work", "milestone": "M1", "status": "CANCELLED", "priority": "p2" }
        ]});
        let html = wave_panel(&wave, &a).render();
        assert!(html.contains("Wave 2"), "{html}");
        assert!(
            html.contains("3 issue(s) in the wave, 1 still open."),
            "{html}"
        );
        assert!(
            html.contains("I0001") && html.contains("Open work"),
            "{html}"
        );
        assert!(!html.contains("I0002") && !html.contains("I0003"), "{html}");
        assert!(
            html.contains("href=\"/ui/prs?issue=I0001#issue\""),
            "{html}"
        );
    }

    /// A wave with nothing open says so in words, and a wave without a number is `unknown`,
    /// never a made-up one.
    #[test]
    fn a_finished_wave_says_so_and_an_unnumbered_one_is_unknown() {
        let a = answer(json!({}));
        let done = json!({ "wave": 0, "issues": [
            { "id": "I0001", "title": "t", "status": "DONE", "priority": "p0" }
        ]});
        let html = wave_panel(&done, &a).render();
        assert!(
            html.contains("Every issue of this wave is done or cancelled."),
            "{html}"
        );
        let html = wave_panel(&json!({ "issues": [] }), &a).render();
        assert!(html.contains("Wave unknown"), "{html}");
    }

    /// Scope conflicts are drawn from the answer and bounded; none is said in words.
    #[test]
    fn scope_conflicts_are_bounded_and_none_is_said() {
        let many: Vec<serde_json::Value> = (0..ROWS + 5)
            .map(|i| json!({ "level": "WARN", "subject": format!("I{i:04}"), "message": "shares share with I9999" }))
            .collect();
        let html = conflicts_panel(&answer(json!({ "serialised_by_scope": many }))).render();
        assert!(
            html.contains("I0000") && html.contains("5 more not shown"),
            "{html}"
        );
        let html = conflicts_panel(&answer(json!({ "serialised_by_scope": [] }))).render();
        assert!(
            html.contains("No two issues of a wave share a scope."),
            "{html}"
        );
    }

    /// The screen draws from a real plan and names its source.
    #[test]
    fn the_screen_renders_from_a_planned_repository() {
        let repo = SyntheticRepository::planned();
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx).main.render();
        assert!(
            html.contains("Waves") && html.contains("plan.waves"),
            "{html}"
        );
    }
}
