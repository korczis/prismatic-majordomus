//! The repository's own workflows as command-graph nodes.
//!
//! A workflow is a recipe the repository declares for a person to run and that no program
//! here implements — building the site, running the suite, publishing. The runner already
//! describes them: [`crate::environment::workflows`] reads `just --dump --dump-format
//! json` and turns it into a typed catalogue, and this is a second reading of that
//! catalogue, not a second parser.
//!
//! # The bridge is not a workflow
//!
//! Once the graph projects the two programs' commands into recipes, the runner reports
//! those recipes too — and reading them back as workflows would produce a node whose
//! execution is a recipe whose execution is that node. That is the cycle this design
//! exists to forbid, and it is closed by construction: [`contribute`] is given the set of
//! names the graph projects, and a recipe with such a name is the graph's own bridge and
//! is not a declaration of anything.
//!
//! # What a workflow's effect is
//!
//! It is not inferred from the name, the group or the body. A recipe is arbitrary shell,
//! and a guess about what arbitrary shell does is exactly the kind of thing that is right
//! until the day it matters. A recipe the runner marks as needing confirmation is
//! destructive, because that is what the marker means; every other workflow is read as a
//! repository mutation, which is the strongest ordinary class and therefore the reading
//! that keeps an unclassified workflow off every machine surface.

use std::collections::BTreeSet;

use crate::environment::{WorkflowCatalogue, WorkflowDescriptor};

use super::model::{
    ArgumentSpec, Availability, CommandId, CommandNode, Diagnostic, Effect, Execution,
    Interactivity, Origin, Projections, Provenance, Requirement, Secrecy, ValueSource,
};

/// The file a projection shows as the declaration of a workflow.
pub const DECLARATION: &str = "justfile";

/// What the read produced.
pub struct Contribution {
    /// The nodes, in the runner's order.
    pub nodes: Vec<CommandNode>,
    /// What the read found.
    pub diagnostics: Vec<Diagnostic>,
}

/// Read the declared workflows, excluding the recipes the graph itself projects.
///
/// `bridged` is the set of recipe names the graph's own workflow projection emits. It is
/// computed from the graph, never read back from the runner, so the exclusion cannot drift
/// from what was generated.
pub fn contribute(catalogue: &WorkflowCatalogue, bridged: &BTreeSet<String>) -> Contribution {
    let mut nodes = Vec::new();
    let mut bridge_names = Vec::new();

    for descriptor in &catalogue.workflows {
        if bridged.contains(&descriptor.name) {
            bridge_names.push(descriptor.name.clone());
            continue;
        }
        nodes.push(node(descriptor, catalogue));
    }

    let mut diagnostics = Vec::new();
    if !bridge_names.is_empty() {
        diagnostics.push(Diagnostic {
            severity: super::model::Severity::Info,
            code: "workflow-bridge-recognised".into(),
            message: format!(
                "{} recipe(s) are this graph's own bridge and are not read as declarations: {}",
                bridge_names.len(),
                bridge_names.join(", ")
            ),
            commands: Vec::new(),
            remedy: None,
        });
    }
    Contribution { nodes, diagnostics }
}

/// One node from one recipe.
fn node(descriptor: &WorkflowDescriptor, catalogue: &WorkflowCatalogue) -> CommandNode {
    let path = vec![descriptor.name.clone()];
    let effect = if descriptor.confirm {
        Effect::Destructive
    } else {
        Effect::RepositoryMutation
    };

    let arguments = descriptor
        .parameters
        .iter()
        .map(|p| ArgumentSpec {
            name: p.name.clone(),
            long: None,
            short: None,
            positional: true,
            takes_value: true,
            required: p.required,
            variadic: p.variadic,
            global: false,
            help: String::new(),
            value_name: Some(p.name.to_uppercase()),
            values: Vec::new(),
            defaults: Vec::new(),
            // The runner declares a parameter's name and whether it is variadic, and
            // nothing about what its values are. Nothing here invents one.
            source: ValueSource::Free,
            secrecy: Secrecy::Public,
        })
        .collect();

    let entrypoint = catalogue
        .entrypoints
        .iter()
        .any(|e| e.workflow == descriptor.name);

    CommandNode {
        id: CommandId::derive(Origin::Workflow, &path),
        origin: Origin::Workflow,
        invocation: format!("just {}", descriptor.name),
        summary: descriptor.description.clone().unwrap_or_default(),
        description: None,
        runnable: true,
        arguments,
        execution: Execution {
            origin: Origin::Workflow,
            argv: path.clone(),
        },
        effect,
        // A recipe runs in the caller's terminal and returns; the runner has no notion of
        // a service, and a recipe that starts one is a recipe that does not return, which
        // no surface here can discover from the dump.
        interactivity: Interactivity::NonInteractive,
        stability: crate::capability::Stability::Implemented,
        availability: Availability {
            available: true,
            reason: None,
            requires: vec![Requirement::Repository, Requirement::WorkflowRunner],
        },
        group: descriptor.group.clone(),
        tags: Vec::new(),
        aliases: Vec::new(),
        deprecation: None,
        provenance: Provenance {
            declared_in: DECLARATION.into(),
            read_by: Some(crate::environment::workflows::SOURCE.into()),
            capability: None,
        },
        projections: Projections::default(),
        entrypoint: Some(entrypoint),
        path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::{WorkflowEntrypoint, WorkflowParameter};

    fn catalogue(names: &[&str]) -> WorkflowCatalogue {
        WorkflowCatalogue {
            state: crate::environment::TierState::Resolved,
            source: Some(crate::environment::workflows::SOURCE.into()),
            workflows: names
                .iter()
                .map(|n| WorkflowDescriptor {
                    name: (*n).into(),
                    namespace: None,
                    description: Some(format!("does {n}")),
                    group: Some("site".into()),
                    parameters: vec![WorkflowParameter {
                        name: "args".into(),
                        variadic: true,
                        required: false,
                    }],
                    dependencies: Vec::new(),
                    confirm: *n == "clean",
                })
                .collect(),
            entrypoints: vec![WorkflowEntrypoint {
                group: "site".into(),
                workflow: "site-build".into(),
                command: "just site-build".into(),
                description: None,
            }],
        }
    }

    #[test]
    fn a_bridged_recipe_is_not_read_back_as_a_declaration() {
        let cat = catalogue(&["site-build", "worktree-status"]);
        let bridged = BTreeSet::from(["worktree-status".to_string()]);
        let c = contribute(&cat, &bridged);
        assert_eq!(c.nodes.len(), 1);
        assert_eq!(c.nodes[0].id.as_str(), "workflow.site-build");
        assert!(c
            .diagnostics
            .iter()
            .any(|d| d.code == "workflow-bridge-recognised"));
    }

    #[test]
    fn a_confirmed_recipe_is_destructive_and_every_other_is_a_repository_mutation() {
        let cat = catalogue(&["clean", "site-build"]);
        let c = contribute(&cat, &BTreeSet::new());
        let clean = c.nodes.iter().find(|n| n.path[0] == "clean").unwrap();
        let build = c.nodes.iter().find(|n| n.path[0] == "site-build").unwrap();
        assert_eq!(clean.effect, Effect::Destructive);
        assert_eq!(build.effect, Effect::RepositoryMutation);
    }

    #[test]
    fn the_runners_entrypoints_reach_the_nodes() {
        let cat = catalogue(&["site-build", "clean"]);
        let c = contribute(&cat, &BTreeSet::new());
        let build = c.nodes.iter().find(|n| n.path[0] == "site-build").unwrap();
        assert_eq!(build.entrypoint, Some(true));
        let clean = c.nodes.iter().find(|n| n.path[0] == "clean").unwrap();
        assert_eq!(clean.entrypoint, Some(false));
    }

    #[test]
    fn a_workflow_never_projects_into_a_machine_surface() {
        let cat = catalogue(&["site-build"]);
        let c = contribute(&cat, &BTreeSet::new());
        let (why, ok) = super::super::policy::machine_verdict(&c.nodes[0]);
        assert!(!ok, "an unclassified workflow must not reach a machine");
        assert!(why.is_some());
    }
}
