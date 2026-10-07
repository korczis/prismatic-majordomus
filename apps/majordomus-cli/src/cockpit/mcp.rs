//! The Cockpit's MCP page: the MCP projection of the registry as `mcp.projection` describes
//! it, and where each executable capability is reachable as `capabilities.projections`
//! derives it. The page holds no tool, no count, no client and no status of its own: a tool
//! added to the registry is on it at the next request, and one removed is gone.

use serde_json::json;

use super::html::{el, El, Node};
use super::nav::Area;
use super::pages::{ask, failed, Page};
use super::view::{
    alert, asked_statistic, badge, card, card_with, cell, chips, facts, id_cell, link, mono,
    nothing, pre, row, table, tag, text_cell,
};
use crate::capability::builtin::mcp::{McpClientStanding, McpProjection, LAUNCHER};
use crate::capability::closure::Matrix;
use crate::capability::{Context, Effect};
use crate::http::router::percent_encode;

const ASKED: &str = "mcp.projection";
const HERE: &str = "/cockpit/mcp";

/// The word an effect is written as, on the wire and in the address of this page.
fn effect_word(effect: Effect) -> &'static str {
    match effect {
        Effect::Read => "read",
        Effect::ProcessState => "process_state",
        Effect::RepositoryMutation => "repository_mutation",
    }
}

/// What a reader is told an effect means, in the words `docs/MCP.md` uses.
fn effect_label(effect: Effect) -> &'static str {
    match effect {
        Effect::Read => "read",
        Effect::ProcessState => "process state",
        Effect::RepositoryMutation => "writes the repository",
    }
}

/// An effect as a badge: the status says how careful a caller should be, the label says
/// why, so the cue never rests on colour alone.
fn effect_badge(effect: Effect) -> El {
    badge(
        match effect {
            Effect::Read => "ok",
            Effect::ProcessState => "info",
            Effect::RepositoryMutation => "warn",
        },
        effect_label(effect),
    )
}

fn capability_href(id: &str) -> String {
    format!("/cockpit/capabilities/{}", percent_encode(id))
}

fn present(value: Option<&String>) -> El {
    match value {
        Some(v) => cell(mono(v.clone())),
        None => text_cell("—"),
    }
}

/// `/cockpit/mcp`, with `?effect=` narrowing the tools to one effect.
pub fn page(ctx: &Context, query: &[(String, String)]) -> Page {
    let asked_effect = query
        .iter()
        .find(|(k, _)| k == "effect")
        .map(|(_, v)| v.as_str())
        .filter(|v| !v.is_empty());
    let input = match asked_effect {
        Some(word) => json!({ "effect": word }),
        None => json!({}),
    };
    let p: McpProjection = match ask(ctx, ASKED, input) {
        Ok(r) => r,
        Err(e) => return failed(Area::Mcp, "MCP", e),
    };
    let matrix: Matrix = match ask(ctx, "capabilities.projections", json!({})) {
        Ok(r) => r,
        Err(e) => return failed(Area::Mcp, "MCP", e),
    };

    // ------------------------------------------------------------ the projection
    let mut stats = el("div").class("mj-stats").child(asked_statistic(
        p.tool_count.to_string(),
        "tools",
        ASKED,
        "tool_count",
    ));
    for e in &p.effects {
        stats = stats.child(asked_statistic(
            e.tools.to_string(),
            effect_label(e.effect),
            ASKED,
            "effects",
        ));
    }
    stats = stats
        .child(asked_statistic(
            p.resources.builtin.to_string(),
            "capability resources",
            ASKED,
            "resources.builtin",
        ))
        .child(asked_statistic(
            p.resources.objects.to_string(),
            "object resources",
            ASKED,
            "resources.objects",
        ));

    let marks = |words: &[String]| {
        el("span")
            .class("mj-marks")
            .children(words.iter().map(|w| tag(w.clone())).collect::<Vec<_>>())
    };
    let transports = table(
        &["Transport", "Reached by", "Sessions attached now"],
        p.transports
            .iter()
            .map(|t| {
                row(vec![
                    cell(mono(t.id.clone())),
                    text_cell(t.reached_by.clone()),
                    text_cell(t.attached.to_string()),
                ])
            })
            .collect(),
    );
    let not_served: Vec<String> = [
        (!p.serving.prompts).then_some("prompts"),
        (!p.serving.notifications).then_some("subscriptions and list-change notifications"),
    ]
    .into_iter()
    .flatten()
    .map(String::from)
    .collect();
    // a row per method rather than a run of tags: a method name does not break, and a
    // table scrolls inside itself where a run of tags pushes the page wide
    let served = table(
        &["Of the protocol", "Served"],
        p.serving
            .methods
            .iter()
            .map(|m| row(vec![cell(mono(m.clone())), cell(badge("ok", "served"))]))
            .chain(
                not_served
                    .iter()
                    .map(|w| row(vec![text_cell(w.clone()), cell(badge("info", "not served"))])),
            )
            .collect(),
    );
    let overview = card_with(
        "The projection",
        link("/cockpit/api", "HTTP routes"),
        el("div")
            .child(stats)
            .child(facts(vec![
                (
                    "Server",
                    Node::Element(mono(format!("{} {}", p.server.name, p.server.version))),
                ),
                ("Protocol versions", Node::Element(marks(&p.protocol_versions))),
                ("Object resources", Node::Element(mono(p.resources.template.clone()))),
            ]))
            .child(transports)
            .child(served)
            .child(el("p").class("mj-note").text(
                "A tool is a capability and a resource is a capability or an object of the layer: MCP declares nothing of its own. The attached sessions are this process's peer board at the moment the page was asked for, not a claim about any other server.",
            )),
    );

    // ------------------------------------------------------------ the writers
    let writers = if p.writers.is_empty() {
        card(
            "Tools that write the repository",
            nothing("No tool of this registry writes the repository."),
        )
    } else {
        let rows: Vec<El> = p
            .writers
            .iter()
            .filter_map(|name| matrix.rows.iter().find(|r| r.mcp_tool.as_ref() == Some(name)))
            .map(|r| {
                row(vec![
                    cell(mono(r.mcp_tool.clone().unwrap_or_default())),
                    id_cell(capability_href(&r.id), r.id.clone()),
                    present(r.http.as_ref()),
                    present(r.cli.as_ref()),
                ])
            })
            .collect();
        card(
            "Tools that write the repository",
            el("div")
                .child(table(&["Tool", "Capability", "HTTP", "Command line"], rows))
                .child(el("p").class("mj-note").text(
                    "Each is announced to a client as not read-only, destructive and not idempotent, and the `initialize` instructions name them. The handler a tool reaches is the one its HTTP route reaches: a refusal there is the same refusal here.",
                )),
        )
    };

    // ------------------------------------------------------------ the tools
    let here = |effect: Option<Effect>| match effect {
        Some(e) => format!("{HERE}?effect={}", effect_word(e)),
        None => HERE.to_string(),
    };
    let mut filter = vec![(
        "every tool".to_string(),
        here(None),
        p.tool_count,
        asked_effect.is_none(),
    )];
    for e in &p.effects {
        filter.push((
            effect_label(e.effect).to_string(),
            here(Some(e.effect)),
            e.tools,
            asked_effect == Some(effect_word(e.effect)),
        ));
    }
    let tool_rows: Vec<El> = p
        .tools
        .iter()
        .map(|t| {
            row(vec![
                cell(mono(t.name.clone())),
                id_cell(capability_href(&t.capability), t.capability.clone()),
                text_cell(t.title.clone()),
                cell(effect_badge(t.effect)),
            ])
        })
        .collect();
    let tools = card(
        "Tools",
        el("div")
            .child(chips(filter))
            .child(if tool_rows.is_empty() {
                nothing("No tool has this effect.")
            } else {
                table(&["Tool", "Capability", "Title", "Effect"], tool_rows)
            })
            .child(el("p").class("mj-note").text(
                "The capability's page shows the input and output schemas a tool carries; they are the capability's own, never restated for MCP.",
            )),
    );

    // ------------------------------------------------------------ the clients
    let command = if p.launcher_in_repository {
        format!("bin/{LAUNCHER}")
    } else {
        LAUNCHER.to_string()
    };
    let client_rows: Vec<El> = p
        .clients
        .iter()
        .map(|c| {
            let (status, label) = match c.standing {
                McpClientStanding::Wired => ("ok", "starts this repository's server"),
                McpClientStanding::Foreign => ("warn", "exists, does not name the launcher"),
                McpClientStanding::Absent => ("info", "not written here"),
            };
            row(vec![
                text_cell(c.title.clone()),
                cell(mono(c.config.clone())),
                cell(badge(status, label)),
            ])
        })
        .collect();
    let mut clients_body = el("div");
    for f in &p.findings {
        clients_body = clients_body.child(alert("warn", format!("{} — {}", f.message, f.remedy)));
    }
    clients_body = clients_body
        .child(if client_rows.is_empty() {
            nothing("The distribution declares no client configuration.")
        } else {
            table(&["Client", "Reads", "Here"], client_rows)
        })
        .child(el("p").class("mj-prose").text(
            "A client is connected by one entry in the file it reads: a stdio server whose command is the launcher. It needs no argument, no environment variable and no secret.",
        ))
        .child(pre(
            serde_json::to_string_pretty(&json!({
                "mcpServers": { "majordomus": { "type": "stdio", "command": command, "args": [] } }
            }))
            .unwrap_or_default(),
        ))
        .child(el("p").class("mj-note").text(
            "The launcher starts the server in the repository the client was opened in, wherever inside it the client's working directory is, and attaches to the one already running when there is one.",
        ));
    let clients = card("Clients", clients_body);

    // ------------------------------------------------------------ the matrix
    let executable: Vec<_> = matrix
        .rows
        .iter()
        .filter(|r| r.cli.is_some() || r.http.is_some() || r.mcp_tool.is_some())
        .collect();
    let unmet = executable.iter().filter(|r| !r.closed).count();
    let matrix_rows: Vec<El> = executable
        .iter()
        .map(|r| {
            row(vec![
                id_cell(capability_href(&r.id), r.id.clone()),
                cell(effect_badge(r.effect)),
                present(r.cli.as_ref()),
                present(r.http.as_ref()),
                present(r.mcp_tool.as_ref()),
                present(r.mcp_resource.as_ref()),
            ])
        })
        .collect();
    let reach = card_with(
        "Where each capability is reachable",
        badge(
            if unmet == 0 { "ok" } else { "fail" },
            if unmet == 0 {
                "every declared exposure is answered".to_string()
            } else {
                format!("{unmet} declared exposure(s) unanswered")
            },
        ),
        el("div")
            .child(table(
                &[
                    "Capability",
                    "Effect",
                    "Command line",
                    "HTTP",
                    "MCP tool",
                    "MCP resource",
                ],
                matrix_rows,
            ))
            .child(el("p").class("mj-note").text(
                "A row per capability that is executable from somewhere, from `capabilities.projections`. A dash is a projection the capability does not declare, which is a decision recorded on its declaration and not an omission of this table.",
            )),
    );

    Page::new(
        Area::Mcp,
        "MCP",
        el("div")
            .class("mj-grid")
            .child(overview)
            .child(writers)
            .child(clients)
            .child(tools)
            .child(reach),
    )
    .subtitle("The MCP projection of the capability registry: what a client is served, what each tool may change, and how a client is connected.")
    .trail(vec![("Cockpit", Some("/cockpit")), ("MCP", None)])
}
