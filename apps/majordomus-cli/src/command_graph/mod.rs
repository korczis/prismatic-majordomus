//! The canonical command graph: every command this repository offers, from whichever
//! program offers it, with where each one is projected.
//!
//! ```text
//!   clap tree            share/commands.yaml         just --dump
//!   (the executable)     (the shell tool)            (the workflows)
//!         │                     │                          │
//!         └──────────┬──────────┴──────────┬───────────────┘
//!                    ▼                     ▼
//!              contributors          capability registry
//!                    └──────────┬──────────┘
//!                               ▼
//!                        CommandGraph  ── fingerprint
//!                               │
//!    ┌──────────┬───────────────┼───────────────┬──────────────┐
//!    ▼          ▼               ▼               ▼              ▼
//!  bridge   completion      MCP · HTTP       Cockpit         docs
//! ```
//!
//! # What this is not
//!
//! It is not a registry. Nothing is declared here and nothing registers with it: every
//! node is read from a declaration that already existed and is already authority for what
//! it states — clap for the shape of the native command line, the shipped command registry
//! for the shell tool, the workflow runner's own dump for the recipes. Adding a command to
//! any of those puts it in the graph, and every projection follows, because a projection
//! asks the graph rather than keeping a list.
//!
//! # Dependency direction
//!
//! The graph depends on the contributors and on the repository environment. Nothing it
//! projects depends on it in return: the generated bridge, the completion engine, the
//! capability module that serves it and the reference pages are all consumers. A
//! projection that fed the graph would make the graph a rendering of its own output.

pub mod bridge;
pub mod complete;
pub mod load;
pub mod model;
pub mod native;
pub mod policy;
pub mod semantics;
pub mod shell;
pub mod tool;
pub mod workflow;

pub use model::{
    ArgumentSpec, Availability, CommandGraph, CommandId, CommandNode, Diagnostic, Effect,
    Execution, Interactivity, Origin, Projections, Provenance, Requirement, Secrecy, Severity,
    Surface, ValueChoice, ValueSource, SCHEMA,
};

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::capability::registry::CapabilityRegistry;
use crate::environment::WorkflowCatalogue;

/// What a build reads.
///
/// Every field is optional in the sense that a build without it produces a smaller graph
/// rather than an error: a checkout with no shell tool, no workflow runner or no built
/// registry still has a command line, and the completion for it must still work.
#[derive(Default)]
pub struct Inputs<'a> {
    /// The capability registry, for the join that gives a command its typed schema, its
    /// MCP tool and its HTTP route. Absent on the fast path, where nothing needs them.
    pub registry: Option<&'a CapabilityRegistry>,
    /// The share directory, where the shell tool's command registry is shipped.
    pub share: Option<&'a Path>,
    /// The workflows the runner reports.
    pub workflows: Option<&'a WorkflowCatalogue>,
    /// The recipe names the last materialised bridge emitted. A recipe with one of these
    /// names is this graph's own projection and is not read back as a declaration.
    pub bridged: BTreeSet<String>,
}

/// Build the graph.
///
/// Deterministic: given the same inputs it produces the same document, byte for byte,
/// including the fingerprint. Nothing here reads the clock, the network or the
/// environment.
pub fn build(inputs: &Inputs<'_>) -> CommandGraph {
    let mut nodes = Vec::new();
    let mut diagnostics = Vec::new();

    let native = native::contribute(inputs.registry);
    nodes.extend(native.nodes);
    diagnostics.extend(native.diagnostics);

    if let Some(share) = inputs.share {
        let tool = tool::contribute(share);
        nodes.extend(tool.nodes);
        diagnostics.extend(tool.diagnostics);
    }

    // The two programs' commands are projected first, because the set of recipe names the
    // bridge would emit is what decides whether a recipe the runner reports is a
    // declaration or this graph's own output.
    for node in &mut nodes {
        let capability = node
            .provenance
            .capability
            .as_ref()
            .and_then(|id| inputs.registry.and_then(|r| r.get(id.as_str())));
        let exposures = capability.map(|c| {
            (
                c.exposure.mcp.as_ref().and_then(|m| m.tool.clone()),
                c.exposure.http.as_ref().map(|h| h.path.clone()),
            )
        });
        node.projections = match &exposures {
            Some((mcp, http)) => policy::project(node, Some((mcp, http))),
            None => policy::project(node, None),
        };
    }

    qualify_collisions(&mut nodes, &mut diagnostics);

    if let Some(catalogue) = inputs.workflows {
        let contribution = workflow::contribute(catalogue, &inputs.bridged);
        diagnostics.extend(contribution.diagnostics);
        let mut declared = contribution.nodes;
        for node in &mut declared {
            node.projections = policy::project(node, None);
        }
        // A declared workflow owns its name. A bridge that would take the same name is
        // withheld and says so, rather than one of them silently winning by the order a
        // directory happened to be walked.
        let taken: BTreeSet<String> = declared
            .iter()
            .filter_map(|n| n.projections.workflow.clone())
            .collect();
        for node in &mut nodes {
            let Some(name) = node.projections.workflow.clone() else {
                continue;
            };
            if taken.contains(&name) {
                node.projections.workflow = None;
                diagnostics.push(
                    Diagnostic::error(
                        "workflow-name-collision",
                        format!(
                            "`just {name}` is a declared workflow, so the bridge for `{}` cannot take that name",
                            node.invocation
                        ),
                        vec![node.id.clone()],
                    )
                    .with_remedy(format!(
                        "rename the declared recipe, or accept that `{}` has no workflow bridge",
                        node.invocation
                    )),
                );
            }
        }
        nodes.extend(declared);
    }

    nodes.sort_by(|a, b| (a.origin, &a.path).cmp(&(b.origin, &b.path)));
    diagnostics.extend(validate(&nodes));
    diagnostics.sort_by(|a, b| (a.severity, &a.code, &a.message).cmp(&(b.severity, &b.code, &b.message)));

    let fingerprint = fingerprint(&nodes);
    CommandGraph {
        schema: SCHEMA.into(),
        fingerprint,
        commands: nodes,
        diagnostics,
    }
}

/// Two programs, one recipe name: qualify both rather than let one win.
///
/// `majordomus bench` is a command of the executable and a command of the shell tool, and
/// the derived recipe name for each is `bench`. Choosing between them by origin order would
/// make the meaning of a recipe depend on the order a contributor happens to run in, so
/// neither keeps the bare name: both are prefixed with the program that owns them, and the
/// graph says so. A short spelling for either is then a decision, declared once as an alias
/// beside the command line.
fn qualify_collisions(nodes: &mut [CommandNode], diagnostics: &mut Vec<Diagnostic>) {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for node in nodes.iter() {
        if let Some(name) = &node.projections.workflow {
            *counts.entry(name.clone()).or_default() += 1;
        }
    }
    for node in nodes.iter_mut() {
        let Some(name) = node.projections.workflow.clone() else {
            continue;
        };
        if counts.get(&name).copied().unwrap_or(0) < 2 {
            continue;
        }
        let qualified = format!("{}-{name}", node.origin.prefix());
        diagnostics.push(Diagnostic::warning(
            "workflow-name-qualified",
            format!(
                "`just {name}` would name two commands, so `{}` is projected as `just {qualified}`",
                node.invocation
            ),
            vec![node.id.clone()],
        ));
        node.projections.workflow = Some(qualified);
    }
}

/// The invariants a graph must hold for its projections to be safe to write.
///
/// Every one of these is a defect that would otherwise surface as a surface behaving
/// oddly: two recipes with one name, a tool advertised twice, a bridge that calls itself.
fn validate(nodes: &[CommandNode]) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for node in nodes {
        *seen.entry(node.id.as_str()).or_default() += 1;
    }
    for (id, count) in seen.iter().filter(|(_, c)| **c > 1) {
        out.push(Diagnostic::error(
            "duplicate-command-id",
            format!("{count} commands claim the identity `{id}`"),
            Vec::new(),
        ));
    }

    let mut recipes: BTreeMap<String, Vec<CommandId>> = BTreeMap::new();
    let mut tools: BTreeMap<String, Vec<CommandId>> = BTreeMap::new();
    for node in nodes {
        if let Some(name) = &node.projections.workflow {
            recipes.entry(name.clone()).or_default().push(node.id.clone());
        }
        if let Some(name) = &node.projections.mcp {
            tools.entry(name.clone()).or_default().push(node.id.clone());
        }
    }
    for (name, ids) in recipes.iter().filter(|(_, v)| v.len() > 1) {
        out.push(Diagnostic::error(
            "duplicate-workflow-projection",
            format!("`just {name}` would be written by {} commands", ids.len()),
            ids.clone(),
        ));
    }
    for (name, ids) in tools.iter().filter(|(_, v)| v.len() > 1) {
        out.push(Diagnostic::error(
            "duplicate-mcp-projection",
            format!("the MCP tool `{name}` is claimed by {} commands", ids.len()),
            ids.clone(),
        ));
    }

    // No bridge may run a bridge. The execution descriptor names the program directly, so
    // this can only fail if a contributor is wrong — which is exactly why it is checked.
    for node in nodes {
        if node.projections.workflow.is_some()
            && node.execution.origin == Origin::Workflow
            && node.origin != Origin::Workflow
        {
            out.push(Diagnostic::error(
                "execution-cycle",
                format!(
                    "`{}` is projected as a recipe and executed through the recipe runner",
                    node.invocation
                ),
                vec![node.id.clone()],
            ));
        }
    }

    // Every compatibility alias names a command that still exists — among the programs
    // this graph actually read. A graph built without the shell tool's registry carries no
    // command of the shell tool, and an alias to one is unresolved rather than stale.
    let ids: Vec<String> = nodes.iter().map(|n| n.id.to_string()).collect();
    let origins: BTreeSet<Origin> = nodes.iter().map(|n| n.origin).collect();
    for alias in semantics::unused_aliases(&ids) {
        let target_origin = CommandId::derive(Origin::Executable, &[])
            .origin()
            .filter(|_| alias.command.starts_with("executable."))
            .or_else(|| alias.command.starts_with("tool.").then_some(Origin::Tool))
            .or_else(|| alias.command.starts_with("workflow.").then_some(Origin::Workflow));
        if target_origin.is_some_and(|o| !origins.contains(&o)) {
            continue;
        }
        out.push(
            Diagnostic::error(
                "alias-names-no-command",
                format!(
                    "the workflow alias `just {}` names `{}`, which the graph does not carry",
                    alias.alias, alias.command
                ),
                Vec::new(),
            )
            .with_remedy("point the alias at the command that replaced it, or remove it"),
        );
    }

    // A secret must never be enumerable anywhere.
    for node in nodes {
        for arg in &node.arguments {
            if arg.secrecy == Secrecy::Secret && arg.source.suggestible() {
                out.push(Diagnostic::error(
                    "secret-is-suggestible",
                    format!(
                        "`{}` takes `{}`, which is a secret with a value source that would enumerate it",
                        node.invocation, arg.name
                    ),
                    vec![node.id.clone()],
                ));
            }
        }
    }

    out
}

/// The identity of a graph's semantic content.
///
/// Over the nodes only: the diagnostics are a reading of the same content, and a graph
/// whose diagnostics changed because a message was reworded is the same graph as far as
/// every cache and every generated file is concerned. Nothing time-dependent is in it, so
/// two builds over one tree agree.
fn fingerprint(nodes: &[CommandNode]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(SCHEMA.as_bytes());
    for node in nodes {
        // serde_json with a BTreeMap-free struct keeps declaration order, and the node
        // order is already deterministic, so the serialisation is stable.
        let bytes = serde_json::to_vec(node).unwrap_or_default();
        hasher.update(&bytes);
    }
    format!("{:x}", hasher.finalize())[..16].to_string()
}

/// The commands a newcomer is offered, in a deterministic order.
///
/// The rule is one sentence and the declarations decide its outcome: an entry point is a
/// runnable command its declaration marks as one — a top-level command of the executable,
/// a public command of the shell tool, a recipe named after its group. Nothing here names
/// a command, so a banner rendered from this cannot outlive the command it offers.
pub fn entrypoints(graph: &CommandGraph, limit: usize) -> Vec<&CommandNode> {
    let mut out: Vec<&CommandNode> = graph
        .commands
        .iter()
        .filter(|c| c.runnable && c.entrypoint == Some(true) && c.availability.available)
        .collect();
    out.sort_by_key(|c| {
        (
            // A workflow first: the repository declared it for a person to run here.
            match c.origin {
                Origin::Workflow => 0,
                Origin::Tool => 1,
                Origin::Executable => 2,
            },
            c.effect,
            c.path.join("-"),
        )
    });
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_graph_builds_from_the_command_line_alone() {
        let graph = build(&Inputs::default());
        assert_eq!(graph.schema, SCHEMA);
        assert!(graph.commands.len() > 40);
        assert_eq!(graph.fingerprint.len(), 16);
        assert!(
            graph.errors().is_empty(),
            "errors: {:?}",
            graph.errors()
        );
    }

    #[test]
    fn the_fingerprint_is_deterministic() {
        let a = build(&Inputs::default());
        let b = build(&Inputs::default());
        assert_eq!(a.fingerprint, b.fingerprint);
    }

    #[test]
    fn every_command_has_a_page_and_a_command_line() {
        let graph = build(&Inputs::default());
        for node in &graph.commands {
            assert!(node.projections.cli.is_some(), "{} has no command line", node.id);
            assert!(
                node.projections.docs.starts_with('/') && node.projections.docs.ends_with('/'),
                "{} has no page",
                node.id
            );
        }
    }

    #[test]
    fn a_surface_spelling_resolves_back_to_one_node() {
        let graph = build(&Inputs::default());
        let node = graph
            .resolve(Surface::Workflow, "worktree-status")
            .expect("the bridge spells worktree status this way");
        assert_eq!(node.id.as_str(), "executable.worktree.status");
        let same = graph
            .resolve(Surface::Cli, "majordomus worktree status")
            .expect("the command line spells it this way");
        assert_eq!(same.id, node.id);
    }

    #[test]
    fn no_mutation_reaches_a_machine_surface() {
        let graph = build(&Inputs::default());
        for node in &graph.commands {
            if node.effect > policy::MACHINE_CEILING {
                assert!(
                    node.projections.mcp.is_none() && node.projections.http.is_none(),
                    "{} is above the ceiling and reaches a machine surface",
                    node.id
                );
                assert!(node.projections.withheld.is_some());
            }
        }
    }

    #[test]
    fn entrypoints_come_from_the_declarations() {
        let graph = build(&Inputs::default());
        let offered = entrypoints(&graph, 5);
        assert!(!offered.is_empty());
        assert!(offered.iter().all(|c| c.runnable));
    }
}
