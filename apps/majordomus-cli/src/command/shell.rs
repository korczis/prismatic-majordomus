//! The shell contributor: the task lifecycle's commands, read from the registry that
//! already owns them.
//!
//! `share/commands.yaml` is authority for what a command of the shell tool means — its
//! summary, its category, its stage, its visibility, what it reads and writes, and the
//! class that says what it does to the repository. The index already reads that file as
//! objects of the `command` kind, and this contributor reads the same file directly so
//! that the graph can be built on the cheap path, without an index, for the shell
//! surfaces that must answer in milliseconds.
//!
//! Nothing is restated here. The class becomes the effect through the one mapping in
//! [`super::model::EffectClass::of_shell_class`]; the summary, the syntax and the
//! visibility are carried through as they are written.

use std::path::Path;

use serde_json::Value;

use super::model::{
    Availability, CommandId, CommandNode, EffectClass, Execution, Interactivity, Projections,
    Program, Provenance, Requirement, Visibility,
};
use crate::metadata::yaml;

/// Where the registry lives inside the tool distribution.
pub const REGISTRY: &str = "share/commands.yaml";

/// Every command of the shell tool, from the registry in this distribution. A registry
/// that cannot be read yields no commands and one diagnostic from the caller: the graph
/// still stands, with the native commands in it.
pub fn contribute(share_dir: &Path) -> Result<Vec<CommandNode>, String> {
    let path = share_dir.join("commands.yaml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let map = yaml::parse_mapping(&text)?;
    let Some(Value::Array(entries)) = map.get("commands") else {
        return Err(format!("{REGISTRY} has no `commands` list"));
    };
    Ok(entries.iter().filter_map(node).collect())
}

fn node(entry: &Value) -> Option<CommandNode> {
    let obj = entry.as_object()?;
    let text = |key: &str| obj.get(key).and_then(yaml::scalar_string);
    let id_word = text("id")?;
    let path = vec![id_word.clone()];
    let summary = text("summary").unwrap_or_default();
    let class = text("class").unwrap_or_default();
    let effect = EffectClass::of_shell_class(&class).unwrap_or(EffectClass::LocalState);
    let visibility = match text("visibility").as_deref() {
        Some("internal") => Visibility::Internal,
        _ => Visibility::Public,
    };
    let mut requires = vec![Requirement::Always];
    if text("requires_repository").as_deref() == Some("true") {
        requires = vec![Requirement::Repository];
    }
    let mut tags: Vec<String> = Vec::new();
    if let Some(category) = text("category") {
        tags.push(category);
    }
    if let Some(stage) = text("stage") {
        tags.push(format!("stage:{stage}"));
    }
    Some(CommandNode {
        id: CommandId::new(Program::Shell, &path),
        program: Program::Shell,
        summary,
        // The registry's `note` is the paragraph a reader gets on the command's page; it
        // is the long description everywhere, and is written in exactly one place.
        description: text("note"),
        // The registry's `syntax` is the usage line the tool's own help prints. One text.
        usage: text("syntax").unwrap_or_else(|| format!("majordomus {id_word}")),
        runnable: true,
        // The shell tool's arguments are not typed anywhere: `syntax` is prose, and
        // parsing prose into an argument model would invent facts the registry does not
        // state. A shell command therefore completes to its name and forwards whatever
        // follows, which is exactly what the tool accepts.
        arguments: Vec::new(),
        execution: Execution::Shell { argv: path.clone() },
        effect,
        interactivity: Interactivity::NonInteractive,
        visibility,
        availability: Availability {
            available: true,
            requires,
            reason: None,
        },
        provenance: Provenance::Registry {
            path: REGISTRY.to_string(),
        },
        aliases: Vec::new(),
        deprecation: None,
        tags,
        examples: Vec::new(),
        path,
        projections: Projections::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry(body: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("commands.yaml"), body).unwrap();
        dir
    }

    #[test]
    fn the_registry_class_becomes_the_effect_and_nothing_is_restated() {
        let dir = registry(
            "version: 1\ncommands:\n  - id: doctor\n    title: doctor\n    summary: Is Majordomus healthy here?\n    category: system\n    stage: inspect\n    visibility: public\n    class: read-only\n    requires_repository: true\n    syntax: majordomus doctor [--json]\n    note: The diagnosis.\n",
        );
        let nodes = contribute(dir.path()).unwrap();
        assert_eq!(nodes.len(), 1);
        let n = &nodes[0];
        assert_eq!(n.id.as_str(), "shell.doctor");
        assert_eq!(n.effect, EffectClass::ReadOnly);
        assert_eq!(n.usage, "majordomus doctor [--json]");
        assert_eq!(n.summary, "Is Majordomus healthy here?");
        assert_eq!(n.availability.requires, vec![Requirement::Repository]);
        assert!(n.tags.contains(&"system".to_string()));
        assert!(n.tags.contains(&"stage:inspect".to_string()));
    }

    #[test]
    fn an_internal_command_is_carried_and_marked_rather_than_dropped() {
        let dir = registry(
            "version: 1\ncommands:\n  - id: help\n    summary: The help text.\n    visibility: internal\n    class: read-only\n",
        );
        let nodes = contribute(dir.path()).unwrap();
        assert_eq!(nodes[0].visibility, Visibility::Internal);
        assert!(!nodes[0].is_listed());
    }

    #[test]
    fn a_registry_that_cannot_be_read_is_an_error_and_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        assert!(contribute(dir.path()).is_err());
        let dir = registry("version: 1\n");
        assert!(contribute(dir.path()).unwrap_err().contains("commands"));
    }

    #[test]
    fn this_distributions_registry_yields_the_lifecycle_commands() {
        let share = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../share")
            .canonicalize()
            .expect("the distribution beside the crate");
        let nodes = contribute(&share).expect("the shipped registry");
        let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
        for expected in ["shell.doctor", "shell.check", "shell.context", "shell.start"] {
            assert!(ids.contains(&expected), "{expected} missing from {ids:?}");
        }
    }
}
