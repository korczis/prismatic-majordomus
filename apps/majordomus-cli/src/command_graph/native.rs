//! The native command line as command-graph nodes.
//!
//! clap is the declaration; this walks it. Nothing here restates a command, an argument, a
//! default or a description — [`crate::cli::docs::tree`] already turns the built
//! `clap::Command` into data for the reference pages, and this is a second reading of the
//! same tree, not a second tree.
//!
//! Three things are added that clap cannot hold, each from a place that already owns it:
//!
//! - the **semantics** — effect, interactivity, requirements — from [`super::semantics`];
//! - the **capability** behind a command, joined through the registry's own
//!   [`crate::capability::model::CliExposure`], which also proves the two agree;
//! - the **value source** of each argument, inferred from the declaration first and
//!   annotated only where the type clap erased is one a registry here knows.
//!
//! # Why the join is checked
//!
//! A capability may declare that it is reached as `majordomus x y`. Nothing dispatched
//! from that declaration before this module existed: the clap arm was written separately,
//! and a capability whose `cli` exposure named a command that was never added produced a
//! documented command that did not exist. The join reports that as an error, which turns a
//! silent documentation defect into a failing build.

use crate::capability::model::CliExposure;
use crate::capability::registry::CapabilityRegistry;
use crate::cli::docs::{self, ArgDoc, CommandDoc};

use super::model::{
    ArgumentSpec, Availability, CommandId, CommandNode, Diagnostic, Execution, Origin,
    Projections, Provenance, Requirement, Secrecy, ValueChoice, ValueSource,
};
use super::semantics;

/// What the walk produced.
pub struct Contribution {
    /// The nodes, in declaration order.
    pub nodes: Vec<CommandNode>,
    /// What the walk found: a stale annotation, a capability that names a command that
    /// does not exist.
    pub diagnostics: Vec<Diagnostic>,
}

/// Walk the built command line into nodes, joined against the capability registry.
///
/// `registry` may be `None`: the graph is buildable without one, which is what keeps the
/// completion path off the index. The join, and the errors it can report, are then absent
/// rather than wrong — the caller that wants them passes a registry.
pub fn contribute(registry: Option<&CapabilityRegistry>) -> Contribution {
    let tree = docs::tree();
    let mut nodes = Vec::new();
    let mut diagnostics = Vec::new();
    let mut paths: Vec<Vec<String>> = Vec::new();

    for doc in tree.flatten() {
        // `flatten` includes the root, which is the executable itself and not a command.
        if doc.path.len() < 2 {
            continue;
        }
        let path: Vec<String> = doc.path[1..].to_vec();
        paths.push(path.clone());
        nodes.push(node(doc, &path, registry));
    }

    for stale in semantics::unused(&paths) {
        diagnostics.push(
            Diagnostic::error(
                "semantics-names-no-command",
                format!("the semantics beside the command line name `{stale}`, which the command line does not declare"),
                Vec::new(),
            )
            .with_remedy("remove the entry, or restore the command it names"),
        );
    }

    if let Some(registry) = registry {
        for capability in registry.iter() {
            let Some(CliExposure { path: words }) = &capability.exposure.cli else {
                continue;
            };
            if !paths.iter().any(|p| p == words) {
                diagnostics.push(
                    Diagnostic::error(
                        "capability-names-no-command",
                        format!(
                            "capability `{}` is exposed as `majordomus {}`, which the command line does not declare",
                            capability.id,
                            words.join(" ")
                        ),
                        Vec::new(),
                    )
                    .with_remedy("add the clap arm, or drop the cli exposure"),
                );
            }
        }
    }

    Contribution { nodes, diagnostics }
}

/// One command of the native command line.
fn node(doc: &CommandDoc, path: &[String], registry: Option<&CapabilityRegistry>) -> CommandNode {
    let sem = semantics::of(path);
    let capability = registry.and_then(|r| r.by_cli(path));

    let provenance = Provenance {
        declared_in: docs::DECLARATION.into(),
        read_by: Some("majordomus commands list".into()),
        capability: capability.map(|c| c.id.clone()),
    };

    // A capability behind a command is the stronger statement about what running it does:
    // the registry classified its kind, and the classification is verified there. The
    // annotation beside the declaration answers only for the commands that have none.
    let effect = capability
        .map(|c| super::model::Effect::of_capability(c.kind))
        .unwrap_or(sem.effect);
    let stability = capability
        .map(|c| c.stability)
        .unwrap_or(crate::capability::Stability::Implemented);

    let arguments = doc
        .args
        .iter()
        .map(|a| argument(a, path, &doc.usage))
        .collect();

    CommandNode {
        id: CommandId::derive(Origin::Executable, path),
        origin: Origin::Executable,
        invocation: format!("majordomus {}", path.join(" ")),
        summary: doc.about.clone(),
        description: doc.long_about.clone(),
        runnable: doc.executable,
        arguments,
        execution: Execution {
            origin: Origin::Executable,
            argv: path.to_vec(),
        },
        effect,
        interactivity: sem.interactivity,
        stability,
        availability: Availability {
            available: true,
            reason: None,
            requires: sem.requires.to_vec(),
        },
        group: path.first().cloned(),
        tags: capability.map(|c| c.tags.clone()).unwrap_or_default(),
        aliases: doc.aliases.clone(),
        deprecation: None,
        provenance,
        projections: Projections::default(),
        entrypoint: Some(path.len() == 1),
        path: path.to_vec(),
    }
}

/// One argument, with its value source inferred and then annotated.
fn argument(arg: &ArgDoc, path: &[String], usage: &str) -> ArgumentSpec {
    let values: Vec<ValueChoice> = arg
        .possible_values
        .iter()
        .map(|v| ValueChoice {
            value: v.name.clone(),
            description: v.help.clone(),
        })
        .collect();

    let source = infer(arg, path, &values);
    let secrecy = semantics::secrecy(&arg.name, arg.value_name.as_deref());
    // A secret is never enumerated, whatever the inference concluded.
    let source = if secrecy == Secrecy::Secret {
        ValueSource::Secret
    } else {
        source
    };

    ArgumentSpec {
        name: arg.name.clone(),
        long: arg.long.clone(),
        short: arg.short,
        positional: arg.positional,
        takes_value: arg.takes_value,
        required: arg.required,
        variadic: arg.positional && usage.contains("..."),
        global: arg.global,
        help: arg.help.clone(),
        value_name: arg.value_name.clone(),
        values,
        defaults: arg.defaults.clone(),
        source,
        secrecy,
    }
}

/// Where an argument's values come from, decided from the declaration before anything is
/// annotated.
///
/// The order is the point: a value set the declaration carries always wins, then the
/// annotation for an identifier whose registry is here, then the placeholder's own meaning.
/// A command author who gives an argument a value-enum type or a `PATH` placeholder has
/// already said everything a completion needs, and is not asked to say it again.
fn infer(arg: &ArgDoc, path: &[String], values: &[ValueChoice]) -> ValueSource {
    if !arg.takes_value {
        return ValueSource::Free;
    }
    if !values.is_empty() {
        return ValueSource::Enumerated;
    }
    if let Some(source) = semantics::binding(path, &arg.name) {
        return source;
    }
    let placeholder = arg.value_name.as_deref().unwrap_or("").to_ascii_uppercase();
    match placeholder.as_str() {
        "PATH" | "DIR" | "FILE" | "OUT" => ValueSource::Path,
        "BRANCH" => ValueSource::Branch,
        "SHELL" => ValueSource::Shell,
        _ => match arg.name.as_str() {
            "repo" | "out" | "path" => ValueSource::Path,
            "branch" | "base" => ValueSource::Branch,
            "shell" => ValueSource::Shell,
            _ => ValueSource::Free,
        },
    }
}

/// The requirements every native command shares, for a reader that wants the default
/// without constructing a node.
pub const NATIVE_REQUIREMENTS: &[Requirement] = &[Requirement::Repository, Requirement::Layer];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_walk_finds_the_command_line() {
        let c = contribute(None);
        assert!(
            c.nodes.len() > 40,
            "the command line has more commands than {}",
            c.nodes.len()
        );
        let status = c
            .nodes
            .iter()
            .find(|n| n.id.as_str() == "executable.worktree.status")
            .expect("worktree status is declared");
        assert_eq!(status.invocation, "majordomus worktree status");
        assert!(status.runnable);
        assert_eq!(status.group.as_deref(), Some("worktree"));
    }

    #[test]
    fn semantics_reach_the_nodes() {
        let c = contribute(None);
        let remove = c
            .nodes
            .iter()
            .find(|n| n.id.as_str() == "executable.worktree.remove")
            .expect("worktree remove is declared");
        assert_eq!(remove.effect, super::super::model::Effect::Destructive);
        let serve = c
            .nodes
            .iter()
            .find(|n| n.id.as_str() == "executable.serve")
            .expect("serve is declared");
        assert_eq!(
            serve.interactivity,
            super::super::model::Interactivity::LongRunning
        );
    }

    #[test]
    fn every_annotation_names_a_command_that_exists() {
        let c = contribute(None);
        let stale: Vec<_> = c
            .diagnostics
            .iter()
            .filter(|d| d.code == "semantics-names-no-command")
            .collect();
        assert!(stale.is_empty(), "stale annotations: {stale:?}");
    }

    #[test]
    fn value_sources_are_inferred_from_the_declaration() {
        let c = contribute(None);
        let path_cmd = c
            .nodes
            .iter()
            .find(|n| n.id.as_str() == "executable.worktree.path")
            .expect("worktree path is declared");
        let branch = path_cmd
            .arguments
            .iter()
            .find(|a| a.name == "branch")
            .expect("it takes a branch");
        assert_eq!(branch.source, ValueSource::Branch);

        // A value-enum argument carries its own values and needs no annotation.
        let enumerated = c
            .nodes
            .iter()
            .flat_map(|n| n.arguments.iter())
            .find(|a| !a.values.is_empty())
            .expect("some argument is a value enum");
        assert_eq!(enumerated.source, ValueSource::Enumerated);
    }
}
