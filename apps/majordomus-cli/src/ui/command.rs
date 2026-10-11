//! Command: everything a person or an agent can run, and what this process is running.
//! Read from `commands.list` (the one catalogue of the executable's and the shell tool's
//! commands, with each one's effect) and `executions.list`.
//!
//! The catalogue is filtered by the capability's own input (`?origin=`), never by this
//! screen: the tabs are links to the same route with the filter the capability declares.

use crate::capability::Context;
use crate::cockpit::html::el;

use super::dashboard::ROWS;
use super::shell::View;
use super::source::{ask_all, ask_input, Source};
use super::view::{hero, kpi, list, panel, state, table, text, unanswered, wide};

/// The route of the screen.
pub const ROUTE: &str = "/ui/command";

const EXECUTIONS: Source = Source::of("executions.list");

/// The origins `commands.list` filters by, as its input declares them.
const ORIGINS: &[(&str, &str)] = &[
    ("", "All"),
    ("executable", "Executable"),
    ("shell", "Shell tool"),
];

/// The Command screen; `query` may carry the catalogue's `origin` filter.
pub fn screen(ctx: &Context, query: &[(String, String)]) -> View {
    let origin = query
        .iter()
        .find(|(k, _)| k == "origin")
        .map(|(_, v)| v.as_str())
        .filter(|o| ORIGINS.iter().any(|(id, _)| id == o && !id.is_empty()));
    let input = match origin {
        Some(o) => serde_json::json!({ "origin": o }),
        None => serde_json::json!({}),
    };
    let commands = ask_input(ctx, "commands.list", input);
    let answers = ask_all(ctx, &[EXECUTIONS]);
    let executions = &answers[&EXECUTIONS];

    let top = hero(
        "terminal",
        "violet",
        "Command",
        "Everything that can be run, and what is running.",
        "Every command of the executable and the shell tool, with what it changes, and the executions this process is running or remembers.",
        vec![
            kpi("Commands", &commands.read("/total")),
            kpi("Executions", &executions.read("/count")),
            kpi("Active", &executions.read("/active")),
            kpi("Queued", &executions.read("/queued")),
            kpi("Live channels", &executions.read("/live_channels")),
        ],
    );

    let tabs = el("nav")
        .class("mj-ui-tabs")
        .attr("aria-label", "Which commands")
        .children(ORIGINS.iter().map(|(id, label)| {
            let current = origin.unwrap_or("") == *id;
            let href = if id.is_empty() {
                ROUTE.to_string()
            } else {
                format!("{ROUTE}?origin={id}")
            };
            el("a")
                .class("mj-ui-tab")
                .attr("href", href)
                .attr_if("aria-current", current.then_some("page"))
                .text(*label)
        }));

    let catalogue = unanswered(&commands).unwrap_or_else(|| {
        let rows = list(&commands, "/commands")
            .iter()
            .map(|c| {
                el("tr")
                    .child(
                        el("td").child(el("code").class("mj-ui-mono").text(text(c, "invocation"))),
                    )
                    .child(el("td").class("mj-ui-dim").text(text(c, "summary")))
                    .child(el("td").child(state(c, "effect")))
            })
            .collect::<Vec<_>>();
        table(
            &["Command", "What it does", "Effect"],
            rows,
            "No command matches.",
        )
    });

    let running = unanswered(executions).unwrap_or_else(|| {
        let rows = list(executions, "/executions")
            .iter()
            .take(ROWS * 2)
            .map(|e| {
                el("tr")
                    .child(el("td").class("mj-ui-mono").text(text(e, "id")))
                    .child(el("td").class("mj-ui-mono").text(text(e, "capability")))
                    .child(el("td").child(state(e, "state")))
            })
            .collect::<Vec<_>>();
        table(
            &["Execution", "Capability", "State"],
            rows,
            "This process is running no execution and remembers none.",
        )
    });

    let grid = el("div")
        .class("mj-ui-grid")
        .child(wide(panel(
            "Commands",
            "Each with the line a person types and what it changes when it runs.",
            el("div").class("mj-ui-stack").child(tabs).child(catalogue),
            &[&commands],
        )))
        .child(wide(panel(
            "Executions",
            "What this process is running, or ran and remembers, through its execution engine.",
            running,
            &[executions],
        )));

    View::new(
        "Command",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}
