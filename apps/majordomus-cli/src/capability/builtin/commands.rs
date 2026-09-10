//! The `commands` module: the command graph, projected.
//!
//! Three capabilities, all read-only, all answered by [`crate::command_graph`] — the same
//! graph the terminal renders, the same graph the workflow bridge is written from, and the
//! same graph a shell's completion is answered from. Declaring them here is what puts the
//! command graph on MCP, on HTTP, in the OpenAPI document, in Swagger UI, in the Cockpit
//! and in the generated reference, without any of those carrying a route, a schema or a
//! sentence of its own.
//!
//! # Why three, and not one
//!
//! Because a client that wants to know what it may call should not have to read every
//! argument of every command to find out. `commands.list` answers with one line per
//! command; `commands.get` answers with one command in full; `commands.graph` answers with
//! the whole document and its diagnostics. Summary first, detail on demand — the shape an
//! agent's context needs, and the shape a command palette needs, are the same shape.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, McpExposure, McpResource, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::command_graph::load;
use crate::command_graph::model::{
    CommandGraph, CommandNode, Effect, Origin, Projections, Severity,
};
use crate::{capability, module};

use super::{get, mcp, Empty};

/// The URI under which the index of commands is read as an MCP resource.
pub const COMMANDS_URI: &str = "majordomus://commands";

// ---------------------------------------------------------------- inputs

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// Which commands to answer with. Every filter is the graph's own vocabulary; none is a
/// search over rendered text.
pub struct CommandFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only the commands of one program: `executable`, `tool` or `workflow`.
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only the commands whose effect is at most this one: `read_only`,
    /// `local_mutation`, `repository_mutation`, `network_mutation`, `destructive`.
    pub effect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Only the commands matching this text in their invocation, summary, tags or identity.
    pub search: Option<String>,
}

impl BenchmarkCases for CommandFilter {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", CommandFilter::default()),
            NamedCase::new(
                "read-only",
                CommandFilter {
                    effect: Some("read_only".into()),
                    ..Default::default()
                },
            ),
            NamedCase::new(
                "one-program",
                CommandFilter {
                    origin: Some("executable".into()),
                    ..Default::default()
                },
            ),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// One command, by its canonical identity.
pub struct CommandRequest {
    /// The identity, `executable.worktree.status`.
    pub id: String,
}

impl BenchmarkCases for CommandRequest {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "worktree-status",
            CommandRequest {
                id: "executable.worktree.status".into(),
            },
        )]
    }
}

// ---------------------------------------------------------------- outputs

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// One command, as an index shows it: enough to choose, never enough to have to skim.
pub struct CommandSummary {
    /// The canonical identity.
    pub id: String,
    /// The command line a person types.
    pub invocation: String,
    /// One line.
    pub summary: String,
    /// Which program runs it.
    pub origin: String,
    /// What running it changes.
    pub effect: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The group it is found under.
    pub group: Option<String>,
    /// Where it appears.
    pub projections: Projections,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The commands this repository offers, filtered.
pub struct CommandIndex {
    /// The schema of the graph these came from.
    pub schema: String,
    /// The fingerprint of that graph: a client may cache against it.
    pub fingerprint: String,
    /// How many commands the graph holds, before the filter.
    pub total: usize,
    /// The commands that matched, in graph order.
    pub commands: Vec<CommandSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
/// The whole graph, with what its build found.
pub struct CommandGraphReport {
    /// The graph.
    pub graph: CommandGraph,
    /// How many findings are errors; a projection refuses to write while this is not zero.
    pub errors: usize,
}

// ---------------------------------------------------------------- handlers

/// The graph this process answers from.
///
/// The fast load: the cache the last materialisation wrote, or the two declarations that
/// can be read without a subprocess. A server answering a request must not spawn one.
fn graph_of(ctx: &Context) -> CommandGraph {
    // The share directory is where the shell tool's command registry is shipped. It is the
    // one this process located: a second resolution from the repository root alone finds
    // another distribution, or none when the executable lives outside the repository, and
    // the conventions are the fallback only for an index built without an application.
    let root = std::path::Path::new(&ctx.index.repository.root);
    let share = crate::share::Share::locate(ctx.index.share.as_deref(), root).ok();
    load::fast_at(root, share.as_ref().map(|s| s.dir()))
}

fn commands_list(ctx: &Context, filter: CommandFilter) -> Result<CommandIndex, CapabilityError> {
    let graph = graph_of(ctx);
    let origin = match filter.origin.as_deref() {
        None => None,
        Some("executable") => Some(Origin::Executable),
        Some("tool") => Some(Origin::Tool),
        Some("workflow") => Some(Origin::Workflow),
        Some(other) => {
            return Err(CapabilityError::InvalidInput(format!(
                "no program is called `{other}`: executable, tool or workflow"
            )))
        }
    };
    let ceiling = match filter.effect.as_deref() {
        None => None,
        Some("read_only") => Some(Effect::ReadOnly),
        Some("local_mutation") => Some(Effect::LocalMutation),
        Some("repository_mutation") => Some(Effect::RepositoryMutation),
        Some("network_mutation") => Some(Effect::NetworkMutation),
        Some("destructive") => Some(Effect::Destructive),
        Some(other) => {
            return Err(CapabilityError::InvalidInput(format!(
                "no effect is called `{other}`"
            )))
        }
    };
    let needle = filter.search.map(|s| s.to_lowercase());

    let commands = graph
        .commands
        .iter()
        .filter(|c| origin.is_none_or(|o| c.origin == o))
        .filter(|c| ceiling.is_none_or(|e| c.effect <= e))
        .filter(|c| {
            needle
                .as_ref()
                .is_none_or(|t| c.haystack().contains(t.as_str()))
        })
        .map(summary)
        .collect();

    Ok(CommandIndex {
        schema: graph.schema.clone(),
        fingerprint: graph.fingerprint.clone(),
        total: graph.commands.len(),
        commands,
    })
}

fn commands_get(ctx: &Context, request: CommandRequest) -> Result<CommandNode, CapabilityError> {
    let graph = graph_of(ctx);
    graph
        .commands
        .iter()
        .find(|c| c.id.as_str() == request.id)
        .cloned()
        .ok_or_else(|| {
            CapabilityError::NotFound(format!("no command carries the identity `{}`", request.id))
        })
}

fn commands_graph(ctx: &Context, _: Empty) -> Result<CommandGraphReport, CapabilityError> {
    let graph = graph_of(ctx);
    let errors = graph
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();
    Ok(CommandGraphReport { graph, errors })
}

/// One line of the index.
fn summary(node: &CommandNode) -> CommandSummary {
    CommandSummary {
        id: node.id.to_string(),
        invocation: node.invocation.clone(),
        summary: node.summary.clone(),
        origin: format!("{:?}", node.origin).to_lowercase(),
        effect: format!("{:?}", node.effect).to_lowercase(),
        group: node.group.clone(),
        projections: node.projections.clone(),
    }
}

// ---------------------------------------------------------------- the module

/// The `commands` module: every command this repository offers, projected once.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "commands",
        title: "Command graph",
        description: "Every command this repository offers, from whichever program offers it: the Rust executable, the shell tool that carries the task lifecycle, and the workflows the repository declares for a person to run. Composed from the three declarations that already exist — the clap tree, the shipped command registry and the workflow runner's own dump — never from a list. Each command carries what running it changes, what it needs, where its argument values come from, and every surface that carries it, with the reason when one does not.",
        stability: Stability::Implemented,
        capabilities: [
            capability! {
                id: "commands.list",
                title: "Every command, one line each",
                description: "The commands this repository offers, filtered by the program that runs them, by what running them changes, or by text. A summary rather than the whole graph: enough to choose a command, and never so much that a client has to read every argument of every command to find one.",
                input: CommandFilter,
                output: CommandIndex,
                stability: Stability::Implemented,
                exposure: Exposure {
                    // A tool for a client that asks a question, and a resource for one that
                    // wants the index in its context without asking: the summary is small by
                    // construction — an id, a summary and an effect per command — which is
                    // what makes it safe to read whole, and what the detail capability is for.
                    mcp: Some(McpExposure {
                        tool: Some("majordomus_commands".into()),
                        resource: Some(McpResource {
                            uri: COMMANDS_URI.into(),
                            name: "commands".into(),
                        }),
                    }),
                    http: get("/api/v1/commands"),
                    cli: None,
                },
                tags: ["commands", "introspection"],
                handler: commands_list,
            },
            capability! {
                id: "commands.get",
                title: "One command in full",
                description: "One command by its canonical identity: its arguments with the source of each one's values, what running it changes, what it needs, where it came from, and every surface that carries it — with the reason a machine surface withholds it when one does.",
                input: CommandRequest,
                output: CommandNode,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_command"),
                    http: get("/api/v1/command"),
                    cli: None,
                },
                tags: ["commands", "introspection"],
                handler: commands_get,
            },
            capability! {
                id: "commands.graph",
                title: "The whole command graph",
                description: "The graph as one document, with its fingerprint and every diagnostic its build found: a duplicate identity, a recipe name two commands would take, an annotation that names a command which no longer exists. Deterministic — two builds over one tree produce the same document — so a client may cache against the fingerprint.",
                input: Empty,
                output: CommandGraphReport,
                stability: Stability::Implemented,
                exposure: Exposure {
                    mcp: mcp("majordomus_command_graph"),
                    http: get("/api/v1/commands/graph"),
                    cli: None,
                },
                tags: ["commands", "introspection", "diagnostics"],
                handler: commands_graph,
            }
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The module's own documentation says three capabilities, all read-only, all answered
    /// by the command graph. That sentence is the thing every surface then derives from:
    /// the MCP tool names, the HTTP routes, the OpenAPI operations, the Cockpit views. A
    /// refactor that dropped one, renamed a route or added a fourth would still compile,
    /// and every suite that exercises the graph behind them would still pass. This is the
    /// assertion that would not.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "commands");
        let expected: &[(&str, &str, &str)] = &[
            ("commands.list", "majordomus_commands", "/api/v1/commands"),
            ("commands.get", "majordomus_command", "/api/v1/command"),
            (
                "commands.graph",
                "majordomus_command_graph",
                "/api/v1/commands/graph",
            ),
        ];
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        let want: Vec<&str> = expected.iter().map(|(id, _, _)| *id).collect();
        assert_eq!(
            ids, want,
            "the module declares a different set of capabilities"
        );
        for (executable, (id, tool, path)) in m.capabilities.iter().zip(expected) {
            let exposure = &executable.capability.exposure;
            assert_eq!(
                exposure.mcp.as_ref().and_then(|m| m.tool.as_deref()),
                Some(*tool),
                "{id} lost or renamed its MCP tool"
            );
            assert_eq!(
                exposure.http.as_ref().map(|h| h.path.as_str()),
                Some(*path),
                "{id} lost or renamed its HTTP route"
            );
            assert!(
                exposure.cli.is_none(),
                "{id} grew a command line of its own; the graph is read through the \
                 executable's own surfaces, and a command that lists commands would be \
                 the second catalogue this module exists to avoid"
            );
        }
    }
}
