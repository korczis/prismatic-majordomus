//! The graph: the contributors composed, validated, named and fingerprinted.
//!
//! Composition is the whole of it. The graph owns no command; it asks each contributor
//! for the commands the source it reads already declares, resolves availability against
//! the facts of the checkout, asks [`super::projection`] for the names each surface knows
//! them by, and checks the invariants that make the result safe to project.
//!
//! Building it costs a walk of the clap declaration, one small YAML file and a directory
//! read: no index, no registry, no subprocess. That is what lets repository entry and tab
//! completion both go through it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::facts::Facts;
use super::model::{
    Availability, CommandId, CommandNode, Diagnostic, EffectClass, Execution, Program, Visibility,
    SCHEMA,
};
use super::{native, projection, shell, workflow};

/// A composed, validated command graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandGraph {
    /// [`SCHEMA`]: the identity of this document's shape.
    pub schema: String,
    /// The executable that built it.
    pub version: String,
    /// The hash of what the surfaces project: identities, paths, texts, arguments,
    /// execution, effects and names. Availability is *not* in it, so that a branch
    /// change, a dirty tree or an uninstalled dependency never invalidates a generated
    /// bridge that would come out identical.
    pub structure: String,
    /// The hash of the whole graph, availability included: the value a client caches on.
    pub fingerprint: String,
    /// Every command, in identity order.
    pub commands: Vec<CommandNode>,
    /// Every finding of the build, in the order they were made.
    pub diagnostics: Vec<Diagnostic>,
}

impl CommandGraph {
    /// Compose the graph of a repository. `share_dir` is the tool distribution the shell
    /// tool's registry is read from.
    ///
    /// A contributor that fails contributes a diagnostic and no commands: a repository
    /// with no workflow tree, or a distribution with no command registry, still has a
    /// graph, because the alternative is a repository entry that fails on a missing
    /// optional file.
    pub fn build(root: &Path, share_dir: &Path, facts: &Facts) -> Self {
        let _phase = crate::perf::phase(crate::perf::Phase::CommandGraphBuild);
        let mut diagnostics = Vec::new();
        let mut commands = native::contribute(&crate::cli::tree());
        match shell::contribute(share_dir) {
            Ok(nodes) => commands.extend(nodes),
            Err(reason) => diagnostics.push(Diagnostic {
                code: "contributor-unavailable".into(),
                fatal: false,
                message: format!("the shell tool's command registry was not read: {reason}"),
                commands: Vec::new(),
            }),
        }
        let (workflows, problems) = workflow::contribute(root);
        commands.extend(workflows);
        for problem in problems {
            diagnostics.push(Diagnostic {
                code: "contributor-object-invalid".into(),
                fatal: false,
                message: format!("a workflow object was not read: {problem}"),
                commands: Vec::new(),
            });
        }

        commands.sort_by(|a, b| a.id.cmp(&b.id));
        diagnostics.extend(validate(&commands));
        diagnostics.extend(projection::assign(&mut commands));
        resolve_availability(&mut commands, facts);

        let structure = structure_fingerprint(&commands);
        let fingerprint = whole_fingerprint(&structure, &commands);
        CommandGraph {
            schema: SCHEMA.to_string(),
            version: crate::VERSION.to_string(),
            structure,
            fingerprint,
            commands,
            diagnostics,
        }
    }

    /// One command by identity.
    pub fn get(&self, id: &str) -> Option<&CommandNode> {
        self.commands.iter().find(|c| c.id.as_str() == id)
    }

    /// The command a surface's name resolves to. One resolution for every surface: the
    /// Just recipe, the recipe's aliases, the Cockpit action and the native path all
    /// reach the same node, and no surface keeps a table of its own.
    ///
    /// ```
    /// use majordomus_cli::command::{CommandGraph, Surface};
    /// # let dir = tempfile::tempdir().unwrap();
    /// # let share = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
    /// let facts = majordomus_cli::command::Facts::read(dir.path());
    /// let graph = CommandGraph::build(dir.path(), &share, &facts);
    /// let by_just = graph.resolve(Surface::Just, &["bench-coverage".to_string()]);
    /// let by_cli = graph.resolve(Surface::Cli, &["bench".to_string(), "coverage".to_string()]);
    /// assert_eq!(by_just.map(|c| c.id.as_str()), Some("native.bench.coverage"));
    /// assert_eq!(by_cli.map(|c| c.id.as_str()), by_just.map(|c| c.id.as_str()));
    /// ```
    pub fn resolve(&self, surface: Surface, words: &[String]) -> Option<&CommandNode> {
        match surface {
            Surface::Just => {
                let name = words.first()?;
                self.commands.iter().find(|c| {
                    c.projections.just.as_deref() == Some(name.as_str())
                        || c.projections.just_aliases.iter().any(|a| a == name)
                })
            }
            Surface::Cockpit => {
                let name = words.first()?;
                self.commands
                    .iter()
                    .find(|c| c.projections.cockpit.as_deref() == Some(name.as_str()))
            }
            Surface::Cli => self.longest_cli_prefix(words).map(|(c, _)| c),
        }
    }

    /// The deepest native command the words begin with, and how many words it took. The
    /// rest are the command's own arguments.
    pub fn longest_cli_prefix(&self, words: &[String]) -> Option<(&CommandNode, usize)> {
        let mut best: Option<(&CommandNode, usize)> = None;
        for c in self.commands.iter().filter(|c| c.program == Program::Native) {
            let n = c.path.len();
            if n <= words.len() && words[..n] == c.path[..] {
                if best.is_none_or(|(_, b)| n > b) {
                    best = Some((c, n));
                }
            }
        }
        best
    }

    /// Every command a person listing what they can run should see, in a deterministic
    /// order: the entry commands first, then the rest by identity.
    pub fn listed(&self) -> Vec<&CommandNode> {
        let mut out: Vec<&CommandNode> = self.commands.iter().filter(|c| c.is_listed()).collect();
        out.sort_by(|a, b| {
            entry_rank(a)
                .cmp(&entry_rank(b))
                .then_with(|| a.id.cmp(&b.id))
        });
        out
    }

    /// Is the graph safe to project? A fatal finding means an ambiguity a surface cannot
    /// resolve, and refusing is the only honest answer.
    pub fn is_valid(&self) -> bool {
        !self.diagnostics.iter().any(|d| d.fatal)
    }
}

/// Where a name was typed. The completion engine and every resolver take one of these
/// rather than guessing from the shape of the words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Surface {
    /// The native command line: `majordomus bench coverage`.
    Cli,
    /// The generated Just bridge: `just bench-coverage`.
    Just,
    /// The Cockpit's action name.
    Cockpit,
}

impl Surface {
    /// The surface a name says it is, for the command line's own `--surface`.
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "cli" => Some(Surface::Cli),
            "just" => Some(Surface::Just),
            "cockpit" => Some(Surface::Cockpit),
            _ => None,
        }
    }
}

/// The order the entry recommendation puts commands in. Deterministic and small: what the
/// declaration tagged as an entry, then diagnostics, then everything else. No ranking is
/// learned, and none depends on anything but the declaration.
fn entry_rank(c: &CommandNode) -> u8 {
    if c.tags.iter().any(|t| t == "entry") {
        0
    } else if c.tags.iter().any(|t| t == "diagnostic") {
        1
    } else if c.effect.is_read_only() {
        2
    } else {
        3
    }
}

/// Resolve every command's availability against the facts of the checkout. The commands
/// themselves declare requirements; nothing here knows what any particular command needs.
fn resolve_availability(commands: &mut [CommandNode], facts: &Facts) {
    for c in commands.iter_mut() {
        let unmet = facts.first_unmet(&c.availability.requires);
        c.availability = Availability {
            available: unmet.is_none(),
            requires: c.availability.requires.clone(),
            reason: unmet.map(|r| r.unmet_reason().to_string()),
        };
    }
}

/// The invariants a graph must satisfy before anything is projected from it.
fn validate(commands: &[CommandNode]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut ids: BTreeMap<&str, usize> = BTreeMap::new();
    for c in commands {
        *ids.entry(c.id.as_str()).or_default() += 1;
    }
    for (id, count) in ids.iter().filter(|(_, n)| **n > 1) {
        diagnostics.push(Diagnostic {
            code: "duplicate-identity".into(),
            fatal: true,
            message: format!("{count} commands claim the identity {id}"),
            commands: vec![CommandId::parse(id).unwrap_or_else(|_| {
                CommandId::new(Program::Workflow, &[(*id).to_string()])
            })],
        });
    }
    for c in commands {
        if let Err(reason) = CommandId::parse(c.id.as_str()) {
            diagnostics.push(Diagnostic {
                code: "invalid-identity".into(),
                fatal: true,
                message: format!("{}: {reason}", c.id),
                commands: vec![c.id.clone()],
            });
        }
        if c.runnable && c.summary.trim().is_empty() {
            diagnostics.push(Diagnostic {
                code: "no-summary".into(),
                fatal: false,
                message: format!("{} can be run and says nothing about itself", c.id),
                commands: vec![c.id.clone()],
            });
        }
        let mut flags: BTreeSet<&str> = BTreeSet::new();
        for a in &c.arguments {
            if let Some(long) = a.long.as_deref() {
                if !flags.insert(long) {
                    diagnostics.push(Diagnostic {
                        code: "duplicate-flag".into(),
                        fatal: true,
                        message: format!("{} declares --{long} twice", c.id),
                        commands: vec![c.id.clone()],
                    });
                }
            }
        }
        let positionals: Vec<bool> = c
            .arguments
            .iter()
            .filter(|a| a.positional)
            .map(|a| a.required)
            .collect();
        if positionals.windows(2).any(|w| !w[0] && w[1]) {
            diagnostics.push(Diagnostic {
                code: "argument-order".into(),
                fatal: false,
                message: format!(
                    "{} has a required positional after an optional one, which cannot be given",
                    c.id
                ),
                commands: vec![c.id.clone()],
            });
        }
        diagnostics.extend(cycle_check(c));
    }
    diagnostics
}

/// The programs a command may never invoke: the ones that would run the graph's own
/// projections and come back. A workflow that called `just` would reach a generated
/// recipe, which calls the executable, which generates the recipe — a bridge that is its
/// own consumer. Refused, with both ends named.
const FORBIDDEN_EXECUTORS: &[&str] = &["just"];

fn cycle_check(c: &CommandNode) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if let Execution::External { .. } = &c.execution {
        for program in c.execution.programs() {
            let name = Path::new(program)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| program.to_string());
            if FORBIDDEN_EXECUTORS.contains(&name.as_str()) {
                out.push(Diagnostic {
                    code: "execution-cycle".into(),
                    fatal: true,
                    message: format!(
                        "{} runs '{name}', which is a projection of this graph: a surface may not invoke another surface",
                        c.id
                    ),
                    commands: vec![c.id.clone()],
                });
            }
        }
    }
    out
}

/// What the surfaces project, hashed. Everything a projection renders is in it and
/// nothing else is: no timestamp, no availability, no path of this machine.
fn structure_fingerprint(commands: &[CommandNode]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(SCHEMA.as_bytes());
    h.update(b"\0");
    h.update(crate::VERSION.as_bytes());
    for c in commands {
        h.update(c.id.as_str().as_bytes());
        h.update(b"\0");
        h.update(c.summary.as_bytes());
        h.update(b"\0");
        h.update(c.usage.as_bytes());
        h.update(b"\0");
        h.update([c.runnable as u8, c.visibility as u8, c.effect as u8]);
        h.update(serde_json::to_vec(&c.arguments).unwrap_or_default());
        h.update(serde_json::to_vec(&c.execution).unwrap_or_default());
        h.update(serde_json::to_vec(&c.projections).unwrap_or_default());
        h.update(serde_json::to_vec(&c.deprecation).unwrap_or_default());
        h.update(b"\n");
    }
    format!("{:x}", h.finalize())
}

fn whole_fingerprint(structure: &str, commands: &[CommandNode]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(structure.as_bytes());
    for c in commands {
        h.update(serde_json::to_vec(&c.availability).unwrap_or_default());
    }
    format!("{:x}", h.finalize())
}

/// The effect of the whole set, for a summary: the strongest one present.
pub fn strongest_effect(commands: &[&CommandNode]) -> Option<EffectClass> {
    commands.iter().map(|c| c.effect).max()
}

/// Whether a command is one an unattended caller may be offered. Derived from the effect
/// and the interactivity, never configured per command and per surface.
pub fn offered_to_machines(c: &CommandNode) -> bool {
    c.visibility == Visibility::Public
        && c.effect.is_read_only()
        && c.interactivity != super::model::Interactivity::TtyRequired
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")
    }

    fn root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn graph() -> CommandGraph {
        let root = root();
        let facts = Facts::read(&root);
        CommandGraph::build(&root, &share(), &facts)
    }

    #[test]
    fn the_graph_composes_every_contributor_and_is_valid() {
        let g = graph();
        assert!(g.is_valid(), "{:#?}", g.diagnostics);
        assert!(g
            .commands
            .iter()
            .any(|c| c.program == Program::Native && c.id.as_str() == "native.capabilities.list"));
        assert!(g
            .commands
            .iter()
            .any(|c| c.program == Program::Shell && c.id.as_str() == "shell.doctor"));
        assert_eq!(g.schema, SCHEMA);
    }

    #[test]
    fn identities_are_unique_and_the_order_is_the_identity_order() {
        let g = graph();
        let ids: Vec<&str> = g.commands.iter().map(|c| c.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted, "the graph comes out in identity order");
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "identities are unique");
    }

    #[test]
    fn building_twice_over_the_same_tree_gives_the_same_fingerprint() {
        assert_eq!(graph().fingerprint, graph().fingerprint);
        assert_eq!(graph().structure, graph().structure);
    }

    #[test]
    fn availability_is_not_in_the_structure_fingerprint() {
        let root = root();
        let with = CommandGraph::build(&root, &share(), &Facts::read(&root));
        let without = CommandGraph::build(&root, &share(), &Facts::none(&root));
        assert_eq!(
            with.structure, without.structure,
            "a checkout that lost a dependency does not need a new bridge"
        );
        assert_ne!(
            with.fingerprint, without.fingerprint,
            "but a client caching on the whole graph sees the change"
        );
    }

    #[test]
    fn a_command_that_needs_something_missing_says_what_and_is_not_hidden() {
        let root = root();
        let g = CommandGraph::build(&root, &share(), &Facts::none(&root));
        let doctor = g.get("shell.doctor").expect("shell.doctor");
        assert!(!doctor.availability.available);
        assert_eq!(
            doctor.availability.reason.as_deref(),
            Some("no Majordomus layer was found here")
        );
        assert!(doctor.is_listed(), "unavailable is not invisible");
    }

    #[test]
    fn every_surface_name_resolves_back_to_the_same_command() {
        let g = graph();
        for c in g.commands.iter().filter(|c| c.is_listed()) {
            let just = c.projections.just.clone().expect("a recipe name");
            assert_eq!(
                g.resolve(Surface::Just, &[just.clone()]).map(|n| &n.id),
                Some(&c.id),
                "the recipe {just} resolves to {}",
                c.id
            );
            let action = c.projections.cockpit.clone().expect("an action name");
            assert_eq!(
                g.resolve(Surface::Cockpit, &[action]).map(|n| &n.id),
                Some(&c.id)
            );
            if c.program == Program::Native {
                assert_eq!(
                    g.resolve(Surface::Cli, &c.path).map(|n| &n.id),
                    Some(&c.id),
                    "the words of {} resolve to it",
                    c.id
                );
            }
        }
    }

    #[test]
    fn recipe_names_are_unique_across_every_program() {
        let g = graph();
        let mut names: Vec<&str> = g
            .commands
            .iter()
            .filter_map(|c| c.projections.just.as_deref())
            .chain(
                g.commands
                    .iter()
                    .flat_map(|c| c.projections.just_aliases.iter().map(String::as_str)),
            )
            .collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(before, names.len(), "no recipe name is claimed twice");
    }

    #[test]
    fn no_command_of_this_repository_runs_a_projection_of_this_graph() {
        let g = graph();
        assert!(
            !g.diagnostics.iter().any(|d| d.code == "execution-cycle"),
            "{:#?}",
            g.diagnostics
        );
        for c in &g.commands {
            for program in c.execution.programs() {
                let name = Path::new(program)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                assert!(!FORBIDDEN_EXECUTORS.contains(&name.as_str()), "{}", c.id);
            }
        }
    }

    #[test]
    fn machine_exposure_is_derived_from_the_effect_and_never_configured() {
        let g = graph();
        let generate = g.get("native.generate").expect("generate");
        assert!(
            !offered_to_machines(generate),
            "a command that rewrites committed artifacts is not offered unattended"
        );
        let list = g.get("native.capabilities.list").expect("capabilities list");
        assert!(offered_to_machines(list));
    }

    #[test]
    fn the_longest_native_prefix_wins_so_a_subcommand_is_not_read_as_its_parent() {
        let g = graph();
        let words = vec!["bench".to_string(), "coverage".to_string(), "--check".to_string()];
        let (node, used) = g.longest_cli_prefix(&words).expect("a command");
        assert_eq!(node.id.as_str(), "native.bench.coverage");
        assert_eq!(used, 2);
    }
}
