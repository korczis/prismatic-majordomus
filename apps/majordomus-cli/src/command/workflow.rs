//! The workflow contributor: everything this repository can run that neither executable
//! implements, declared one file at a time and discovered.
//!
//! A workflow is a script of this repository — the site build, the CI plan, the release
//! packaging — that legitimately stays outside the Rust executable. Before this
//! contributor existed each of them was a recipe in the `justfile` with its description
//! written beside it, which meant the description existed twice and the workflow existed
//! nowhere a machine could read.
//!
//! Now each is one object under [`DIRECTORY`]: a typed, schema-validated file naming what
//! it does, what it needs, what it changes and the argument vectors it runs. Dropping a
//! file there is the whole registration: the graph discovers it, the Just bridge projects
//! it, completion offers it, the reference documents it and the Cockpit lists it. No
//! central list names it, here or anywhere else.

use std::path::Path;

use serde_json::Value;

use super::model::{
    Availability, CommandId, CommandNode, Deprecation, EffectClass, Execution, Interactivity,
    Projections, Program, Provenance, Requirement, Visibility,
};
use crate::metadata::yaml;

/// Where workflow objects live, relative to the repository root. A conventional tree, not
/// a manifest: every `.yaml` file directly under it is one workflow.
pub const DIRECTORY: &str = ".ai/repo/workflows/commands";

/// The document schema every workflow object declares.
pub const SCHEMA: &str = "workflow-command/v1";

/// Every workflow declared in a repository, in identity order so that discovery order
/// cannot reach the output. A repository with no such directory has no workflows, which
/// is not an error.
pub fn contribute(repo_root: &Path) -> (Vec<CommandNode>, Vec<String>) {
    let dir = repo_root.join(DIRECTORY);
    let mut nodes = Vec::new();
    let mut problems = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return (nodes, problems);
    };
    let mut paths: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .collect();
    paths.sort();
    for path in paths {
        let relative = format!(
            "{DIRECTORY}/{}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        match read(&path, &relative) {
            Ok(node) => nodes.push(node),
            Err(reason) => problems.push(format!("{relative}: {reason}")),
        }
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    (nodes, problems)
}

fn read(path: &Path, relative: &str) -> Result<CommandNode, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let map = yaml::parse_mapping(&text)?;
    let text_of = |key: &str| map.get(key).and_then(yaml::scalar_string);
    let declared_schema = text_of("schema").unwrap_or_default();
    if declared_schema != SCHEMA {
        return Err(format!(
            "declares schema '{declared_schema}', and this executable reads '{SCHEMA}'"
        ));
    }
    let id_word = text_of("id").ok_or("no id")?;
    if !is_identity(&id_word) {
        return Err(format!("id '{id_word}' is not [a-z0-9][a-z0-9-]*"));
    }
    let summary = text_of("summary").ok_or("no summary")?;
    let effect = text_of("effect")
        .ok_or("no effect")
        .and_then(|e| effect_class(&e).ok_or(format!("effect '{e}' is not one of the classes")))?;
    let steps = steps(map.get("steps"))?;
    if steps.is_empty() {
        return Err("no steps: a workflow that runs nothing is not a command".into());
    }
    let path_words = vec![id_word.clone()];
    Ok(CommandNode {
        id: CommandId::new(Program::Workflow, &path_words),
        program: Program::Workflow,
        summary,
        description: text_of("description"),
        usage: format!(
            "just {id_word}{}",
            if truthy(map.get("variadic")) {
                " [ARGS]..."
            } else {
                ""
            }
        ),
        runnable: true,
        arguments: Vec::new(),
        execution: Execution::External { steps },
        effect,
        interactivity: match text_of("interactivity").as_deref() {
            Some("tty-required") => Interactivity::TtyRequired,
            Some("tty-preferred") => Interactivity::TtyPreferred,
            _ => Interactivity::NonInteractive,
        },
        visibility: match text_of("visibility").as_deref() {
            Some("internal") => Visibility::Internal,
            _ => Visibility::Public,
        },
        availability: Availability {
            available: true,
            requires: requirements(map.get("requires"))?,
            reason: None,
        },
        provenance: Provenance::Declared {
            path: relative.to_string(),
        },
        aliases: aliases(map.get("aliases")),
        deprecation: text_of("deprecated").map(|note| Deprecation {
            replaced_by: None,
            note,
        }),
        tags: strings(map.get("tags")),
        examples: Vec::new(),
        path: path_words,
        projections: Projections::default(),
    })
}

fn is_identity(word: &str) -> bool {
    !word.is_empty()
        && !word.starts_with('-')
        && word
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn effect_class(text: &str) -> Option<EffectClass> {
    match text {
        "read-only" => Some(EffectClass::ReadOnly),
        "process-state" => Some(EffectClass::ProcessState),
        "local-state" => Some(EffectClass::LocalState),
        "generated-output" => Some(EffectClass::GeneratedOutput),
        "network" => Some(EffectClass::Network),
        "destructive" => Some(EffectClass::Destructive),
        _ => None,
    }
}

fn requirement(text: &str) -> Option<Requirement> {
    match text {
        "always" => Some(Requirement::Always),
        "repository" => Some(Requirement::Repository),
        "git" => Some(Requirement::Git),
        "cargo-toolchain" => Some(Requirement::CargoToolchain),
        "native-executable" => Some(Requirement::NativeExecutable),
        "site" => Some(Requirement::Site),
        "site-dependencies" => Some(Requirement::SiteDependencies),
        "browser" => Some(Requirement::Browser),
        _ => None,
    }
}

fn requirements(value: Option<&Value>) -> Result<Vec<Requirement>, String> {
    let names = strings(value);
    if names.is_empty() {
        return Ok(vec![Requirement::Always]);
    }
    names
        .iter()
        .map(|n| requirement(n).ok_or(format!("requirement '{n}' is not one this executable knows")))
        .collect()
}

fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(yaml::scalar_string).collect(),
        _ => Vec::new(),
    }
}

fn truthy(value: Option<&Value>) -> bool {
    matches!(value.and_then(yaml::scalar_string).as_deref(), Some("true"))
}

fn aliases(value: Option<&Value>) -> Vec<super::model::Alias> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                let o = item.as_object()?;
                Some(super::model::Alias {
                    name: o.get("name").and_then(yaml::scalar_string)?,
                    reason: o
                        .get("reason")
                        .and_then(yaml::scalar_string)
                        .unwrap_or_else(|| "a name this workflow also answers to".into()),
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The steps, each a whole argument vector. A step is a list of words, never a line for a
/// shell to split: nothing this repository declares is handed to a shell to re-parse, so
/// no argument of a workflow can become a command of its own.
fn steps(value: Option<&Value>) -> Result<Vec<Vec<String>>, String> {
    let Some(Value::Array(items)) = value else {
        return Err("steps must be a list of argument vectors".into());
    };
    let mut out = Vec::new();
    for item in items {
        let Value::Array(words) = item else {
            return Err("a step is a list of words, not a command line".into());
        };
        let words: Vec<String> = words.iter().filter_map(yaml::scalar_string).collect();
        if words.is_empty() {
            return Err("a step with no program in it".into());
        }
        out.push(words);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let commands = dir.path().join(DIRECTORY);
        std::fs::create_dir_all(&commands).unwrap();
        for (name, body) in files {
            std::fs::write(commands.join(name), body).unwrap();
        }
        dir
    }

    const SITE_BUILD: &str = "schema: workflow-command/v1\nid: site-build\ntitle: Build the website\nsummary: Build the website with zola.\neffect: generated-output\nrequires:\n  - site\ntags:\n  - site\nsteps:\n  - [scripts/site-build]\n";

    #[test]
    fn a_file_dropped_into_the_tree_is_the_whole_registration() {
        let dir = repo(&[("site-build.yaml", SITE_BUILD)]);
        let (nodes, problems) = contribute(dir.path());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(nodes.len(), 1);
        let n = &nodes[0];
        assert_eq!(n.id.as_str(), "workflow.site-build");
        assert_eq!(n.effect, EffectClass::GeneratedOutput);
        assert_eq!(n.availability.requires, vec![Requirement::Site]);
        assert!(matches!(&n.execution, Execution::External { steps } if steps == &[vec!["scripts/site-build".to_string()]]));
    }

    #[test]
    fn removing_the_file_removes_the_command() {
        let dir = repo(&[("site-build.yaml", SITE_BUILD)]);
        assert_eq!(contribute(dir.path()).0.len(), 1);
        std::fs::remove_file(dir.path().join(DIRECTORY).join("site-build.yaml")).unwrap();
        assert!(contribute(dir.path()).0.is_empty());
    }

    #[test]
    fn discovery_order_cannot_reach_the_output() {
        let dir = repo(&[
            ("z.yaml", &SITE_BUILD.replace("site-build", "zeta")),
            ("a.yaml", &SITE_BUILD.replace("site-build", "alpha")),
        ]);
        let ids: Vec<String> = contribute(dir.path())
            .0
            .iter()
            .map(|n| n.id.to_string())
            .collect();
        assert_eq!(ids, ["workflow.alpha", "workflow.zeta"]);
    }

    #[test]
    fn a_malformed_object_is_a_finding_and_the_rest_still_stand() {
        let dir = repo(&[
            ("site-build.yaml", SITE_BUILD),
            ("broken.yaml", "schema: workflow-command/v1\nid: broken\n"),
            ("wrong-schema.yaml", "schema: nope/v1\nid: x\n"),
            (
                "bad-effect.yaml",
                "schema: workflow-command/v1\nid: e\nsummary: x\neffect: whatever\nsteps:\n  - [true]\n",
            ),
        ]);
        let (nodes, problems) = contribute(dir.path());
        assert_eq!(nodes.len(), 1, "the good one still stands");
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("no summary")));
        assert!(problems.iter().any(|p| p.contains("declares schema")));
        assert!(problems.iter().any(|p| p.contains("effect")));
    }

    #[test]
    fn a_repository_with_no_workflow_tree_has_no_workflows_and_no_complaint() {
        let dir = tempfile::tempdir().unwrap();
        let (nodes, problems) = contribute(dir.path());
        assert!(nodes.is_empty() && problems.is_empty());
    }
}
