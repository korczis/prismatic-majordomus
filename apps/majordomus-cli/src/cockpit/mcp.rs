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
        Effect::RemoteMutation => "remote_mutation",
    }
}

/// What a reader is told an effect means, in the words `docs/MCP.md` uses.
fn effect_label(effect: Effect) -> &'static str {
    match effect {
        Effect::Read => "read",
        Effect::ProcessState => "process state",
        Effect::RepositoryMutation => "writes the repository",
        Effect::RemoteMutation => "changes other machines",
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
            Effect::RemoteMutation => "fail",
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
    let asked: Result<(McpProjection, Matrix), String> = ask(ctx, ASKED, input)
        .and_then(|p| ask(ctx, "capabilities.projections", json!({})).map(|matrix| (p, matrix)));
    match asked {
        Ok((p, matrix)) => rendered(&p, &matrix, asked_effect),
        Err(e) => failed(Area::Mcp, "MCP", e),
    }
}

/// The page for one description of the projection and one matrix. Everything on it is
/// read from those two values, so what a registry with no writer, a distribution with no
/// client or a capability with an unanswered exposure looks like can be asked without
/// building such a registry.
fn rendered(p: &McpProjection, matrix: &Matrix, asked_effect: Option<&str>) -> Page {
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
            .chain(not_served.iter().map(|w| {
                row(vec![
                    text_cell(w.clone()),
                    cell(badge("info", "not served")),
                ])
            }))
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
            .filter_map(|name| {
                matrix
                    .rows
                    .iter()
                    .find(|r| r.mcp_tool.as_ref() == Some(name))
            })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A description of the projection as the capability answers it, with the parts a
    /// test varies passed in.
    fn projection(
        writers: &[&str],
        tools: serde_json::Value,
        launcher_in_repository: bool,
        clients: serde_json::Value,
        findings: serde_json::Value,
    ) -> McpProjection {
        serde_json::from_value(json!({
            "server": { "name": "majordomus", "title": "Majordomus", "version": "0.0.0" },
            "protocol_versions": ["2025-06-18"],
            "transports": [{ "id": "stdio", "reached_by": "majordomus mcp", "attached": 1 }],
            "serving": {
                "methods": ["initialize", "tools/list"],
                "tools": true, "resources": true, "prompts": false, "notifications": false
            },
            "tool_count": 2,
            "tools": tools,
            "effects": [{ "effect": "read", "tools": 2 }],
            "writers": writers,
            "resources": { "builtin": 1, "objects": 3, "template": "majordomus://{kind}/{id}" },
            "launcher_in_repository": launcher_in_repository,
            "clients": clients,
            "findings": findings
        }))
        .expect("a projection as the capability answers it")
    }

    fn matrix(rows: serde_json::Value) -> Matrix {
        serde_json::from_value(json!({ "rows": rows, "unbacked": [], "unclassified": [] }))
            .expect("a matrix as the capability answers it")
    }

    fn row_of(id: &str, tool: Option<&str>, effect: &str, closed: bool) -> serde_json::Value {
        let mut row = json!({
            "id": id, "module": "m", "kind": "query", "stability": "stable",
            "effect": effect, "http": format!("GET /api/v1/{id}"), "closed": closed,
            "source": "builtin"
        });
        if let Some(tool) = tool {
            row["mcp_tool"] = json!(tool);
        }
        row
    }

    fn a_read_tool() -> serde_json::Value {
        json!([{
            "name": "majordomus_read", "capability": "a.read", "module": "a", "title": "A read",
            "effect": "read",
            "hints": { "read_only": true, "destructive": false, "idempotent": true, "open_world": false }
        }])
    }

    #[test]
    fn a_registry_with_no_writer_says_so_and_one_with_a_writer_names_its_routes() {
        let rows = matrix(json!([
            row_of("a.read", Some("majordomus_read"), "read", true),
            row_of(
                "a.write",
                Some("majordomus_write"),
                "repository_mutation",
                true
            ),
        ]));
        let none = rendered(
            &projection(&[], a_read_tool(), false, json!([]), json!([])),
            &rows,
            None,
        )
        .main
        .render();
        assert!(none.contains("No tool of this registry writes the repository."));

        let one = rendered(
            &projection(
                &["majordomus_write"],
                a_read_tool(),
                false,
                json!([]),
                json!([]),
            ),
            &rows,
            None,
        )
        .main
        .render();
        assert!(!one.contains("No tool of this registry writes the repository."));
        assert!(one.contains("majordomus_write"));
        assert!(one.contains("GET /api/v1/a.write"));
    }

    #[test]
    fn a_filter_no_tool_passes_says_so_instead_of_an_empty_table() {
        let rows = matrix(json!([row_of(
            "a.read",
            Some("majordomus_read"),
            "read",
            true
        )]));
        let page = rendered(
            &projection(&[], json!([]), false, json!([]), json!([])),
            &rows,
            Some("process_state"),
        )
        .main
        .render();
        assert!(page.contains("No tool has this effect."));
        let listed = rendered(
            &projection(&[], a_read_tool(), false, json!([]), json!([])),
            &rows,
            Some("read"),
        )
        .main
        .render();
        assert!(!listed.contains("No tool has this effect."));
        assert!(listed.contains("majordomus_read"));
    }

    #[test]
    fn each_client_is_shown_where_it_stands_and_the_command_is_the_one_that_would_run() {
        let rows = matrix(json!([row_of(
            "a.read",
            Some("majordomus_read"),
            "read",
            true
        )]));
        let clients = json!([
            { "id": "claude", "title": "Claude Code", "config": ".mcp.json", "standing": "wired" },
            { "id": "codex", "title": "Codex", "config": ".codex/config.toml", "standing": "foreign" },
            { "id": "gemini", "title": "Gemini CLI", "config": ".gemini/settings.json", "standing": "absent" },
        ]);
        let findings = json!([{
            "code": "mcp_client_config_foreign",
            "message": "the codex file names something else",
            "remedy": "name the launcher in it"
        }]);
        let here = rendered(
            &projection(&[], a_read_tool(), true, clients.clone(), findings),
            &rows,
            None,
        )
        .main
        .render();
        assert!(
            here.contains("starts this repository&#39;s server")
                || here.contains("starts this repository's server")
        );
        assert!(here.contains("exists, does not name the launcher"));
        assert!(here.contains("not written here"));
        assert!(here.contains("the codex file names something else — name the launcher in it"));
        assert!(here.contains(&format!("bin/{LAUNCHER}")));

        let installed = rendered(
            &projection(&[], a_read_tool(), false, clients, json!([])),
            &rows,
            None,
        )
        .main
        .render();
        assert!(!installed.contains(&format!("bin/{LAUNCHER}")));
        assert!(installed.contains(LAUNCHER));

        let undeclared = rendered(
            &projection(&[], a_read_tool(), false, json!([]), json!([])),
            &rows,
            None,
        )
        .main
        .render();
        assert!(undeclared.contains("The distribution declares no client configuration."));
    }

    #[test]
    fn an_unanswered_exposure_is_counted_and_a_closed_matrix_says_every_one_is_answered() {
        let p = projection(&[], a_read_tool(), false, json!([]), json!([]));
        let closed = rendered(
            &p,
            &matrix(json!([row_of(
                "a.read",
                Some("majordomus_read"),
                "read",
                true
            )])),
            None,
        )
        .main
        .render();
        assert!(closed.contains("every declared exposure is answered"));

        let open = rendered(
            &p,
            &matrix(json!([
                row_of("a.read", Some("majordomus_read"), "read", true),
                row_of("a.gone", Some("majordomus_gone"), "read", false),
                row_of("a.lost", None, "process_state", false),
            ])),
            None,
        )
        .main
        .render();
        assert!(open.contains("2 declared exposure(s) unanswered"));
        assert!(!open.contains("every declared exposure is answered"));
    }
}
