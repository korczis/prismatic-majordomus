//! The typed model of the command graph: what a command is, and what every projection of
//! it may say.
//!
//! Nothing here is a list of commands. The types describe the *shape* of a command node;
//! the nodes themselves come from the contributors in [`super::native`], [`super::tool`]
//! and [`super::workflow`], each of which reads a declaration that already exists — the
//! clap tree, the shipped command registry, the workflow runner's own dump — rather than
//! restating one.
//!
//! # Identity
//!
//! A [`CommandId`] is `<origin>.<path joined by dots>`: `executable.worktree.status`,
//! `tool.check`, `workflow.site-build`. The origin is in the identity because the three
//! programs are genuinely different programs — `majordomus check` is the shell tool and
//! `majordomus worktree status` is the Rust executable, and a graph that pretended they
//! shared a namespace would have to invent a rule for the day they collide. The id is
//! never a display string: it survives a renamed group, a moved recipe and a reworded
//! summary, which is what the projections key on.

use std::collections::BTreeMap;
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::{CapabilityId, CapabilityKind};

/// The schema every serialisation of the graph carries.
pub const SCHEMA: &str = "majordomus/command-graph/v1";

/// Which program runs a command. Part of the identity, because the three are different
/// programs that share one name on the path.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CommandOrigin")]
pub enum Origin {
    /// The Rust executable, `apps/majordomus-cli`: the read-only interfaces, generation,
    /// introspection, the servers. Declared by clap.
    Executable,
    /// The shell tool, `bin/majordomus`: the task lifecycle. Declared by the shipped
    /// command registry and dispatched by the tool itself.
    Tool,
    /// A workflow the repository declares for a person to run — a `just` recipe that is
    /// not a generated bridge. Declared by the justfile and read from the runner's dump.
    Workflow,
}

impl Origin {
    /// The prefix this origin contributes to every id under it.
    pub fn prefix(self) -> &'static str {
        match self {
            Origin::Executable => "executable",
            Origin::Tool => "tool",
            Origin::Workflow => "workflow",
        }
    }

    /// The program a person types, for the rendering of an invocation.
    pub fn program(self) -> &'static str {
        match self {
            Origin::Executable => "majordomus",
            Origin::Tool => "majordomus",
            Origin::Workflow => "just",
        }
    }
}

/// The canonical identity of one command.
///
/// Constructed from the origin and the command path, never written by hand, so that a
/// projection cannot invent one and a rename of a display string cannot change one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct CommandId(String);

impl CommandId {
    /// Derive the id of a command with this path under this origin.
    ///
    /// The path is the words a person types after the program's own name. Each word is
    /// lowered and every character outside `[a-z0-9-]` becomes `-`, so that a recipe named
    /// with a character the id grammar refuses still has one stable identity rather than
    /// none.
    ///
    /// ```
    /// use majordomus_cli::command_graph::{CommandId, Origin};
    /// let id = CommandId::derive(Origin::Executable, &["worktree".into(), "status".into()]);
    /// assert_eq!(id.as_str(), "executable.worktree.status");
    /// ```
    pub fn derive(origin: Origin, path: &[String]) -> Self {
        let mut out = String::from(origin.prefix());
        for word in path {
            out.push('.');
            out.extend(word.chars().map(|c| match c {
                'a'..='z' | '0'..='9' | '-' => c,
                'A'..='Z' => c.to_ascii_lowercase(),
                _ => '-',
            }));
        }
        CommandId(out)
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The origin this id belongs to, read back from its prefix.
    pub fn origin(&self) -> Option<Origin> {
        match self.0.split('.').next()? {
            "executable" => Some(Origin::Executable),
            "tool" => Some(Origin::Tool),
            "workflow" => Some(Origin::Workflow),
            _ => None,
        }
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What running a command changes. The one thing a surface policy is allowed to ask.
///
/// The order is the order of increasing consequence, and it is the order the derived
/// exposure policy reads: a surface declares the strongest effect it will carry, and
/// every node at or below it is projected there. Nothing configures a surface per
/// command.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CommandEffect")]
pub enum Effect {
    /// Reads and answers. Changes nothing anywhere.
    ReadOnly,
    /// Writes only where the repository keeps a checkout's own state — the process's
    /// memory, `.ai/local/`, a build directory. Nothing a commit would carry.
    LocalMutation,
    /// Writes tracked files: generated artifacts, the worktree, git itself.
    RepositoryMutation,
    /// Reaches the network with an effect on the far side: a push, a deploy, a release.
    NetworkMutation,
    /// Removes something a person would have to reconstruct.
    Destructive,
}

impl Effect {
    /// The effect of a capability, from the kind the registry already classified.
    ///
    /// The Rust executable writes nothing to the repository from a capability handler, so
    /// the strongest a capability reaches is this process's own memory.
    pub fn of_capability(kind: CapabilityKind) -> Self {
        match kind {
            CapabilityKind::Query | CapabilityKind::Resource => Effect::ReadOnly,
            CapabilityKind::Command => Effect::LocalMutation,
        }
    }

    /// Does this effect leave the repository as it found it?
    pub fn is_read_only(self) -> bool {
        matches!(self, Effect::ReadOnly)
    }
}

/// How a command behaves towards the caller's terminal and the caller's patience.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CommandInteractivity")]
pub enum Interactivity {
    /// Runs, answers, exits. Safe to call from a machine surface.
    NonInteractive,
    /// Asks the person something, or reads the body of a record from a terminal. A
    /// machine surface that offered it would hang.
    Interactive,
    /// Serves until it is stopped. A request/response surface cannot carry it.
    LongRunning,
}

/// Where a command means anything, and why not when it does not.
///
/// Derived from the repository environment rather than declared per command, so that a
/// projection reads a field instead of re-deciding. The reason is carried because the
/// answer a person needs is never `false`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CommandAvailability")]
pub struct Availability {
    /// Can it be run here, now?
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Why not, in one line, when it cannot.
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// What it needs, whether or not that is satisfied here.
    pub requires: Vec<Requirement>,
}

impl Availability {
    /// Available, with nothing standing in the way.
    pub fn always() -> Self {
        Availability {
            available: true,
            reason: None,
            requires: Vec::new(),
        }
    }
}

/// One thing a command needs before it can run.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CommandRequirement")]
pub enum Requirement {
    /// A git repository.
    Repository,
    /// The `.ai/` layer, initialised.
    Layer,
    /// An active task record.
    Task,
    /// The Rust executable, built.
    Executable,
    /// The workflow runner, installed.
    WorkflowRunner,
    /// A cargo workspace and a toolchain to build it.
    RustToolchain,
    /// The site sources and its generator.
    Site,
}

/// Where a value for an argument comes from, when something in this repository knows the
/// set.
///
/// This is the completion contract, and it is a property of the *argument*, not of a shell
/// script: the same source answers a shell's TAB, a generated form's select and a machine
/// surface's enumeration of what it will accept. Inference from the declaration comes
/// first — a value-enum argument carries its own values, a `PATH` placeholder is a path —
/// and only what cannot be inferred is annotated beside the command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[schemars(rename = "CommandValueSource")]
pub enum ValueSource {
    /// Nothing here knows the set; the caller types a value.
    Free,
    /// The declaration carries the values; they are on the argument.
    Enumerated,
    /// A path in the filesystem.
    Path,
    /// A path inside the repository.
    RepositoryPath,
    /// A capability id, from the registry.
    Capability,
    /// A command id, from this graph.
    Command,
    /// A rule id, from the effective rule set.
    Rule,
    /// An object kind, from the index.
    ObjectKind,
    /// A git branch in this repository.
    Branch,
    /// A graph id, from the graph registry.
    Graph,
    /// A moment id, from the why catalogue.
    Moment,
    /// A shell name, from the shells the activation supports.
    Shell,
    /// A secret. Never enumerated, never cached, never suggested.
    Secret,
}

impl ValueSource {
    /// May a completion offer values for this source at all?
    pub fn suggestible(self) -> bool {
        !matches!(self, ValueSource::Free | ValueSource::Secret)
    }
}

/// How openly a value may be handled.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CommandSecrecy")]
pub enum Secrecy {
    /// Ordinary: may be logged, completed, shown.
    Public,
    /// A path or an identifier that names something private. Shown, never logged.
    Sensitive,
    /// A credential. Never completed, never logged, never cached.
    Secret,
}

/// One argument of a command, as the declaration gives it plus what can be inferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CommandArgument")]
pub struct ArgumentSpec {
    /// The argument's id.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// `--long`, without the dashes.
    pub long: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// `-s`, without the dash.
    pub short: Option<char>,
    /// Given by position rather than by flag.
    pub positional: bool,
    /// Takes a value at all; a flag does not.
    pub takes_value: bool,
    /// Must be given.
    pub required: bool,
    /// Takes any number of values.
    pub variadic: bool,
    /// Accepted by every command under the one that declares it.
    pub global: bool,
    /// The help text, one line.
    pub help: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The placeholder, `PATH`.
    pub value_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The values the declaration carries, each with its help.
    pub values: Vec<ValueChoice>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The defaults, as the declaration renders them.
    pub defaults: Vec<String>,
    /// Where further values come from.
    pub source: ValueSource,
    /// How openly the value may be handled.
    pub secrecy: Secrecy,
}

/// One value an argument accepts, from the declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CommandValueChoice")]
pub struct ValueChoice {
    /// The value as typed.
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Its help, when the declaration carries one.
    pub description: Option<String>,
}

/// How a command is actually run: the program and the words before the caller's own.
///
/// This is what forbids a cycle. A projection renders an invocation from the execution
/// descriptor, so a generated bridge always spells the *canonical* program — never the
/// surface it is a bridge for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CommandExecution")]
pub struct Execution {
    /// Which program.
    pub origin: Origin,
    /// The words that precede the caller's arguments, the program's own name excluded.
    pub argv: Vec<String>,
}

impl Execution {
    /// The command line a person types, without arguments.
    pub fn invocation(&self) -> String {
        let mut out = String::from(self.origin.program());
        for word in &self.argv {
            out.push(' ');
            out.push_str(word);
        }
        out
    }
}

/// Where a node came from, in enough detail to open the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CommandProvenance")]
pub struct Provenance {
    /// The repository-relative file that declares it.
    pub declared_in: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The command that reads that declaration, when a reader wants to reproduce it.
    pub read_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The capability this command runs, when it runs one.
    pub capability: Option<CapabilityId>,
}

/// A command that is no longer the name to use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CommandDeprecation")]
pub struct Deprecation {
    /// Why, in one line.
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// What to use instead.
    pub replaced_by: Option<CommandId>,
}

/// Where one command appears, derived from its effect, its interactivity and its origin.
///
/// Every field is computed by [`super::policy`]. Nothing declares a projection, and no
/// surface keeps a list of what it carries: a surface asks the graph.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CommandProjections")]
pub struct Projections {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The command line, as typed.
    pub cli: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The generated workflow bridge's recipe name.
    pub workflow: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The MCP tool name, when the capability behind it declares one.
    pub mcp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The HTTP route, when the capability behind it declares one.
    pub http: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The Cockpit address, when the surface carries it.
    pub cockpit: Option<String>,
    /// The page on the site. Every command has one.
    pub docs: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Why a machine surface does not carry it, when one does not.
    pub withheld: Option<String>,
}

/// One command, from whichever program offers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandNode {
    /// The canonical identity.
    pub id: CommandId,
    /// Which program runs it.
    pub origin: Origin,
    /// The words after the program's own name.
    pub path: Vec<String>,
    /// The command line a person types, rendered once here.
    pub invocation: String,
    /// One line.
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The longer description, when the declaration carries one.
    pub description: Option<String>,
    /// Can it be run on its own, or does it only group the commands under it?
    pub runnable: bool,
    /// The arguments, in declaration order.
    pub arguments: Vec<ArgumentSpec>,
    /// How it is run.
    pub execution: Execution,
    /// What it changes.
    pub effect: Effect,
    /// How it behaves towards a terminal.
    pub interactivity: Interactivity,
    /// Where it stands.
    pub stability: crate::capability::Stability,
    /// Where it means anything.
    pub availability: Availability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The group a person finds it under, when the declaration has groups.
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Free tags.
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// Other names that resolve to this node, declared once here and honoured by every
    /// projection that has a use for one.
    pub aliases: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Whether this is still the name to use.
    pub deprecation: Option<Deprecation>,
    /// Where it came from.
    pub provenance: Provenance,
    /// Where it appears.
    pub projections: Projections,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Is this command an entry point of its group — the one a newcomer is offered?
    pub entrypoint: Option<bool>,
}

impl CommandNode {
    /// Everything a search matches on, lowered, joined.
    pub fn haystack(&self) -> String {
        let mut s = self.invocation.to_lowercase();
        s.push(' ');
        s.push_str(&self.summary.to_lowercase());
        for tag in &self.tags {
            s.push(' ');
            s.push_str(&tag.to_lowercase());
        }
        s.push(' ');
        s.push_str(self.id.as_str());
        s
    }
}

/// How bad a finding is.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CommandDiagnosticSeverity")]
pub enum Severity {
    /// The graph is wrong and a projection built from it would be wrong.
    Error,
    /// Worth saying; the graph stands.
    Warning,
    /// A fact a reader may want.
    Info,
}

/// One thing the build found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "CommandDiagnostic")]
pub struct Diagnostic {
    /// How bad.
    pub severity: Severity,
    /// A stable code, for a gate to match on.
    pub code: String,
    /// What is wrong, in one line.
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    /// The nodes it is about.
    pub commands: Vec<CommandId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// What to do about it.
    pub remedy: Option<String>,
}

impl Diagnostic {
    /// An error about one or more commands.
    pub fn error(code: &str, message: impl Into<String>, commands: Vec<CommandId>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            code: code.into(),
            message: message.into(),
            commands,
            remedy: None,
        }
    }

    /// A warning about one or more commands.
    pub fn warning(code: &str, message: impl Into<String>, commands: Vec<CommandId>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            code: code.into(),
            message: message.into(),
            commands,
            remedy: None,
        }
    }

    /// The same, with a remedy.
    pub fn with_remedy(mut self, remedy: impl Into<String>) -> Self {
        self.remedy = Some(remedy.into());
        self
    }
}

/// The whole graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandGraph {
    /// The schema of this document.
    pub schema: String,
    /// A hash of the semantic content: the identity a cache and a generated projection
    /// key on. Nothing that varies between two runs over the same tree is in it.
    pub fingerprint: String,
    /// Every command, in a deterministic order: origin, then path.
    pub commands: Vec<CommandNode>,
    /// What the build found.
    pub diagnostics: Vec<Diagnostic>,
}

impl CommandGraph {
    /// Look one up.
    pub fn get(&self, id: &CommandId) -> Option<&CommandNode> {
        self.commands.iter().find(|c| &c.id == id)
    }

    /// Resolve a surface's spelling of a command back to the canonical node.
    ///
    /// This is the one place a surface alias is interpreted. A completion for `just`
    /// resolves the recipe name here and then asks the same engine the command line asks,
    /// which is why there is no second completion implementation.
    pub fn resolve(&self, surface: Surface, spelling: &str) -> Option<&CommandNode> {
        self.commands.iter().find(|c| match surface {
            Surface::Cli => c.projections.cli.as_deref() == Some(spelling),
            Surface::Workflow => {
                c.projections.workflow.as_deref() == Some(spelling)
                    || c.aliases.iter().any(|a| a == spelling)
            }
            Surface::Mcp => c.projections.mcp.as_deref() == Some(spelling),
            Surface::Http => c.projections.http.as_deref() == Some(spelling),
        })
    }

    /// The errors, if any. A projection refuses to write when this is not empty.
    pub fn errors(&self) -> Vec<&Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .collect()
    }

    /// The commands of one origin.
    pub fn of_origin(&self, origin: Origin) -> impl Iterator<Item = &CommandNode> {
        self.commands.iter().filter(move |c| c.origin == origin)
    }

    /// The runnable commands, grouped by their group, in graph order.
    pub fn by_group(&self) -> BTreeMap<String, Vec<&CommandNode>> {
        let mut out: BTreeMap<String, Vec<&CommandNode>> = BTreeMap::new();
        for node in self.commands.iter().filter(|c| c.runnable) {
            let key = node.group.clone().unwrap_or_else(|| "other".into());
            out.entry(key).or_default().push(node);
        }
        out
    }
}

/// A surface that spells commands its own way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "CommandSurfaceName")]
pub enum Surface {
    /// The command line of either program.
    Cli,
    /// The workflow runner.
    Workflow,
    /// MCP.
    Mcp,
    /// HTTP.
    Http,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_derived_and_stable() {
        let a = CommandId::derive(Origin::Executable, &["worktree".into(), "status".into()]);
        assert_eq!(a.as_str(), "executable.worktree.status");
        assert_eq!(a.origin(), Some(Origin::Executable));
        let b = CommandId::derive(Origin::Workflow, &["site-build".into()]);
        assert_eq!(b.as_str(), "workflow.site-build");
        // A spelling the grammar refuses still yields one identity rather than none.
        let c = CommandId::derive(Origin::Workflow, &["Site Build".into()]);
        assert_eq!(c.as_str(), "workflow.site-build");
    }

    #[test]
    fn effects_order_by_consequence() {
        assert!(Effect::ReadOnly < Effect::LocalMutation);
        assert!(Effect::RepositoryMutation < Effect::NetworkMutation);
        assert!(Effect::NetworkMutation < Effect::Destructive);
        assert!(Effect::of_capability(CapabilityKind::Query).is_read_only());
        assert!(!Effect::of_capability(CapabilityKind::Command).is_read_only());
    }

    #[test]
    fn a_secret_is_never_suggestible() {
        assert!(!ValueSource::Secret.suggestible());
        assert!(!ValueSource::Free.suggestible());
        assert!(ValueSource::Branch.suggestible());
    }

    #[test]
    fn an_invocation_is_rendered_from_the_execution() {
        let e = Execution {
            origin: Origin::Executable,
            argv: vec!["worktree".into(), "status".into()],
        };
        assert_eq!(e.invocation(), "majordomus worktree status");
    }
}
