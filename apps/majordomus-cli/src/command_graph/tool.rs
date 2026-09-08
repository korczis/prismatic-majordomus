//! The shell tool's commands as command-graph nodes.
//!
//! `bin/majordomus` is a different program with the same name: the task lifecycle, written
//! in shell. Its command surface already has a canonical owner — `share/commands.yaml`,
//! shipped beside the rules, reconciled against the tool's own dispatch table by the
//! `command_surface_complete` doctrine. This reads that owner. It does not parse the help
//! text, and it does not keep a list of its own: a command added to the registry and the
//! dispatch table appears here on the next build, and one removed disappears.
//!
//! # What is translated, and what is read as it stands
//!
//! Two fields are translated into the graph's vocabulary, because the graph spans three
//! programs and the registry speaks for one:
//!
//! | registry `class` | graph effect | why |
//! |---|---|---|
//! | `read-only` | read-only | writes nothing |
//! | `state-mutating` | local mutation | writes under `.ai/local/`, which no commit carries |
//! | `generated-output-mutating` | repository mutation | rewrites tracked, generated files |
//!
//! and interactivity is read from the declared `syntax`: a command whose synopsis takes a
//! body on standard input holds a terminal, and no request/response surface may carry it.
//! Both are derivations from what the registry already states, not new declarations.

use std::path::Path;

use serde::Deserialize;

use crate::metadata::yaml;

use super::model::{
    Availability, CommandId, CommandNode, Diagnostic, Effect, Execution, Interactivity, Origin,
    Projections, Provenance, Requirement,
};

/// The shipped registry this reads, relative to the share directory.
pub const REGISTRY: &str = "commands.yaml";

/// The path a projection shows as the declaration.
pub const DECLARATION: &str = "share/commands.yaml";

/// The dispatch table the registry is reconciled against, named so a diagnostic can point
/// at it.
pub const DISPATCH: &str = "bin/majordomus";

/// The shipped command registry, as far as the graph reads it.
#[derive(Debug, Deserialize)]
struct Registry {
    version: u32,
    #[serde(default)]
    commands: Vec<Entry>,
}

/// One command of the shell tool.
#[derive(Debug, Deserialize)]
struct Entry {
    id: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    visibility: Option<String>,
    #[serde(default)]
    class: Option<String>,
    #[serde(default)]
    requires_repository: Option<bool>,
    #[serde(default)]
    requires_task: Option<String>,
    #[serde(default)]
    syntax: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

/// What the read produced.
pub struct Contribution {
    /// The nodes, in registry order.
    pub nodes: Vec<CommandNode>,
    /// What the read found.
    pub diagnostics: Vec<Diagnostic>,
}

/// Read the shell tool's commands from the shipped registry under `share`.
///
/// A repository without the registry contributes nothing and is not an error: the graph is
/// a projection of what is here, and an installation that ships only the executable has no
/// shell tool to describe.
pub fn contribute(share: &Path) -> Contribution {
    let path = share.join(REGISTRY);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Contribution {
            nodes: Vec::new(),
            diagnostics: Vec::new(),
        };
    };
    let registry: Registry = match yaml::parse_into(&text) {
        Ok(r) => r,
        Err(e) => {
            return Contribution {
                nodes: Vec::new(),
                diagnostics: vec![Diagnostic::error(
                    "tool-registry-unreadable",
                    format!("{DECLARATION} could not be read: {e}"),
                    Vec::new(),
                )],
            }
        }
    };
    let mut diagnostics = Vec::new();
    if registry.version != 1 {
        diagnostics.push(Diagnostic::warning(
            "tool-registry-version",
            format!(
                "{DECLARATION} declares version {}, and this reader knows version 1",
                registry.version
            ),
            Vec::new(),
        ));
    }

    let nodes = registry
        .commands
        .iter()
        // An internal command is dispatched and absent from the help text; the graph shows
        // what a person may run, and the registry is what decides which those are.
        .filter(|e| e.visibility.as_deref() != Some("internal"))
        .map(node)
        .collect();

    Contribution { nodes, diagnostics }
}

/// One node from one registry entry.
fn node(entry: &Entry) -> CommandNode {
    let path = vec![entry.id.clone()];
    let effect = effect_of(entry.class.as_deref());
    let interactivity = interactivity_of(entry.syntax.as_deref());

    let mut requires = Vec::new();
    if entry.requires_repository.unwrap_or(true) {
        requires.push(Requirement::Repository);
    }
    requires.push(Requirement::Layer);
    if entry.requires_task.as_deref() == Some("required") {
        requires.push(Requirement::Task);
    }

    CommandNode {
        id: CommandId::derive(Origin::Tool, &path),
        origin: Origin::Tool,
        invocation: format!("majordomus {}", entry.id),
        summary: entry.summary.clone(),
        description: entry.note.clone(),
        runnable: true,
        // The shell tool's arguments are prose in its own help text and are not declared
        // in a form anything can read. The graph says so by carrying none rather than by
        // inventing some: a completion offers the command, and the command explains
        // itself.
        arguments: Vec::new(),
        execution: Execution {
            origin: Origin::Tool,
            argv: path.clone(),
        },
        effect,
        interactivity,
        stability: crate::capability::Stability::BehaviorallyVerified,
        availability: Availability {
            available: true,
            reason: None,
            requires,
        },
        group: entry.category.clone(),
        tags: Vec::new(),
        aliases: Vec::new(),
        deprecation: None,
        provenance: Provenance {
            declared_in: DECLARATION.into(),
            read_by: Some(format!("{DISPATCH} (the dispatch table it is reconciled against)")),
            capability: None,
        },
        projections: Projections::default(),
        entrypoint: Some(true),
        path,
    }
}

/// The graph's effect for the registry's class.
///
/// An unknown class is the strongest reading rather than the weakest: a class this reader
/// does not recognise is a class added after it, and treating it as read-only would
/// advertise it to every machine surface.
fn effect_of(class: Option<&str>) -> Effect {
    match class {
        Some("read-only") => Effect::ReadOnly,
        Some("state-mutating") => Effect::LocalMutation,
        Some("generated-output-mutating") => Effect::RepositoryMutation,
        _ => Effect::RepositoryMutation,
    }
}

/// Whether the declared synopsis holds a terminal.
fn interactivity_of(syntax: Option<&str>) -> Interactivity {
    match syntax {
        Some(s) if s.contains('<') && !s.contains("<--") => Interactivity::Interactive,
        _ => Interactivity::NonInteractive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_translate_to_effects() {
        assert_eq!(effect_of(Some("read-only")), Effect::ReadOnly);
        assert_eq!(effect_of(Some("state-mutating")), Effect::LocalMutation);
        assert_eq!(
            effect_of(Some("generated-output-mutating")),
            Effect::RepositoryMutation
        );
        // A class from a later version of the registry is read as the strongest, not the
        // weakest: an unknown effect must not reach a machine surface.
        assert_eq!(effect_of(Some("something-new")), Effect::RepositoryMutation);
        assert_eq!(effect_of(None), Effect::RepositoryMutation);
    }

    #[test]
    fn a_body_on_standard_input_is_interactive() {
        assert_eq!(
            interactivity_of(Some("majordomus checkpoint [--derive] < body.md")),
            Interactivity::Interactive
        );
        assert_eq!(
            interactivity_of(Some("majordomus check [--json]")),
            Interactivity::NonInteractive
        );
    }

    #[test]
    fn the_shipped_registry_reads() {
        let share = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|root| root.join("share"))
            .expect("the repository root is two levels above the crate");
        if !share.join(REGISTRY).exists() {
            return; // an installation without the shell tool contributes nothing
        }
        let c = contribute(&share);
        assert!(c.nodes.len() > 20, "read {} commands", c.nodes.len());
        let check = c
            .nodes
            .iter()
            .find(|n| n.id.as_str() == "tool.check")
            .expect("check is a public command of the shell tool");
        assert_eq!(check.invocation, "majordomus check");
        assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    }
}
