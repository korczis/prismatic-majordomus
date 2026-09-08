//! Workflows the repository keeps outside the executable, discovered where they live.
//!
//! Some steps of a repository are not, and should not become, Rust: a release script, a
//! site build, a probe that drives a browser. They are still things a person can be told
//! to run, so they belong in the graph — as discovered nodes with their provenance, never
//! as a list this module keeps.
//!
//! `just` already holds them, and already publishes them as structured data:
//! `just --dump --dump-format json` gives every recipe with its doc comment, its
//! parameters, its group and its attributes. That is what is read. Nothing here parses
//! justfile syntax, and nothing here runs a recipe.
//!
//! The one semantic inference made is the repository's own convention: a recipe carrying
//! `[confirm]` destroys something. Everything else is classified as a local change, and no
//! discovered workflow is ever offered to a machine surface — the executable cannot know
//! what a script does, and a guess in that direction is the one that cannot be taken back.

use std::path::Path;
use std::process::Command;

use crate::control::effect::{EffectClass, Interactivity, Semantics};
use crate::control::graph::{Argument, Availability, CommandNode, Execution, ValueSource};
use crate::control::projection::{self, Projections};

/// One discovered workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workflow {
    /// The recipe's name, which is also its spelling on the `just` surface.
    pub name: String,
    /// Its doc comment, when it has one.
    pub doc: Option<String>,
    /// Its group, when it declares one.
    pub group: Option<String>,
    /// Its parameters, in order.
    pub parameters: Vec<Parameter>,
    /// Whether it asks before it runs, which is how this repository marks a recipe that
    /// destroys something.
    pub confirms: bool,
    /// Where it was discovered, repository-relative.
    pub source: String,
}

/// One parameter of a recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Its name.
    pub name: String,
    /// Whether it takes everything that follows.
    pub variadic: bool,
    /// Its default, when it has one.
    pub default: Option<String>,
}

impl Workflow {
    /// What running it does, by this repository's own convention.
    pub fn semantics(&self) -> Semantics {
        Semantics {
            effect: if self.confirms {
                EffectClass::Destructive
            } else {
                EffectClass::LocalMutation
            },
            interactivity: Interactivity::Batch,
        }
    }

    /// The graph node for it. It is on the `just` surface and on no other: it has no
    /// command line of its own, and it is never offered to a machine.
    pub fn node(&self) -> CommandNode {
        let path = vec!["workflow".to_string(), self.name.clone()];
        CommandNode {
            id: format!("workflow.{}", self.name),
            summary: self
                .doc
                .clone()
                .unwrap_or_else(|| format!("the `{}` workflow", self.name)),
            description: None,
            usage: format!("just {}", self.name),
            group: false,
            semantics: Some(self.semantics()),
            execution: Execution::External {
                runner: format!("just {}", self.name),
                source: self.source.clone(),
            },
            arguments: self
                .parameters
                .iter()
                .map(|p| Argument {
                    name: p.name.clone(),
                    long: None,
                    short: None,
                    positional: true,
                    help: String::new(),
                    value_name: None,
                    takes_value: true,
                    required: p.default.is_none() && !p.variadic,
                    defaults: p.default.clone().into_iter().collect(),
                    values: ValueSource::None,
                })
                .collect(),
            examples: Vec::new(),
            projections: Projections {
                cli: Vec::new(),
                just: Some(self.name.clone()),
                mcp: None,
                http: None,
                cockpit: None,
                docs: projection::docs_route(&path),
            },
            availability: Availability::Available,
            provenance: self.source.clone(),
            path,
        }
    }
}

/// Every workflow `just` holds for this repository, from its own structured dump.
///
/// Absent `just`, an unreadable dump or a dump this version does not understand is not an
/// error: the repository has no discovered workflows and the reason is returned, because a
/// graph without them is still a graph.
pub fn discover(root: &Path) -> Result<Vec<Workflow>, String> {
    let out = Command::new("just")
        .arg("--dump")
        .arg("--dump-format")
        .arg("json")
        .arg("--unstable")
        .current_dir(root)
        .output()
        .map_err(|e| format!("just could not be run: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "just --dump exited {}: {}",
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("just's dump is not JSON: {e}"))?;
    Ok(parse(&value, "justfile"))
}

/// The workflows in one dump. Separate from the process so that the shape `just` produces
/// is tested without `just` being installed.
pub fn parse(dump: &serde_json::Value, source: &str) -> Vec<Workflow> {
    let Some(recipes) = dump.get("recipes").and_then(|r| r.as_object()) else {
        return Vec::new();
    };
    let mut out: Vec<Workflow> = recipes
        .iter()
        .filter(|(_, r)| !r.get("private").and_then(|p| p.as_bool()).unwrap_or(false))
        .map(|(name, r)| Workflow {
            name: name.clone(),
            doc: r
                .get("doc")
                .and_then(|d| d.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            group: r
                .get("attributes")
                .and_then(|a| a.as_array())
                .and_then(|a| a.iter().find_map(group_of)),
            confirms: r
                .get("attributes")
                .and_then(|a| a.as_array())
                .is_some_and(|a| a.iter().any(is_confirm)),
            parameters: r
                .get("parameters")
                .and_then(|p| p.as_array())
                .map(|p| {
                    p.iter()
                        .map(|param| Parameter {
                            name: param
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or_default()
                                .to_string(),
                            variadic: !matches!(
                                param.get("kind").and_then(|k| k.as_str()),
                                None | Some("singular")
                            ),
                            default: param
                                .get("default")
                                .and_then(|d| d.as_str())
                                .map(str::to_string),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            source: source.to_string(),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// An attribute is either a bare name or an object with one key; both shapes appear in
/// `just`'s dump across versions, and both are read rather than one being assumed.
fn attribute_name(value: &serde_json::Value) -> Option<&str> {
    value.as_str().or_else(|| {
        value
            .as_object()
            .and_then(|o| o.keys().next().map(|k| k.as_str()))
    })
}

fn is_confirm(value: &serde_json::Value) -> bool {
    attribute_name(value).is_some_and(|n| n.eq_ignore_ascii_case("confirm"))
}

fn group_of(value: &serde_json::Value) -> Option<String> {
    let name = attribute_name(value)?;
    if !name.eq_ignore_ascii_case("group") {
        return None;
    }
    value
        .as_object()
        .and_then(|o| o.values().next())
        .and_then(|v| v.as_str().or_else(|| v.as_array()?.first()?.as_str()))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dump() -> serde_json::Value {
        serde_json::json!({
            "recipes": {
                "site-build": {
                    "doc": "Build the site.",
                    "private": false,
                    "attributes": [{"group": ["site"]}],
                    "parameters": [{"name": "args", "kind": "star", "default": null}]
                },
                "clean": {
                    "doc": "Remove the build output.",
                    "private": false,
                    "attributes": ["confirm"],
                    "parameters": []
                },
                "default": {
                    "doc": null,
                    "private": true,
                    "attributes": [],
                    "parameters": []
                }
            }
        })
    }

    #[test]
    fn a_private_recipe_is_not_a_workflow() {
        let found = parse(&dump(), "justfile");
        assert_eq!(
            found.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(),
            vec!["clean", "site-build"]
        );
    }

    #[test]
    fn confirm_is_read_as_destruction_and_a_group_is_read_as_a_group() {
        let found = parse(&dump(), "justfile");
        let clean = found.iter().find(|w| w.name == "clean").unwrap();
        assert_eq!(clean.semantics().effect, EffectClass::Destructive);
        let site = found.iter().find(|w| w.name == "site-build").unwrap();
        assert_eq!(site.group.as_deref(), Some("site"));
        assert_eq!(site.semantics().effect, EffectClass::LocalMutation);
        assert!(site.parameters[0].variadic);
    }

    #[test]
    fn a_discovered_workflow_is_never_offered_to_a_machine() {
        for w in parse(&dump(), "justfile") {
            let node = w.node();
            assert!(node.projections.mcp.is_none());
            assert!(node.projections.cockpit.is_none());
            assert!(node.projections.cli.is_empty());
        }
    }
}
