//! The `commands` module: the canonical command graph, read over every transport.
//!
//! Two capabilities, because agents and people ask two different questions. The index is
//! what a caller reads first — one line per command, with its effect and the surfaces it
//! reaches — and it is small enough to hand to a model without spending a context window on
//! arguments nobody asked about. The detail is the whole node, asked for by identity.
//!
//! Neither of them declares anything: both read [`crate::control::graph`], which is a walk
//! of the command line's own declaration. A command added there is in these answers, and in
//! their MCP tools, their HTTP routes, their OpenAPI operations and the Cockpit, with
//! nothing here edited.
//!
//! ```
//! use majordomus_cli::capability::builtin::commands::module;
//! let m = module();
//! let list = m.capabilities.iter().find(|c| c.capability.id.as_str() == "commands.list").unwrap();
//! assert_eq!(list.capability.exposure.http.as_ref().unwrap().path, "/api/v1/commands");
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::capability::CachePolicy;
use crate::control::graph::{self, CommandNode};
use crate::{capability, module};

use super::{get, mcp};

/// Which commands to answer with. Every field narrows; none of them is required, and the
/// unfiltered answer is the whole graph's index.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandQuery {
    /// Only commands whose identity starts with this namespace (`worktree`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// Only commands with this effect (`read_only`, `destructive`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
    /// Only commands whose identity or summary contains this text, case-insensitively.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    /// Only commands a machine surface may invoke.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_callable: Option<bool>,
}

impl BenchmarkCases for CommandQuery {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![
            NamedCase::new("all", CommandQuery::default()),
            NamedCase::new(
                "by-namespace",
                CommandQuery {
                    namespace: Some("worktree".into()),
                    ..CommandQuery::default()
                },
            ),
            NamedCase::new(
                "by-effect",
                CommandQuery {
                    effect: Some("read_only".into()),
                    ..CommandQuery::default()
                },
            ),
        ]
    }
}

/// One command, as the index shows it: enough to choose one, and no more.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandSummary {
    /// The canonical identity.
    pub id: String,
    /// The one-line description.
    pub summary: String,
    /// What running it changes; absent for a command that only groups others.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
    /// The surfaces it reaches, in a fixed order.
    pub surfaces: Vec<String>,
    /// Whether a machine surface may invoke it.
    pub machine_callable: bool,
}

/// The index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandIndex {
    /// The graph's fingerprint, so a caller can tell whether its cache is current.
    pub fingerprint: String,
    /// How many commands the graph holds, before the query narrowed it.
    pub total: usize,
    /// The commands the query matched, in identity order.
    pub commands: Vec<CommandSummary>,
    /// What the composition found wrong; empty is the healthy answer.
    pub diagnostics: Vec<graph::Diagnostic>,
}

/// One command, asked for by identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandInput {
    /// The canonical identity, `worktree.create`.
    pub id: String,
}

impl BenchmarkCases for CommandInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        vec![NamedCase::new(
            "one",
            CommandInput {
                id: "capabilities.list".into(),
            },
        )]
    }
}

fn summarise(node: &CommandNode) -> CommandSummary {
    let mut surfaces = Vec::new();
    if !node.projections.cli.is_empty() {
        surfaces.push("cli".to_string());
    }
    if node.projections.just.is_some() {
        surfaces.push("just".to_string());
    }
    if node.projections.mcp.is_some() {
        surfaces.push("mcp".to_string());
    }
    if node.projections.http.is_some() {
        surfaces.push("http".to_string());
    }
    if node.projections.cockpit.is_some() {
        surfaces.push("cockpit".to_string());
    }
    CommandSummary {
        id: node.id.clone(),
        summary: node.summary.clone(),
        effect: node.semantics.map(|s| s.effect.label().to_string()),
        surfaces,
        machine_callable: node.machine_callable(),
    }
}

fn list(_: &Context, query: CommandQuery) -> Result<CommandIndex, CapabilityError> {
    let g = graph::of_this_executable();
    let needle = query.search.map(|s| s.to_lowercase());
    let commands: Vec<CommandSummary> = g
        .commands
        .iter()
        .filter(|c| !c.group)
        .filter(|c| {
            query
                .namespace
                .as_deref()
                .is_none_or(|n| c.id == n || c.id.starts_with(&format!("{n}.")))
        })
        .filter(|c| {
            query.effect.as_deref().is_none_or(|e| {
                c.semantics.is_some_and(|s| {
                    serde_json::to_string(&s.effect).unwrap_or_default() == format!("\"{e}\"")
                })
            })
        })
        .filter(|c| {
            needle.as_deref().is_none_or(|n| {
                c.id.to_lowercase().contains(n) || c.summary.to_lowercase().contains(n)
            })
        })
        .filter(|c| {
            query
                .machine_callable
                .is_none_or(|want| c.machine_callable() == want)
        })
        .map(summarise)
        .collect();
    Ok(CommandIndex {
        fingerprint: g.fingerprint.clone(),
        total: g.commands.iter().filter(|c| !c.group).count(),
        commands,
        diagnostics: g.diagnostics.clone(),
    })
}

fn get_one(_: &Context, input: CommandInput) -> Result<CommandNode, CapabilityError> {
    graph::of_this_executable()
        .find(&input.id)
        .cloned()
        .ok_or_else(|| CapabilityError::NotFound(format!("no command '{}'", input.id)))
}

/// The module.
pub fn module() -> ModuleDescriptor {
    module! {
        id: "commands",
        title: "Commands",
        description: "The canonical command graph: every command this repository can be asked to run, what running each one changes, and the surfaces it reaches. Read from the command line's own declaration, never from a list.",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "commands.list",
                title: "Every command, in one line each",
                description: "The index of the command graph: identity, summary, effect and the surfaces each command reaches, narrowed by namespace, effect, free text or whether a machine may call it. Small on purpose — the detail of one command is commands.get.",
                input: CommandQuery,
                output: CommandIndex,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_commands"), http: get("/api/v1/commands"), cli: None },
                tags: ["commands", "introspection"],
                cache: CachePolicy::Process { max_entries: 16, ttl_seconds: None },
                handler: list,
            },
            capability! {
                id: "commands.get",
                title: "One command, whole",
                description: "One command by canonical identity: its arguments and where their values come from, what running it changes, how it runs, the names it has answered to, and every surface spelling of it — including the surfaces it is absent from.",
                input: CommandInput,
                output: CommandNode,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure { mcp: mcp("majordomus_command"), http: get("/api/v1/commands/get"), cli: None },
                tags: ["commands", "introspection"],
                cache: CachePolicy::Process { max_entries: 32, ttl_seconds: None },
                handler: get_one,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The declaration is the only place these ids and projection names exist.
    #[test]
    fn the_declaration_yields_the_projections_it_claims() {
        let m = module();
        assert_eq!(m.id.as_str(), "commands");
        let ids: Vec<&str> = m
            .capabilities
            .iter()
            .map(|e| e.capability.id.as_str())
            .collect();
        assert_eq!(ids, ["commands.list", "commands.get"]);
    }

    #[test]
    fn the_index_narrows_and_reports_the_whole_as_its_total() {
        let ctx = crate::synthetic::SyntheticRepository::small()
            .expect("a repository")
            .context()
            .expect("a context");
        let all = list(&ctx, CommandQuery::default()).expect("the index");
        assert!(all.total > 20);
        assert_eq!(all.commands.len(), all.total);
        assert!(all.diagnostics.is_empty(), "{:?}", all.diagnostics);

        let worktree = list(
            &ctx,
            CommandQuery {
                namespace: Some("worktree".into()),
                ..CommandQuery::default()
            },
        )
        .expect("the index");
        assert!(!worktree.commands.is_empty());
        assert!(worktree
            .commands
            .iter()
            .all(|c| c.id.starts_with("worktree")));
        assert_eq!(
            worktree.total, all.total,
            "the total is of the graph, not of the answer"
        );

        let machines = list(
            &ctx,
            CommandQuery {
                machine_callable: Some(false),
                ..CommandQuery::default()
            },
        )
        .expect("the index");
        assert!(machines.commands.iter().all(|c| !c.machine_callable));
        assert!(machines.commands.iter().any(|c| c.id == "worktree.remove"));
    }

    #[test]
    fn a_command_that_does_not_exist_is_not_found_rather_than_empty() {
        let ctx = crate::synthetic::SyntheticRepository::small()
            .expect("a repository")
            .context()
            .expect("a context");
        assert!(get_one(
            &ctx,
            CommandInput {
                id: "capabilities.list".into()
            }
        )
        .is_ok());
        let missing = get_one(
            &ctx,
            CommandInput {
                id: "no.such".into(),
            },
        );
        assert!(matches!(missing, Err(CapabilityError::NotFound(_))));
    }
}
