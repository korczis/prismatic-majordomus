//! Command: everything a person or an agent can run, and what this process is running.
//! Read from `commands.list` (the one catalogue of the executable's and the shell tool's
//! commands, with each one's effect) and `executions.list`.
//!
//! The catalogue is filtered by the capability's own input (`?origin=`), never by this
//! screen: the tabs are links to the same route with the filter the capability declares.

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::dashboard::ROWS;
use super::shell::View;
use super::source::{ask_all, ask_input, Answer, Source};
use super::view::{
    code_link, hero, items, kpi, list, more, none, panel, state, table, text, unanswered, value,
    wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/command";

const EXECUTIONS: Source = Source::of("executions.list");

/// Every capability with its projections, so a command the registry also serves as a
/// capability can lead to it: a command and a capability are one when the capability's
/// declared command-line path is the command's invocation (`capabilities.list`, `exposure.cli`).
const CAPABILITIES: Source = Source::of("capabilities.list");

/// The capability a command is, by its invocation: the one whose declared command-line path,
/// after the executable's name, is that invocation. None when no capability declares it.
///
/// The definition, one command at a time. The screen asks [`by_command_line`], which answers
/// the same for a whole catalogue in one pass; the tests hold the two to each other.
#[cfg(test)]
fn capability_of<'a>(invocation: &str, capabilities: &'a [serde_json::Value]) -> Option<&'a str> {
    let line = invocation.strip_prefix("majordomus ")?;
    capabilities.iter().find_map(|c| {
        let path: Vec<&str> = c
            .pointer("/exposure/cli/path")?
            .as_array()?
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect();
        (!path.is_empty() && path.join(" ") == line)
            .then(|| c.get("id").and_then(serde_json::Value::as_str))
            .flatten()
    })
}

/// Every capability that declares a command line, by that line as a person types it after
/// the executable's name: what [`capability_of`] answers, for a whole catalogue at once. A
/// catalogue of a few hundred commands against a registry of a few thousand capabilities is
/// one pass over each, where asking [`capability_of`] per command walked the registry and
/// built every path again for every row (measured: 0.4-0.6 s of a screen that otherwise
/// renders in a few milliseconds). The first capability to declare a line keeps it, as
/// [`capability_of`] finds the first.
fn by_command_line(capabilities: &[serde_json::Value]) -> std::collections::HashMap<String, &str> {
    let mut lines = std::collections::HashMap::new();
    for c in capabilities {
        let Some(path) = c
            .pointer("/exposure/cli/path")
            .and_then(serde_json::Value::as_array)
        else {
            continue;
        };
        let words: Vec<&str> = path.iter().filter_map(serde_json::Value::as_str).collect();
        let Some(id) = c.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !words.is_empty() {
            lines.entry(words.join(" ")).or_insert(id);
        }
    }
    lines
}

/// Where a capability's inspector is.
pub fn capability_href(id: &str) -> String {
    format!("{}?id={id}#detail", super::capabilities::ROUTE)
}

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
    let answers = ask_all(ctx, &[EXECUTIONS, CAPABILITIES]);
    let executions = &answers[&EXECUTIONS];
    let registry = items(&answers[&CAPABILITIES], "/capabilities");

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

    let catalogue = catalogue(&commands, registry);
    let running = running(executions);

    let grid = el("div")
        .class("mj-ui-grid")
        .child(wide(panel(
            "Commands",
            "Each with the line a person types and what it changes when it runs.",
            el("div").class("mj-ui-stack").child(tabs).child(catalogue),
            &[&commands, &answers[&CAPABILITIES]],
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

/// The catalogue as a table: each command with the line a person types (a link to the
/// capability it is, when the registry declares one by that line), what it does and what it
/// changes.
fn catalogue(commands: &Answer, registry: &[serde_json::Value]) -> El {
    unanswered(commands).unwrap_or_else(|| {
        let lines = by_command_line(registry);
        let rows = items(commands, "/commands")
            .iter()
            .map(|c| {
                let invocation = text(c, "invocation");
                let capability = invocation
                    .strip_prefix("majordomus ")
                    .and_then(|line| lines.get(line).copied());
                let command = match capability {
                    Some(id) => code_link(capability_href(id), invocation)
                        .attr("title", format!("the capability {id}")),
                    None => el("code").class("mj-ui-mono").text(value(c, "invocation")),
                };
                el("tr")
                    .child(el("td").child(command))
                    .child(
                        el("td")
                            .class("mj-ui-dim")
                            .nodes(super::prose::marks(&value(c, "summary"))),
                    )
                    .child(el("td").child(state(c, "effect")))
            })
            .collect::<Vec<_>>();
        table(
            &["Command", "What it does", "Effect"],
            rows,
            "No command matches.",
        )
    })
}

/// The executions this process runs or remembers: each with the capability it runs, a link
/// to that capability, and its state; a longer list says how many it leaves out.
fn running(executions: &Answer) -> El {
    unanswered(executions).unwrap_or_else(|| {
        let all = list(executions, "/executions");
        let rows = all
            .iter()
            .take(ROWS * 2)
            .map(|e| {
                let capability = text(e, "capability");
                el("tr")
                    .child(el("td").class("mj-ui-mono").text(value(e, "id")))
                    .child(el("td").child(if capability.is_empty() {
                        none()
                    } else {
                        code_link(capability_href(&capability), capability)
                    }))
                    .child(el("td").child(state(e, "state")))
            })
            .collect::<Vec<_>>();
        el("div")
            .child(table(
                &["Execution", "Capability", "State"],
                rows,
                "This process is running no execution and remembers none.",
            ))
            .children(more((ROWS * 2).min(all.len()), all.len(), None))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synthetic::SyntheticRepository;

    /// The catalogue's index answers what `capability_of` answers, for every command line a
    /// capability declares and for one none does: the screen and the single lookup cannot
    /// name different capabilities.
    #[test]
    fn the_index_of_command_lines_answers_what_capability_of_answers() {
        let registry = vec![
            serde_json::json!({ "id": "plan.status", "exposure": { "cli": { "path": ["plan", "status"] } } }),
            serde_json::json!({ "id": "plan.second", "exposure": { "cli": { "path": ["plan", "status"] } } }),
            serde_json::json!({ "id": "bench.run", "exposure": { "cli": { "path": ["bench"] } } }),
            serde_json::json!({ "id": "no.cli", "exposure": {} }),
            serde_json::json!({ "id": "empty.path", "exposure": { "cli": { "path": [] } } }),
        ];
        let lines = by_command_line(&registry);
        assert_eq!(lines.len(), 2, "one entry per declared line: {lines:?}");
        for invocation in [
            "majordomus plan status",
            "majordomus bench",
            "majordomus nothing",
        ] {
            let line = invocation.strip_prefix("majordomus ").expect("the prefix");
            assert_eq!(
                lines.get(line).copied(),
                capability_of(invocation, &registry),
                "{invocation}"
            );
        }
        assert_eq!(
            lines.get("plan status").copied(),
            Some("plan.status"),
            "the first keeps it"
        );
    }

    /// The origin filter is the capability's own input: the tab it names is current, and an
    /// origin the capability does not declare is ignored rather than passed on.
    #[test]
    fn the_origin_filter_marks_its_tab_and_an_unknown_one_is_ignored() {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &[("origin".into(), "executable".into())])
            .main
            .render();
        assert!(
            html.contains(&format!(
                "href=\"{ROUTE}?origin=executable\" aria-current=\"page\""
            )),
            "{html}"
        );
        let html = screen(&ctx, &[("origin".into(), "nonsense".into())])
            .main
            .render();
        assert!(
            html.contains(&format!("href=\"{ROUTE}\" aria-current=\"page\"")),
            "{html}"
        );
    }

    /// An execution this process started is listed with its capability and its state.
    #[test]
    fn an_execution_this_process_started_is_listed() {
        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let started = ctx
            .execute(
                "executions.start",
                serde_json::json!({
                    "capability": "executions.demonstrate",
                    "input": { "steps": 1, "delay_ms": 0 }
                }),
            )
            .expect("an execution starts");
        let id = started["id"].as_str().expect("its id").to_string();
        let html = screen(&ctx, &[]).main.render();
        assert!(html.contains(&id), "{html}");
        assert!(html.contains("executions.demonstrate"), "{html}");
        assert!(
            !html.contains("This process is running no execution"),
            "{html}"
        );
    }

    /// A command the registry also serves as a capability leads to that capability: the two
    /// are one when the capability's declared command-line path is the command's invocation.
    #[test]
    fn a_command_leads_to_the_capability_that_declares_its_invocation() {
        let registry = vec![
            serde_json::json!({ "id": "plan.status", "exposure": { "cli": { "path": ["plan", "status"] } } }),
            serde_json::json!({ "id": "no.cli", "exposure": {} }),
            serde_json::json!({ "id": "empty.cli", "exposure": { "cli": { "path": [] } } }),
        ];
        assert_eq!(
            capability_of("majordomus plan status", &registry),
            Some("plan.status")
        );
        assert_eq!(capability_of("majordomus plan", &registry), None);
        assert_eq!(capability_of("bin/other plan status", &registry), None);
        assert_eq!(
            capability_href("plan.status"),
            "/ui/capabilities?id=plan.status#detail"
        );

        let repo = SyntheticRepository::small().expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let html = screen(&ctx, &[]).main.render();
        assert!(
            html.contains("href=\"/ui/capabilities?id="),
            "a command of the executable leads to its capability: {html}"
        );
        assert!(
            html.contains("capabilities.list"),
            "the join names its source: {html}"
        );
    }

    /// An execution names the capability it runs, a link to it; one that names none says so,
    /// and a list longer than its rows says how many it leaves out. A command no capability
    /// declares is its line as text, and a catalogue that could not answer says why.
    #[test]
    fn an_execution_leads_to_its_capability_and_a_long_list_counts_the_rest() {
        let answer = |source: Source, value: Result<serde_json::Value, String>| Answer {
            source,
            route: None,
            exposure: Default::default(),
            value: value.map(Into::into),
            took: std::time::Duration::ZERO,
            standing: crate::ui::source::Standing::NOW,
        };
        let many: Vec<serde_json::Value> = (0..ROWS * 2 + 3)
            .map(|i| serde_json::json!({ "id": format!("x-{i}"), "capability": "plan.status", "state": "done" }))
            .chain([serde_json::json!({ "id": "x-bare" })])
            .collect();
        let html = running(&answer(
            EXECUTIONS,
            Ok(serde_json::json!({ "executions": many })),
        ))
        .render();
        assert!(
            html.contains("href=\"/ui/capabilities?id=plan.status#detail\""),
            "{html}"
        );
        assert!(html.contains("4 more not shown"), "{html}");
        let bare = running(&answer(
            EXECUTIONS,
            Ok(serde_json::json!({ "executions": [{ "id": "x-bare" }] })),
        ))
        .render();
        assert!(
            bare.contains(">none<") && bare.contains("unknown"),
            "{bare}"
        );
        let commands = answer(
            Source::of("commands.list"),
            Ok(serde_json::json!({ "commands": [
                { "invocation": "majordomus plan status", "summary": "s", "effect": "read_only" },
                { "invocation": "bin/tool", "effect": "local_mutation" }
            ]})),
        );
        let registry = vec![
            serde_json::json!({ "id": "plan.status", "exposure": { "cli": { "path": ["plan", "status"] } } }),
        ];
        let html = catalogue(&commands, &registry).render();
        assert!(
            html.contains("title=\"the capability plan.status\""),
            "{html}"
        );
        assert!(
            html.contains("<code class=\"mj-ui-mono\">bin/tool</code>"),
            "{html}"
        );
        assert!(
            html.contains(">none<"),
            "a command with no summary says so: {html}"
        );
        let refused = catalogue(
            &answer(Source::of("commands.list"), Err("no".into())),
            &registry,
        )
        .render();
        assert!(
            refused.contains("commands.list could not answer: no"),
            "{refused}"
        );
    }
}
