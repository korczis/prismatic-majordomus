//! The exposure policy: which surfaces carry a command, derived from what the command is.
//!
//! Nothing here reads a per-command configuration, because there is none. A surface is not
//! a property a declaration sets; it is a consequence of the effect, the interactivity and
//! the origin — and those are already on the node. The whole policy is the two functions
//! below, so the question "why is this not an MCP tool?" has one answer in one place, and
//! `commands explain` can quote it.
//!
//! The rules, in one table:
//!
//! | effect / interactivity | command line | workflow bridge | MCP · HTTP | Cockpit |
//! |---|---|---|---|---|
//! | read-only, non-interactive | yes | yes | yes, when a capability backs it | yes |
//! | local mutation | yes | yes | yes, when a capability backs it | yes, confirmed |
//! | repository / network mutation | yes | yes | no | no |
//! | destructive | yes | yes | no | no |
//! | interactive | yes | yes | no | no |
//! | long-running | yes | yes | no | no |
//!
//! The command line and the workflow runner carry everything: a person at a terminal is
//! the caller both were built for, and withholding a command from the terminal that
//! implements it would be theatre. Every machine surface stops at local mutation, and the
//! two that execute — MCP and HTTP — additionally require a capability behind the command,
//! because a surface that executes must have a typed input schema to execute *from*.

use super::model::{
    CommandNode, Effect, Interactivity, Origin, Projections, Provenance, Surface,
};

/// Where the architecture of the command graph is documented; the page a command without
/// one of its own points at.
pub const DOCS_PREFIX: &str = "/docs/commands";

/// The strongest effect a machine surface will carry.
pub const MACHINE_CEILING: Effect = Effect::LocalMutation;

/// The route of a command's page.
///
/// Not a new page tree. Each program's commands already have a reference on this site, and
/// the rule here is only which one: the executable's command line under `/docs/cli/`, the
/// shell tool's commands under `/commands/`, both generated from the same declarations this
/// graph reads. A recipe has no page of its own and points at the architecture, rather than
/// at a page that does not exist.
pub fn docs_route(node: &CommandNode) -> String {
    match node.origin {
        Origin::Executable => format!("/docs/cli/{}/", node.path.join("/")),
        Origin::Tool => format!("/commands/{}/", node.path.join("-")),
        Origin::Workflow => format!("{DOCS_PREFIX}/"),
    }
}

/// The workflow runner's spelling of a command path: the words joined by `-`.
///
/// `majordomus worktree status` becomes `worktree-status`. The rule is an algorithm and
/// not a table, so a command added tomorrow has a recipe name today.
pub fn workflow_name(path: &[String]) -> String {
    path.join("-")
}

/// Words the workflow runner cannot accept as a recipe name, or that would read as one of
/// its own options.
///
/// A projection that would produce one of these is qualified rather than emitted, and the
/// graph says so. The list is a property of the runner's grammar, which is a contract, not
/// a fact discoverable from this repository.
pub const RESERVED_WORKFLOW_NAMES: &[&str] = &["default", "list", "help", "choose", "init"];

/// Decide every projection of one node.
///
/// `capability` carries the MCP tool name and HTTP route the registry already declared for
/// the capability behind this command, when there is one; the policy decides whether they
/// may be *used*, never what they are called.
pub fn project(
    node: &CommandNode,
    capability: Option<(&Option<String>, &Option<String>)>,
) -> Projections {
    let mut out = Projections {
        docs: docs_route(node),
        ..Projections::default()
    };

    if node.runnable {
        out.cli = Some(node.invocation.clone());
    } else {
        // A node that only groups others still has a page and a completion, and it is
        // still what a person types on the way to a leaf.
        out.cli = Some(node.invocation.clone());
    }

    // The workflow runner carries the two programs' commands as a bridge. A workflow is
    // already a recipe; projecting it back into one would be the cycle this design exists
    // to forbid.
    if node.runnable && node.origin != Origin::Workflow {
        let name = workflow_name(&node.path);
        if !RESERVED_WORKFLOW_NAMES.contains(&name.as_str()) {
            out.workflow = Some(name);
        }
    }
    if node.origin == Origin::Workflow {
        out.workflow = Some(node.path.join("-"));
    }

    let (withheld, machine_ok) = machine_verdict(node);
    if machine_ok {
        if let Some((mcp, http)) = capability {
            out.mcp.clone_from(mcp);
            out.http.clone_from(http);
        }
        // The Cockpit renders any command a machine surface may execute; the capability
        // page is where a generated form for it already lives.
        if out.http.is_some() {
            out.cockpit = Some("/cockpit/commands".into());
        }
    }
    out.withheld = withheld;
    out
}

/// Whether a machine surface may carry this command, and the reason when it may not.
///
/// The reason is the product, not the boolean: it is what `commands explain`, the Cockpit's
/// disabled state and the completion's filtering all show, and it is written once here.
pub fn machine_verdict(node: &CommandNode) -> (Option<String>, bool) {
    match node.interactivity {
        Interactivity::Interactive => (
            Some("asks the person something; a request/response surface would hang".into()),
            false,
        ),
        Interactivity::LongRunning => (
            Some("serves until it is stopped; a request/response surface cannot carry it".into()),
            false,
        ),
        Interactivity::NonInteractive => {
            if node.effect > MACHINE_CEILING {
                (
                    Some(format!(
                        "effect {:?} is above the machine ceiling {:?}",
                        node.effect, MACHINE_CEILING
                    )),
                    false,
                )
            } else if !node.runnable {
                (Some("groups other commands; nothing to execute".into()), false)
            } else {
                (None, true)
            }
        }
    }
}

/// Whether a projection on this surface should offer the command at all.
///
/// A completion asks this before it offers a candidate, so an unavailable command is not
/// suggested prominently — and the reason it carries is what a shell that can show one
/// displays.
pub fn offered_on(node: &CommandNode, surface: Surface) -> bool {
    match surface {
        Surface::Cli => true,
        Surface::Workflow => node.projections.workflow.is_some(),
        Surface::Mcp => node.projections.mcp.is_some(),
        Surface::Http => node.projections.http.is_some(),
    }
}

/// The provenance of a node whose declaration is one file.
pub fn declared_in(path: &str) -> Provenance {
    Provenance {
        declared_in: path.into(),
        read_by: None,
        capability: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Stability;
    use crate::command_graph::model::{Availability, CommandId, Execution};

    fn node(effect: Effect, interactivity: Interactivity, runnable: bool) -> CommandNode {
        let path = vec!["worktree".to_string(), "status".to_string()];
        CommandNode {
            id: CommandId::derive(Origin::Executable, &path),
            origin: Origin::Executable,
            invocation: "majordomus worktree status".into(),
            summary: "s".into(),
            description: None,
            runnable,
            arguments: Vec::new(),
            execution: Execution {
                origin: Origin::Executable,
                argv: path.clone(),
            },
            effect,
            interactivity,
            stability: Stability::Implemented,
            availability: Availability::always(),
            group: None,
            tags: Vec::new(),
            aliases: Vec::new(),
            deprecation: None,
            provenance: declared_in("apps/majordomus-cli/src/cli.rs"),
            projections: Projections::default(),
            entrypoint: None,
            path,
        }
    }

    #[test]
    fn read_only_reaches_every_surface() {
        let n = node(Effect::ReadOnly, Interactivity::NonInteractive, true);
        let mcp = Some("majordomus_worktree_status".to_string());
        let http = Some("/api/v1/worktree".to_string());
        let p = project(&n, Some((&mcp, &http)));
        assert_eq!(p.cli.as_deref(), Some("majordomus worktree status"));
        assert_eq!(p.workflow.as_deref(), Some("worktree-status"));
        assert_eq!(p.mcp, mcp);
        assert_eq!(p.http, http);
        assert!(p.withheld.is_none());
        assert_eq!(p.docs, "/docs/cli/worktree/status/");
    }

    #[test]
    fn a_repository_mutation_is_withheld_from_machines_with_a_reason() {
        let n = node(
            Effect::RepositoryMutation,
            Interactivity::NonInteractive,
            true,
        );
        let mcp = Some("t".to_string());
        let http = Some("/api/v1/x".to_string());
        let p = project(&n, Some((&mcp, &http)));
        assert!(p.mcp.is_none() && p.http.is_none());
        assert!(p.withheld.is_some());
        // and the terminal still has it
        assert!(p.cli.is_some() && p.workflow.is_some());
    }

    #[test]
    fn a_server_is_never_a_machine_tool() {
        let n = node(Effect::ReadOnly, Interactivity::LongRunning, true);
        let (why, ok) = machine_verdict(&n);
        assert!(!ok);
        assert!(why.unwrap().contains("stopped"));
    }

    #[test]
    fn a_workflow_is_never_bridged_back_into_a_workflow() {
        let mut n = node(Effect::ReadOnly, Interactivity::NonInteractive, true);
        n.origin = Origin::Workflow;
        n.path = vec!["site-build".into()];
        n.execution.origin = Origin::Workflow;
        let p = project(&n, None);
        assert_eq!(p.workflow.as_deref(), Some("site-build"));
    }
}
