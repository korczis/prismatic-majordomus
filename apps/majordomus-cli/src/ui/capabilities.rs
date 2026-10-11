//! Capabilities: what the product can be asked, and how. Read from `repository.info` (the
//! registry's own tallies) and `capabilities.list` filtered by kind; one capability in full
//! from `capabilities.describe` when the reader picks it (`?id=`).
//!
//! The resources of the layer — one per object — are counted here and listed nowhere: they
//! are the objects, and the Documentation screen is where a reader meets them.

use serde_json::Value;

use crate::capability::Context;
use crate::cockpit::html::{el, El};

use super::shell::View;
use super::source::{ask_all, ask_input, Answer, Source};
use super::view::{
    counts, donut, hero, kpi, list, nothing, panel, state, table, text, unanswered, wide,
};

/// The route of the screen.
pub const ROUTE: &str = "/ui/capabilities";

const REPOSITORY: Source = Source::of("repository.info");
const COMMANDS: Source = Source::with("capabilities.list", r#"{"kind":"command"}"#);
const QUERIES: Source = Source::with("capabilities.list", r#"{"kind":"query"}"#);

/// The Capabilities screen; `query` may name one capability to show in full.
pub fn screen(ctx: &Context, query: &[(String, String)]) -> View {
    let answers = ask_all(ctx, &[REPOSITORY, COMMANDS, QUERIES]);
    let (repo, commands, queries) = (
        &answers[&REPOSITORY],
        &answers[&COMMANDS],
        &answers[&QUERIES],
    );
    let picked = query
        .iter()
        .find(|(k, _)| k == "id")
        .map(|(_, v)| v.clone());

    let top = hero(
        "puzzle",
        "violet",
        "Capabilities",
        "Everything the product can be asked, once, from every surface.",
        "Each capability is declared once and projected onto the API, MCP and the command line; the UI asks the same declarations.",
        vec![
            kpi("Capabilities", &repo.read("/capabilities/total")),
            kpi("Built in", &repo.read("/capabilities/builtin")),
            kpi("HTTP routes", &repo.read("/capabilities/http_routes")),
            kpi("MCP tools", &repo.read("/capabilities/mcp_tools")),
            kpi("CLI commands", &repo.read("/capabilities/cli_commands")),
            kpi("Modules", &repo.read("/capabilities/modules")),
        ],
    );

    let mut grid = el("div").class("mj-ui-grid");
    if let Some(id) = picked {
        grid = grid.child(wide(detail(ctx, &id)));
    }
    grid = grid
        .child(panel(
            "By kind",
            "Queries read, commands change, resources are the objects of the layer.",
            unanswered(repo)
                .unwrap_or_else(|| donut("capabilities", &counts(repo, "/capabilities/by_kind"))),
            &[repo],
        ))
        .child(panel(
            "By stability",
            "How far each capability's behaviour is proven.",
            unanswered(repo).unwrap_or_else(|| {
                donut("capabilities", &counts(repo, "/capabilities/by_stability"))
            }),
            &[repo],
        ))
        .child(wide(panel(
            "Commands",
            "The capabilities that change something, each with its module and stability.",
            catalogue(commands, "No capability changes anything."),
            &[commands],
        )))
        .child(wide(panel(
            "Queries",
            "The capabilities that read, each with its module and stability.",
            catalogue(queries, "No capability reads anything."),
            &[queries],
        )));

    View::new(
        "Capabilities",
        el("div").class("mj-ui-stack").child(top).child(grid),
    )
}

fn catalogue(answer: &Answer, empty: &str) -> El {
    unanswered(answer).unwrap_or_else(|| {
        let rows = list(answer, "/capabilities")
            .iter()
            .map(|c| {
                let id = text(c, "id");
                el("tr")
                    .attr("id", format!("capability-{id}"))
                    .child(
                        el("td").child(
                            el("a")
                                .class("mj-ui-row-link")
                                .attr("href", format!("{ROUTE}?id={id}#detail"))
                                .child(el("span").class("mj-ui-mono mj-ui-dim").text(id.clone()))
                                .child(el("span").class("mj-ui-cell-title").text(text(c, "title"))),
                        ),
                    )
                    .child(el("td").class("mj-ui-mono").text(text(c, "module")))
                    .child(el("td").child(state(c, "stability")))
            })
            .collect::<Vec<_>>();
        table(&["Capability", "Module", "Stability"], rows, empty)
    })
}

/// One capability in full, as `capabilities.describe` answers it: what it is, every
/// surface it is exposed on, and the fields its input takes.
fn detail(ctx: &Context, id: &str) -> El {
    let described = ask_input(
        ctx,
        "capabilities.describe",
        serde_json::json!({ "id": id }),
    );
    let body = unanswered(&described).unwrap_or_else(|| {
        let at = |p: &str| {
            described
                .ok()
                .and_then(|v| v.pointer(p))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let fields = described
            .ok()
            .and_then(|v| v.pointer("/input/schema/properties"))
            .and_then(Value::as_object)
            .map(|props| {
                props
                    .iter()
                    .map(|(name, schema)| {
                        el("tr")
                            .child(el("td").class("mj-ui-mono").text(name))
                            .child(
                                el("td")
                                    .class("mj-ui-dim")
                                    .text(text(schema, "description")),
                            )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        el("div")
            .class("mj-ui-stack")
            .child(
                el("div")
                    .class("mj-ui-detail-head")
                    .child(el("h3").class("mj-ui-detail-title").text(at("/title")))
                    .child(el("p").class("mj-ui-dim").text(at("/description"))),
            )
            .child(
                el("dl").class("mj-ui-facts").children(
                    [
                        ("kind", "/kind"),
                        ("module", "/module"),
                        ("stability", "/stability"),
                    ]
                    .into_iter()
                    .map(|(label, p)| {
                        el("div")
                            .class("mj-ui-fact")
                            .child(el("dt").text(label))
                            .child(el("dd").text(at(p)))
                    }),
                ),
            )
            .child(if fields.is_empty() {
                nothing("It takes no input.")
            } else {
                table(
                    &["Input field", "What it means"],
                    fields,
                    "It takes no input.",
                )
            })
    });
    panel(
        id,
        "One capability, as its declaration describes it.",
        body,
        &[&described],
    )
    .attr("id", "detail")
}
