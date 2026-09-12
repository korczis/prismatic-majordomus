//! Workflows: the recipes a person can run here, read from the workflow runner's own
//! structured description of them.
//!
//! `just --dump --dump-format json` already answers every question worth asking about a
//! recipe — its name, its doc comment, its `[group(...)]`, its parameters, its
//! dependencies, whether it is private, whether it asks before running. So nothing here
//! parses a `justfile`. A second parser for a file its own tool can describe is a second
//! source of truth that goes stale the first time the syntax grows.
//!
//! # The recommended commands
//!
//! The banner shows a handful of commands, and the one thing it must not do is carry a
//! list of them: a hard-coded `just test` outlives the recipe it names. The rule instead
//! is one sentence, and the justfile is what decides its outcome:
//!
//! > **The entry point of a group is the public recipe whose name is the group's name.**
//!
//! A group without such a recipe has no entry point and contributes nothing. Groups are
//! offered in the order the justfile declares them, so moving a group up the file moves
//! its command up the banner, and naming a recipe after its group is what publishes it.
//! There is no metadata to invent, no annotation to remember, and no configuration file
//! that can disagree with the justfile.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde_json::Value;

use super::{
    TierState, WorkflowCatalogue, WorkflowDescriptor, WorkflowEntrypoint, WorkflowParameter,
};

/// The command whose output is read. Named here so the snapshot's provenance can quote it.
pub const SOURCE: &str = "just --dump --dump-format json";

/// The command that orders the groups. `--dump` sorts recipes by name and reports no group
/// order, so declaration order — the only order a person recognises — comes from here.
pub const GROUP_SOURCE: &str = "just --groups --unsorted";

/// How long the runner may take. It reads one file and prints a document; a second is
/// already pathological, and a snapshot is better without it than late.
pub const TIMEOUT: Duration = Duration::from_secs(2);

/// Read the catalogue by asking `just` about the repository at `root`.
///
/// Returns [`WorkflowCatalogue::unavailable`] when `just` is not installed, when the
/// repository has no justfile, or when the runner refuses it — none of which is an error
/// here: a repository is not obliged to have a workflow runner.
pub fn resolve(root: &Path) -> WorkflowCatalogue {
    let Some(dump) = run(root, &["--dump", "--dump-format", "json"]) else {
        return WorkflowCatalogue::unavailable();
    };
    let Ok(document): Result<Value, _> = serde_json::from_str(&dump) else {
        return WorkflowCatalogue::unavailable();
    };
    let workflows = workflows_of(&document);
    let groups = run(root, &["--groups", "--unsorted"])
        .map(|text| group_order(&text))
        .unwrap_or_default();
    let entrypoints = entrypoints(&workflows, &groups);
    WorkflowCatalogue {
        state: TierState::Resolved,
        source: Some(SOURCE.into()),
        workflows,
        entrypoints,
    }
}

/// Every public workflow of a `just --dump --dump-format json` document, sorted by name.
///
/// Recipes of imported modules are included under their module's name, which is how a
/// modular justfile stays one catalogue rather than becoming several.
///
/// ```
/// use majordomus_cli::environment::workflows::workflows_of;
/// let document = serde_json::json!({
///     "recipes": {
///         "build": { "name": "build", "doc": "Build it.", "private": false,
///                    "attributes": [{"group": "build"}], "parameters": [], "dependencies": [] },
///         "_hidden": { "name": "_hidden", "doc": null, "private": true,
///                      "attributes": [], "parameters": [], "dependencies": [] }
///     },
///     "modules": {}
/// });
/// let found = workflows_of(&document);
/// assert_eq!(found.len(), 1, "a private recipe is not a workflow");
/// assert_eq!(found[0].name, "build");
/// assert_eq!(found[0].group.as_deref(), Some("build"));
/// ```
pub fn workflows_of(document: &Value) -> Vec<WorkflowDescriptor> {
    let mut out = Vec::new();
    collect(document, None, &mut out);
    crate::order::canonical(&mut out);
    out
}

fn collect(document: &Value, namespace: Option<&str>, out: &mut Vec<WorkflowDescriptor>) {
    for (name, recipe) in document
        .get("recipes")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        // `just` marks a recipe private two ways: the `[private]` attribute, which it
        // reports as `private`, and a leading underscore, which it does not. Both mean
        // "not for a person to run", and a catalogue that showed the second would offer
        // commands the runner hides from `--list`.
        let private =
            recipe.get("private").and_then(Value::as_bool) == Some(true) || name.starts_with('_');
        if private {
            continue;
        }
        let attributes = recipe.get("attributes").and_then(Value::as_array);
        out.push(WorkflowDescriptor {
            name: match namespace {
                Some(module) => format!("{module} {name}"),
                None => name.clone(),
            },
            namespace: namespace.map(str::to_string),
            description: recipe
                .get("doc")
                .and_then(Value::as_str)
                .map(str::to_string),
            group: attributes.and_then(|a| attribute(a, "group")),
            parameters: recipe
                .get("parameters")
                .and_then(Value::as_array)
                .map(|params| params.iter().map(parameter).collect())
                .unwrap_or_default(),
            dependencies: recipe
                .get("dependencies")
                .and_then(Value::as_array)
                .map(|deps| {
                    deps.iter()
                        .filter_map(|d| d.get("recipe").and_then(Value::as_str))
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            confirm: attributes.is_some_and(|a| attribute(a, "confirm").is_some()),
        });
    }
    for (module, sub) in document
        .get("modules")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        collect(sub, Some(module), out);
    }
}

/// The value of a named attribute. `just` reports an attribute either as a bare string
/// (`"private"`) or as a single-member object (`{"group": "test"}`).
fn attribute(attributes: &[Value], name: &str) -> Option<String> {
    attributes.iter().find_map(|a| match a {
        Value::String(s) if s == name => Some(String::new()),
        Value::Object(m) => m.get(name).map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }),
        _ => None,
    })
}

fn parameter(value: &Value) -> WorkflowParameter {
    // `kind` is `singular`, `plus` (one or more) or `star` (any number).
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("singular");
    let has_default = value
        .get("default")
        .is_some_and(|d| !matches!(d, Value::Null));
    WorkflowParameter {
        name: value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        variadic: matches!(kind, "star" | "plus"),
        required: !has_default && !matches!(kind, "star"),
    }
}

/// The groups in the order `just --groups --unsorted` printed them.
///
/// ```
/// use majordomus_cli::environment::workflows::group_order;
/// let text = "Recipe groups:\n    build\n    serve\n    test\n";
/// assert_eq!(group_order(text), ["build", "serve", "test"]);
/// ```
pub fn group_order(text: &str) -> Vec<String> {
    text.lines()
        .skip_while(|l| !l.trim_end().ends_with(':'))
        .skip(1)
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// The entry point of each group: the public recipe whose name is the group's name.
///
/// Groups are offered in `order`; a group with no such recipe contributes nothing, and a
/// group not in `order` follows the ones that are, by name, so a runner that could not be
/// asked for the declaration order still produces a deterministic answer.
///
/// ```
/// use majordomus_cli::environment::workflows::{entrypoints, workflows_of};
/// let document = serde_json::json!({ "modules": {}, "recipes": {
///     "check":  { "name": "check",  "doc": "Every gate.", "private": false,
///                 "attributes": [{"group": "check"}], "parameters": [], "dependencies": [] },
///     "check-fast": { "name": "check-fast", "doc": null, "private": false,
///                 "attributes": [{"group": "check"}], "parameters": [], "dependencies": [] },
///     "site-build": { "name": "site-build", "doc": null, "private": false,
///                 "attributes": [{"group": "site"}], "parameters": [], "dependencies": [] }
/// }});
/// let found = entrypoints(&workflows_of(&document), &["check".into(), "site".into()]);
/// assert_eq!(found.len(), 1, "site has no recipe named `site`, so it offers none");
/// assert_eq!(found[0].command, "just check");
/// ```
pub fn entrypoints(workflows: &[WorkflowDescriptor], order: &[String]) -> Vec<WorkflowEntrypoint> {
    let mut groups: Vec<String> = order.to_vec();
    let mut rest: Vec<String> = workflows
        .iter()
        .filter_map(|w| w.group.clone())
        .filter(|g| !groups.contains(g))
        .collect();
    crate::order::canonical(&mut rest);
    rest.dedup();
    groups.extend(rest);

    groups
        .into_iter()
        .filter_map(|group| {
            let workflow = workflows
                .iter()
                .find(|w| w.group.as_deref() == Some(group.as_str()) && w.name == group)?;
            Some(WorkflowEntrypoint {
                command: format!("just {}", workflow.name),
                workflow: workflow.name.clone(),
                description: workflow.description.clone(),
                group,
            })
        })
        .collect()
}

fn run(root: &Path, args: &[&str]) -> Option<String> {
    let out = super::probe::bounded_output(
        Command::new("just")
            .arg("--justfile")
            .arg(root.join("justfile"))
            .args(args),
        TIMEOUT,
    )?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> Value {
        serde_json::json!({
            "modules": {
                "site": { "modules": {}, "recipes": {
                    "build": { "name": "build", "doc": "Build the site.", "private": false,
                               "attributes": [], "parameters": [], "dependencies": [] }
                }}
            },
            "recipes": {
                "test": { "name": "test", "doc": "Every gate.", "private": false,
                          "attributes": [{"group": "test"}], "parameters": [],
                          "dependencies": [{"recipe": "build"}] },
                "clean": { "name": "clean", "doc": "Remove it.", "private": false,
                           "attributes": [{"confirm": "Sure? [y/N]"}, {"group": "build"}],
                           "parameters": [], "dependencies": [] },
                "build": { "name": "build", "doc": null, "private": false,
                           "attributes": [{"group": "build"}],
                           "parameters": [{"name": "args", "kind": "star", "default": null}],
                           "dependencies": [] },
                "default": { "name": "default", "doc": null, "private": true,
                             "attributes": [], "parameters": [], "dependencies": [] }
            }
        })
    }

    #[test]
    fn a_private_recipe_is_not_a_workflow() {
        let found = workflows_of(&document());
        assert!(!found.iter().any(|w| w.name == "default"));
    }

    #[test]
    fn recipes_come_back_in_a_stable_order_whatever_the_runner_emitted() {
        let found = workflows_of(&document());
        let names: Vec<&str> = found.iter().map(|w| w.name.as_str()).collect();
        assert_eq!(names, ["build", "clean", "site build", "test"]);
    }

    #[test]
    fn a_modules_recipe_carries_its_module_in_its_name() {
        let found = workflows_of(&document());
        let module_recipe = found
            .iter()
            .find(|w| w.namespace.as_deref() == Some("site"))
            .expect("the module's recipe");
        assert_eq!(module_recipe.name, "site build");
        assert_eq!(
            module_recipe.description.as_deref(),
            Some("Build the site.")
        );
    }

    #[test]
    fn attributes_become_the_facts_a_reader_needs() {
        let found = workflows_of(&document());
        let clean = found.iter().find(|w| w.name == "clean").expect("clean");
        assert!(clean.confirm, "a recipe that asks first says so");
        assert_eq!(clean.group.as_deref(), Some("build"));
        let build = found.iter().find(|w| w.name == "build").expect("build");
        assert!(!build.confirm);
        assert_eq!(build.parameters[0].name, "args");
        assert!(build.parameters[0].variadic);
        assert!(
            !build.parameters[0].required,
            "a star parameter is optional"
        );
    }

    /// The rule the banner's commands come from, stated as an assertion: a group offers
    /// the recipe named after it, in the order the justfile declares its groups, and a
    /// group with no such recipe offers nothing rather than something arbitrary.
    #[test]
    fn the_entrypoint_of_a_group_is_the_recipe_named_after_it() {
        let workflows = workflows_of(&document());
        let found = entrypoints(&workflows, &["test".into(), "build".into()]);
        let commands: Vec<&str> = found.iter().map(|e| e.command.as_str()).collect();
        assert_eq!(
            commands,
            ["just test", "just build"],
            "declaration order wins"
        );
    }

    #[test]
    fn a_group_order_that_could_not_be_read_still_answers_deterministically() {
        let workflows = workflows_of(&document());
        let found = entrypoints(&workflows, &[]);
        let commands: Vec<&str> = found.iter().map(|e| e.command.as_str()).collect();
        assert_eq!(commands, ["just build", "just test"], "then by group name");
    }

    #[test]
    fn a_repository_without_a_workflow_runner_is_not_an_error() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let catalogue = resolve(dir.path());
        assert_eq!(catalogue.state, TierState::Unavailable);
        assert!(catalogue.workflows.is_empty());
        assert!(catalogue.entrypoints.is_empty());
    }

    #[test]
    fn the_group_listing_header_is_not_read_as_a_group() {
        assert_eq!(group_order("Recipe groups:\n    a\n    b\n"), ["a", "b"]);
        assert_eq!(group_order(""), Vec::<String>::new());
    }
}
