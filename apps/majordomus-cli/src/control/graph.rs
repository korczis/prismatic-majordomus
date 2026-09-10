//! One graph of everything this repository can be asked to do.
//!
//! Nothing here declares a command. The structure is walked out of the clap declaration
//! that already is the command line ([`crate::cli::tree`]); the identity, the exposures
//! and the provenance of anything backed by a capability come from the registry that
//! already owns them; the semantics that neither can carry — what running it changes — are
//! declared beside the command's own examples in `cli.rs`; and workflows the repository
//! keeps outside the executable are discovered rather than listed. The graph is the
//! composition, and it is the only thing the projections read.
//!
//! It is deliberately cheap. Building it reads no repository index, spawns nothing and
//! touches no file, so the paths that must be fast — completion, the `just` bridge, the
//! banner — pay for a clap walk and nothing else.
//!
//! ```
//! use majordomus_cli::control::graph;
//! let g = graph::of_this_executable();
//! assert!(g.find("capabilities.list").is_some());
//! // every runnable command says what running it does
//! assert!(g.commands.iter().filter(|c| !c.group).all(|c| c.semantics.is_some()));
//! // and the fingerprint is a function of the graph, not of the clock
//! assert_eq!(g.fingerprint, graph::of_this_executable().fingerprint);
//! ```

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::{CapabilityRegistry, Stability};
use crate::cli::{self, ArgDoc, CommandDoc};
use crate::control::effect::Semantics;
use crate::control::projection::{self, Projections, Surface};

/// The schema of the machine-readable graph. A published contract: consumers pin it.
pub const SCHEMA: &str = "majordomus/commands/v1";

/// How a command runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[schemars(rename = "CommandExecution")]
pub enum Execution {
    /// The executable itself runs it, through the clap declaration.
    Native {
        /// The capability behind it, when one is declared for this path.
        #[serde(skip_serializing_if = "Option::is_none")]
        capability: Option<String>,
    },
    /// A workflow the repository keeps outside the executable, discovered where it lives.
    External {
        /// What runs it, as a person would type it.
        runner: String,
        /// Where it was discovered.
        source: String,
    },
}

/// Whether a command can be offered, and why not when it cannot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "snake_case")]
#[schemars(rename = "CommandAvailability")]
pub enum Availability {
    /// Offer it.
    Available,
    /// Do not offer it as usable; the reason is shown wherever it is listed.
    Unavailable {
        /// One line, in the reader's terms.
        reason: String,
    },
}

impl Availability {
    /// Is it offered?
    pub fn is_available(&self) -> bool {
        matches!(self, Availability::Available)
    }
}

/// Where an argument's values come from. Inferred from the argument's own declaration:
/// a value-enum carries its values, a value name from the typed vocabulary names a
/// registry, and everything else has no source rather than a guessed one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
#[schemars(rename = "CommandValueSource")]
pub enum ValueSource {
    /// Nothing can be offered.
    None,
    /// The values the declaration accepts.
    Enum {
        /// Each value with the help clap declared for it.
        values: Vec<EnumValue>,
    },
    /// A path on this machine.
    Path,
    /// Identities a registry of this repository holds.
    Registry {
        /// Which registry.
        registry: RegistryValues,
    },
}

/// One accepted value of a value-enum argument.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CommandEnumValue")]
pub struct EnumValue {
    /// The value as typed.
    pub value: String,
    /// Its help, when the declaration gave one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

/// A registry that can answer with identities. Each one is something this repository
/// already holds; none of them is a list kept for completion.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CommandValueRegistry")]
pub enum RegistryValues {
    /// Capability identities.
    Capability,
    /// The kinds the layer declares.
    ObjectKind,
    /// The operational moments of the Why catalogue.
    Moment,
    /// Branches of this repository.
    Branch,
    /// Canonical commands of this graph.
    Command,
}

impl RegistryValues {
    /// The value name in the clap declaration that names this registry. The vocabulary is
    /// small on purpose: an argument opts into dynamic values by being declared with one
    /// of these names, which is a decision made once, where the argument is declared.
    pub fn of_value_name(name: &str) -> Option<Self> {
        match name {
            "CAPABILITY" => Some(RegistryValues::Capability),
            "KIND" => Some(RegistryValues::ObjectKind),
            "MOMENT" => Some(RegistryValues::Moment),
            "BRANCH" => Some(RegistryValues::Branch),
            "COMMAND" => Some(RegistryValues::Command),
            _ => None,
        }
    }

    /// Does answering this need the repository's index? The engine uses it to keep the
    /// cheap answers cheap.
    pub fn needs_index(self) -> bool {
        matches!(self, RegistryValues::ObjectKind | RegistryValues::Moment)
    }
}

/// One argument of one command, as the declaration has it, with where its values come
/// from resolved once here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CommandArgument")]
pub struct Argument {
    /// The argument's id.
    pub name: String,
    /// `--transport`, without the dashes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long: Option<String>,
    /// `-t`, without the dash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<char>,
    /// Given by position rather than by flag.
    pub positional: bool,
    /// The help text.
    pub help: String,
    /// The placeholder for the value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_name: Option<String>,
    /// Whether it takes a value at all.
    pub takes_value: bool,
    /// Must be given.
    pub required: bool,
    /// The default value(s), as clap renders them.
    pub defaults: Vec<String>,
    /// Where its values come from.
    pub values: ValueSource,
}

impl Argument {
    fn of(arg: &ArgDoc) -> Self {
        let values = if !arg.possible_values.is_empty() {
            ValueSource::Enum {
                values: arg
                    .possible_values
                    .iter()
                    .map(|v| EnumValue {
                        value: v.name.clone(),
                        help: v.help.clone(),
                    })
                    .collect(),
            }
        } else if let Some(registry) = arg
            .value_name
            .as_deref()
            .and_then(RegistryValues::of_value_name)
        {
            ValueSource::Registry { registry }
        } else if matches!(arg.value_name.as_deref(), Some("PATH" | "DIR" | "FILE")) {
            ValueSource::Path
        } else {
            ValueSource::None
        };
        Argument {
            name: arg.name.clone(),
            long: arg.long.clone(),
            short: arg.short,
            positional: arg.positional,
            help: arg.help.clone(),
            value_name: arg.value_name.clone(),
            takes_value: arg.takes_value,
            required: arg.required,
            defaults: arg.defaults.clone(),
            values,
        }
    }
}

/// One example, carried through so that every surface shows the same one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CommandExample")]
pub struct Example {
    /// The example's id.
    pub id: String,
    /// One line: what it shows.
    pub title: String,
    /// The command line as a person types it.
    pub command: String,
}

/// One canonical command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandNode {
    /// The stable identity: the path, joined by dots. Survives a change of summary, of
    /// help, of recipe spelling and of tool name, because none of those is in it.
    pub id: String,
    /// The words after `majordomus`.
    pub path: Vec<String>,
    /// The one-line description; the one text every surface shows.
    pub summary: String,
    /// The long description, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The usage line.
    pub usage: String,
    /// Whether it only groups other commands and cannot be run.
    pub group: bool,
    /// What running it does; absent for a group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantics: Option<Semantics>,
    /// How it runs.
    pub execution: Execution,
    /// Its arguments.
    pub arguments: Vec<Argument>,
    /// Its documented examples.
    pub examples: Vec<Example>,
    /// Every spelling of it.
    pub projections: Projections,
    /// Whether it is offered.
    pub availability: Availability,
    /// Names it has answered to before; kept working by every surface that can.
    pub aliases: Vec<String>,
    /// Where it is declared, repository-relative.
    pub provenance: String,
}

impl CommandNode {
    /// May a machine surface invoke it? A group never; anything else by its semantics.
    pub fn machine_callable(&self) -> bool {
        !self.group && self.semantics.is_some_and(Semantics::machine_callable)
    }
}

/// Something wrong with the graph, or with what fed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CommandDiagnostic")]
pub struct Diagnostic {
    /// The stable code a reader greps for.
    pub code: String,
    /// Whether the graph is unusable, or merely poorer.
    pub fatal: bool,
    /// What it is about.
    pub subject: String,
    /// One line.
    pub detail: String,
}

/// The whole graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandGraph {
    /// The schema of this document.
    pub schema: String,
    /// The version of the executable that produced it.
    pub version: String,
    /// A hash of the graph's own content: identities, paths, arguments, semantics and
    /// projections. No clock, no path of this machine, no ordering accident.
    pub fingerprint: String,
    /// Every command, in path order.
    pub commands: Vec<CommandNode>,
    /// What the composition found wrong.
    pub diagnostics: Vec<Diagnostic>,
}

impl CommandGraph {
    /// One command by its identity.
    pub fn find(&self, id: &str) -> Option<&CommandNode> {
        self.commands.iter().find(|c| c.id == id)
    }

    /// The command a surface's spelling resolves to: the alias resolution every surface
    /// shares instead of keeping a map of its own.
    ///
    /// ```
    /// use majordomus_cli::control::{graph, projection::Surface};
    /// let g = graph::of_this_executable();
    /// let by_just = g.resolve(Surface::Just, "capabilities-list").map(|c| c.id.as_str());
    /// let by_cli = g.resolve(Surface::Cli, "capabilities list").map(|c| c.id.as_str());
    /// assert_eq!(by_just, Some("capabilities.list"));
    /// assert_eq!(by_just, by_cli);
    /// ```
    pub fn resolve(&self, surface: Surface, spelling: &str) -> Option<&CommandNode> {
        self.commands.iter().find(|c| match surface {
            Surface::Cli => c.path.join(" ") == spelling,
            Surface::Just => c.projections.just.as_deref() == Some(spelling),
            Surface::Mcp => c.projections.mcp.as_deref() == Some(spelling),
            Surface::Http => c.projections.http.as_deref() == Some(spelling),
            Surface::Cockpit => c.projections.cockpit.as_deref() == Some(spelling),
            Surface::Docs => c.projections.docs == spelling,
        })
    }

    /// Is anything fatally wrong?
    pub fn is_valid(&self) -> bool {
        !self.diagnostics.iter().any(|d| d.fatal)
    }
}

/// The graph of this executable, with no repository and no external workflow: the clap
/// declaration, the builtin registry and the semantics declared beside them.
///
/// This is the cheap constructor, and the one every fast path uses.
pub fn of_this_executable() -> CommandGraph {
    let registry = CapabilityRegistry::builder()
        .with_modules(crate::capability::builtin::modules())
        .build()
        .unwrap_or_default();
    build(&cli::tree(), &registry, &[])
}

/// What a command declares beside itself: the semantics of running it, and the names it
/// has answered to before. Everything else about a command is read off the declaration or
/// computed.
///
/// A parameter rather than a constant so that the composition can be exercised over a
/// command line this crate does not ship — which is how the invariant that a new command
/// needs no edit to any projection is proved rather than asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declaration<'a> {
    /// The command's path as a person types it, without `majordomus`.
    pub command: &'a str,
    /// What running it changes.
    pub semantics: Semantics,
    /// Names it has answered to before.
    pub aliases: &'a [&'a str],
}

/// The declarations of this executable's own command line.
pub fn declarations() -> Vec<Declaration<'static>> {
    cli::EXAMPLES
        .iter()
        .map(|set| Declaration {
            command: set.command,
            semantics: set.semantics,
            aliases: set.aliases,
        })
        .collect()
}

/// The graph over a command-line tree, a registry and the external workflows discovered
/// for this repository, using this executable's own declarations.
pub fn build(
    tree: &CommandDoc,
    registry: &CapabilityRegistry,
    workflows: &[crate::control::workflow::Workflow],
) -> CommandGraph {
    compose(tree, registry, workflows, &declarations())
}

/// The composition itself, over any command line and any set of declarations.
pub fn compose(
    tree: &CommandDoc,
    registry: &CapabilityRegistry,
    workflows: &[crate::control::workflow::Workflow],
    declarations: &[Declaration<'_>],
) -> CommandGraph {
    let mut commands = Vec::new();
    let mut diagnostics = Vec::new();

    // what a capability declares for a command-line path, by that path
    let by_cli: BTreeMap<Vec<String>, &crate::capability::Capability> = registry
        .iter()
        .filter_map(|c| c.exposure.cli.as_ref().map(|e| (e.path.clone(), c)))
        .collect();

    let semantics: BTreeMap<&str, Semantics> = declarations
        .iter()
        .map(|d| (d.command, d.semantics))
        .collect();
    let aliases: BTreeMap<&str, &[&str]> = declarations
        .iter()
        .map(|d| (d.command, d.aliases))
        .collect();

    for doc in tree.flatten() {
        // the root is the executable itself, not a command
        let path: Vec<String> = doc.path.iter().skip(1).cloned().collect();
        if path.is_empty() {
            continue;
        }
        let key = path.join(" ");
        let declared = semantics.get(key.as_str()).copied();
        if doc.executable && declared.is_none() {
            diagnostics.push(Diagnostic {
                code: "COMMAND_WITHOUT_SEMANTICS".into(),
                fatal: true,
                subject: format!("majordomus {key}"),
                detail: "a command that can be run does not say what running it changes; \
                         declare its semantics beside its examples in cli.rs"
                    .into(),
            });
        }
        let capability = by_cli.get(&path);
        // A command that groups others and can also be run — `why` answering as
        // `why list` — is a convenience of the command line, not a second operation. A
        // machine addresses the subcommand, so the parent projects no machine name of its
        // own unless a capability declares one for exactly that path.
        let declared_tool = capability
            .and_then(|c| c.exposure.mcp.as_ref())
            .and_then(|m| m.tool.clone());
        let addressable = declared.is_some_and(Semantics::machine_callable)
            && (doc.subcommands.is_empty() || declared_tool.is_some());
        let machine = addressable;
        let node = CommandNode {
            id: path.join("."),
            summary: doc.about.clone(),
            description: doc.long_about.clone(),
            usage: doc.usage.clone(),
            group: !doc.executable,
            semantics: declared,
            execution: Execution::Native {
                capability: capability.map(|c| c.id.as_str().to_string()),
            },
            arguments: doc.args.iter().map(Argument::of).collect(),
            examples: doc
                .examples
                .iter()
                .map(|e| Example {
                    id: e.id.clone(),
                    title: e.title.clone(),
                    command: e.command.clone(),
                })
                .collect(),
            projections: Projections {
                cli: path.clone(),
                just: doc.executable.then(|| projection::just_recipe(&path)),
                mcp: machine.then(|| declared_tool.unwrap_or_else(|| projection::mcp_tool(&path))),
                http: capability
                    .and_then(|c| c.exposure.http.as_ref())
                    .map(|h| format!("{} {}", h.method.as_str(), h.path)),
                cockpit: machine.then(|| projection::cockpit_action(&path.join("."))),
                docs: projection::docs_route(&path),
            },
            availability: match capability.map(|c| c.stability) {
                Some(s) if !s.executable() => Availability::Unavailable {
                    reason: format!("the capability behind it is {}", stability_word(s)),
                },
                _ => Availability::Available,
            },
            provenance: cli::DECLARATION.to_string(),
            aliases: aliases
                .get(key.as_str())
                .map(|names| names.iter().map(|n| n.to_string()).collect())
                .unwrap_or_default(),
            path,
        };
        commands.push(node);
    }

    for workflow in workflows {
        if commands
            .iter()
            .any(|c| c.projections.just.as_deref() == Some(workflow.name.as_str()))
        {
            // the bridge's own recipe for a canonical command; not a second command
            continue;
        }
        commands.push(workflow.node());
    }

    commands.sort_by(|a, b| a.path.cmp(&b.path));
    diagnostics.extend(collisions(&commands));

    let fingerprint = fingerprint(&commands);
    CommandGraph {
        schema: SCHEMA.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        fingerprint,
        commands,
        diagnostics,
    }
}

fn stability_word(s: Stability) -> &'static str {
    match s {
        Stability::Planned => "planned",
        Stability::Unsupported => "unsupported",
        Stability::Implemented => "implemented",
        Stability::BehaviorallyVerified => "behaviourally verified",
        Stability::Experimental => "experimental",
    }
}

/// How one surface spells a command; `None` when the command is not on it.
type Spelling = fn(&CommandNode) -> Option<String>;

/// Two commands that would answer to one spelling on one surface. Never resolved by
/// order: both ends are named and the graph is refused.
fn collisions(commands: &[CommandNode]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let surfaces: [(Surface, Spelling); 4] = [
        (Surface::Just, |c| c.projections.just.clone()),
        (Surface::Mcp, |c| c.projections.mcp.clone()),
        (Surface::Cockpit, |c| c.projections.cockpit.clone()),
        (Surface::Docs, |c| Some(c.projections.docs.clone())),
    ];
    for (surface, take) in surfaces {
        let mut seen: BTreeMap<String, &str> = BTreeMap::new();
        for c in commands {
            let Some(name) = take(c) else { continue };
            if let Some(first) = seen.insert(name.clone(), c.id.as_str()) {
                out.push(Diagnostic {
                    code: "PROJECTION_COLLISION".into(),
                    fatal: true,
                    subject: format!("{}:{name}", surface.label()),
                    detail: format!(
                        "'{first}' and '{}' both project to '{name}' on the {} surface",
                        c.id,
                        surface.label()
                    ),
                });
            }
        }
    }
    out
}

/// A hash of the graph's semantic content, in a fixed order, with nothing of the moment in
/// it: two runs over one declaration agree, and a cache keyed on it is safe.
fn fingerprint(commands: &[CommandNode]) -> String {
    let mut material = String::new();
    for c in commands {
        material.push_str(&c.id);
        material.push('\u{1f}');
        material.push_str(&c.summary);
        material.push('\u{1f}');
        material.push_str(match &c.semantics {
            Some(s) => s.effect.label(),
            None => "group",
        });
        material.push('\u{1f}');
        material.push_str(c.projections.just.as_deref().unwrap_or(""));
        material.push('\u{1f}');
        for a in &c.arguments {
            material.push_str(&a.name);
            material.push('=');
            material.push_str(&serde_json::to_string(&a.values).unwrap_or_default());
            material.push(',');
        }
        material.push('\u{1e}');
    }
    crate::policy::sha256_hex(&material)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_graph_of_this_executable_is_valid_and_complete() {
        let g = of_this_executable();
        assert!(
            g.is_valid(),
            "the graph carries fatal diagnostics: {:?}",
            g.diagnostics
        );
        assert!(g.commands.len() > 20, "{} commands", g.commands.len());
    }

    #[test]
    fn every_runnable_command_declares_what_running_it_does() {
        let g = of_this_executable();
        let missing: Vec<&str> = g
            .commands
            .iter()
            .filter(|c| !c.group && c.semantics.is_none())
            .map(|c| c.id.as_str())
            .collect();
        assert!(missing.is_empty(), "undeclared: {missing:?}");
    }

    #[test]
    fn nothing_that_changes_the_repository_is_offered_to_a_machine() {
        let g = of_this_executable();
        for c in &g.commands {
            if let Some(s) = c.semantics {
                if !s.machine_callable() {
                    assert!(
                        c.projections.mcp.is_none() && c.projections.cockpit.is_none(),
                        "{} is {} and is offered to a machine surface",
                        c.id,
                        s.effect.label()
                    );
                }
            }
        }
    }

    #[test]
    fn the_fingerprint_does_not_move_between_runs() {
        assert_eq!(
            of_this_executable().fingerprint,
            of_this_executable().fingerprint
        );
    }
}
