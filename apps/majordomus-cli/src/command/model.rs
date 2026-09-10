//! The canonical command descriptor: identity, path, arguments, execution, effect,
//! availability, provenance and the projections derived from them. Plain data,
//! serialisable, with no handler, no shell string and no transport type in it.
//!
//! A [`CommandNode`] is to the developer surfaces what
//! [`crate::capability::Capability`] is to the machine surfaces: the one place a command
//! is described. The command line, the generated Just bridge, shell completion, the
//! Cockpit's palette and the generated reference are renderings of these values and
//! declare nothing of their own.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The schema of the serialised graph, `majordomus/command-graph/v1`. A public machine
/// interface: the Cockpit, the completion index and any external tool read it.
pub const SCHEMA: &str = "majordomus/command-graph/v1";

/// Which executable owns a command. Two programs share the name `majordomus` in this
/// repository — the Rust executable and the shell tool — and a workflow is a third owner
/// that runs something else entirely. The program is part of the canonical identity
/// because two programs may legitimately both have a `bench`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum Program {
    /// The Rust executable, `apps/majordomus-cli`: the read-only interfaces, generation
    /// and introspection. Its commands are declared by clap in `src/cli.rs`.
    Native,
    /// The shell tool, `bin/majordomus`: the task lifecycle. Its commands are declared by
    /// `share/commands.yaml`, which the index reads as objects of the `command` kind.
    Shell,
    /// A workflow of this repository: something neither executable implements, declared
    /// as one object under the workflow-command tree and executed by the program it names.
    Workflow,
}

impl Program {
    /// The identity segment, and the prefix a losing projection takes.
    ///
    /// ```
    /// use majordomus_cli::command::Program;
    /// assert_eq!(Program::Native.as_str(), "native");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Program::Native => "native",
            Program::Shell => "shell",
            Program::Workflow => "workflow",
        }
    }

    /// The prefix a projection uses when this program loses a name collision. The shell
    /// tool's is `mj`, the prefix its own library already uses for everything it owns
    /// (`MJ_`, `mj_`), so the projection reads as the tool a person already knows.
    pub fn projection_prefix(self) -> &'static str {
        match self {
            Program::Native => "rust",
            Program::Shell => "mj",
            Program::Workflow => "wf",
        }
    }

    /// Which program keeps the bare name when two want it. Lower wins. Fixed and
    /// documented so that a projection name never depends on discovery order.
    ///
    /// ```
    /// use majordomus_cli::command::Program;
    /// assert!(Program::Native.precedence() < Program::Shell.precedence());
    /// assert!(Program::Shell.precedence() < Program::Workflow.precedence());
    /// ```
    pub fn precedence(self) -> u8 {
        match self {
            Program::Native => 0,
            Program::Shell => 1,
            Program::Workflow => 2,
        }
    }

    /// Every program, in precedence order.
    pub const ALL: &'static [Program] = &[Program::Native, Program::Shell, Program::Workflow];
}

impl fmt::Display for Program {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A stable, machine-meaningful command identity: the program, a dot, and the command's
/// path words joined by dots. `native.bench.coverage`, `shell.doctor`,
/// `workflow.site-build`.
///
/// The identity survives a title change, a help rewrite and a projection rename, because
/// none of those is in it. Every surface name — the Just recipe, the Cockpit action, the
/// documentation route — is derived from this and never the other way round.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct CommandId(String);

impl CommandId {
    /// The identity of a command of a program, from its path words.
    ///
    /// ```
    /// use majordomus_cli::command::{CommandId, Program};
    /// let id = CommandId::new(Program::Native, &["bench".into(), "coverage".into()]);
    /// assert_eq!(id.as_str(), "native.bench.coverage");
    /// ```
    pub fn new(program: Program, path: &[String]) -> Self {
        let mut text = String::from(program.as_str());
        for word in path {
            text.push('.');
            text.push_str(word);
        }
        CommandId(text)
    }

    /// Parse and validate: a known program, a dot, and at least one word, each word
    /// matching `[a-z0-9][a-z0-9-]*`.
    ///
    /// ```
    /// use majordomus_cli::command::CommandId;
    /// assert!(CommandId::parse("native.bench.coverage").is_ok());
    /// assert!(CommandId::parse("shell.doctor").is_ok());
    /// assert!(CommandId::parse("native").is_err(), "a path is required");
    /// assert!(CommandId::parse("nope.thing").is_err(), "the program must be known");
    /// assert!(CommandId::parse("native.Bench").is_err(), "words are lowercase");
    /// ```
    pub fn parse(text: &str) -> Result<Self, String> {
        let Some((program, rest)) = text.split_once('.') else {
            return Err("needs a program, a dot, and at least one path word".into());
        };
        if !Program::ALL.iter().any(|p| p.as_str() == program) {
            return Err(format!("program '{program}' is not one this executable knows"));
        }
        if rest.is_empty() {
            return Err("the path after the program is empty".into());
        }
        for word in rest.split('.') {
            let ok = !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                && !word.starts_with('-');
            if !ok {
                return Err(format!("path word '{word}' is not [a-z0-9][a-z0-9-]*"));
            }
        }
        Ok(CommandId(text.to_string()))
    }

    /// The identity as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The program that owns it.
    pub fn program(&self) -> Program {
        let head = self.0.split('.').next().unwrap_or_default();
        Program::ALL
            .iter()
            .copied()
            .find(|p| p.as_str() == head)
            .unwrap_or(Program::Workflow)
    }

    /// The path words after the program.
    pub fn path(&self) -> Vec<String> {
        self.0.split('.').skip(1).map(str::to_string).collect()
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What a command does to the world. The one semantic fact no spelling can be trusted to
/// carry, and the fact every surface policy is derived from: what may be offered to a
/// machine caller, what a Cockpit must confirm, what documentation must warn about.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum EffectClass {
    /// Writes nothing anywhere: a report, a listing, a validation.
    ReadOnly,
    /// Changes the memory of the process it runs in and nothing on disk.
    ProcessState,
    /// Writes durable records under the checkout-local half of the layer
    /// (`.ai/local/`), which is never normative and never committed.
    LocalState,
    /// Rewrites generated artifacts that are committed for review: the projections, the
    /// provider bootstraps, the site's data.
    GeneratedOutput,
    /// Reaches the network: a push, a deployment, a fetch.
    Network,
    /// Removes or overwrites something a re-run cannot restore.
    Destructive,
}

impl EffectClass {
    /// The class a `share/commands.yaml` entry's `class` names. The shell registry's
    /// vocabulary is a subset of this one, mapped once here rather than at each reader.
    ///
    /// ```
    /// use majordomus_cli::command::EffectClass;
    /// assert_eq!(EffectClass::of_shell_class("read-only"), Some(EffectClass::ReadOnly));
    /// assert_eq!(EffectClass::of_shell_class("nonsense"), None);
    /// ```
    pub fn of_shell_class(text: &str) -> Option<Self> {
        match text {
            "read-only" => Some(EffectClass::ReadOnly),
            "state-mutating" => Some(EffectClass::LocalState),
            "generated-output-mutating" => Some(EffectClass::GeneratedOutput),
            _ => None,
        }
    }

    /// Does a call of this class leave everything as it found it?
    pub fn is_read_only(self) -> bool {
        matches!(self, EffectClass::ReadOnly)
    }

    /// May a surface run this without asking the person first? Everything that writes
    /// outside the process, and everything that reaches the network, is confirmed.
    pub fn needs_confirmation(self) -> bool {
        !matches!(self, EffectClass::ReadOnly | EffectClass::ProcessState)
    }
}

/// Whether a command needs a terminal. A command that requires one is never offered
/// through a surface that has none.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Interactivity {
    /// Runs to completion with no terminal and no input.
    #[default]
    NonInteractive,
    /// Runs without a terminal but renders better with one.
    TtyPreferred,
    /// Prompts, edits or streams; without a terminal it cannot do its work.
    TtyRequired,
}

/// Who may see a command. `Internal` is dispatched and undocumented; the surfaces that
/// list commands to people and machines skip it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Listed everywhere the policy allows.
    #[default]
    Public,
    /// Dispatched, never listed.
    Internal,
}

/// What a command needs before it can run here. Availability is derived from these, never
/// from a check written into a template or a shell script.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Requirement {
    /// Nothing: it runs wherever the executable does.
    Always,
    /// A repository with the layer initialised.
    Repository,
    /// A git checkout.
    Git,
    /// The Rust toolchain (`cargo`) on the path.
    CargoToolchain,
    /// The Rust executable built or installed.
    NativeExecutable,
    /// The site tree present.
    Site,
    /// The site's build dependencies installed (`node_modules`).
    SiteDependencies,
    /// A browser automation runtime.
    Browser,
}

impl Requirement {
    /// The sentence a surface shows when the requirement is not met. One text; the
    /// completion filter, the Cockpit's disabled state and the diagnostics all print it.
    pub fn unmet_reason(self) -> &'static str {
        match self {
            Requirement::Always => "always available",
            Requirement::Repository => "no Majordomus layer was found here",
            Requirement::Git => "this is not a git checkout",
            Requirement::CargoToolchain => "cargo is not on the path",
            Requirement::NativeExecutable => "the Rust executable is not built",
            Requirement::Site => "the site tree is not present",
            Requirement::SiteDependencies => "the site's npm dependencies are not installed",
            Requirement::Browser => "no browser automation runtime was found",
        }
    }
}

/// Whether a command can run in this checkout, and why not when it cannot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Availability {
    /// Can it run here?
    pub available: bool,
    /// What it needs.
    pub requires: Vec<Requirement>,
    /// The unmet requirement's reason, when one is unmet. One sentence, never a stack.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Availability {
    /// Available, needing nothing.
    pub fn always() -> Self {
        Availability {
            available: true,
            requires: vec![Requirement::Always],
            reason: None,
        }
    }
}

/// How sensitive an argument's value is. Nothing marked below [`Sensitivity::Public`] is
/// ever completed, cached, logged or echoed into a projection.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Sensitivity {
    /// An ordinary value: completed, shown, cached.
    #[default]
    Public,
    /// Shown but never completed from repository state (a personal note, a message).
    Sensitive,
    /// A credential. Never completed, never cached, never rendered.
    Secret,
}

/// One accepted value of an enumerated argument, with the help clap carries for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnumValue {
    /// The value as typed.
    pub value: String,
    /// Its help, when the declaration gives one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Where an argument's candidate values come from. Derived from the argument's own type
/// and placeholder, never from a completion callback written per command: an enumerated
/// argument carries its values, a path argument says so, and an identifier argument names
/// the registry that owns the identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum ValueSource {
    /// Nothing can be offered: free text.
    None,
    /// The values the type accepts, from the declaration itself.
    Enumerated {
        /// The accepted values, in declaration order.
        values: Vec<EnumValue>,
    },
    /// A path in the filesystem.
    Path {
        /// Only directories are accepted.
        directories_only: bool,
    },
    /// Identifiers a registry of this repository owns.
    Registry {
        /// Which registry.
        registry: ValueRegistry,
    },
}

impl ValueSource {
    /// The value source an argument's placeholder names. The placeholder is the one the
    /// author already writes in the declaration (`value_name = "CAPABILITY"`), so a typed
    /// argument gets its completion without a second declaration anywhere.
    ///
    /// A placeholder this table does not know yields [`ValueSource::None`]: an unknown
    /// placeholder offers nothing rather than guessing.
    ///
    /// ```
    /// use majordomus_cli::command::{ValueSource, ValueRegistry};
    /// assert!(matches!(
    ///     ValueSource::of_placeholder("CAPABILITY"),
    ///     ValueSource::Registry { registry: ValueRegistry::Capability }
    /// ));
    /// assert!(matches!(ValueSource::of_placeholder("DIR"), ValueSource::Path { directories_only: true }));
    /// assert!(matches!(ValueSource::of_placeholder("WHATEVER"), ValueSource::None));
    /// ```
    pub fn of_placeholder(placeholder: &str) -> Self {
        match placeholder {
            "DIR" => ValueSource::Path {
                directories_only: true,
            },
            "PATH" | "FILE" | "RECORD" | "OUT" => ValueSource::Path {
                directories_only: false,
            },
            "CAPABILITY" => ValueSource::Registry {
                registry: ValueRegistry::Capability,
            },
            "COMMAND" => ValueSource::Registry {
                registry: ValueRegistry::Command,
            },
            "MODULE" => ValueSource::Registry {
                registry: ValueRegistry::Module,
            },
            "KIND" => ValueSource::Registry {
                registry: ValueRegistry::ObjectKind,
            },
            "MOMENT" => ValueSource::Registry {
                registry: ValueRegistry::Moment,
            },
            "AUDIENCE" => ValueSource::Registry {
                registry: ValueRegistry::Audience,
            },
            "AREA" => ValueSource::Registry {
                registry: ValueRegistry::Area,
            },
            "TARGET" => ValueSource::Registry {
                registry: ValueRegistry::DistributionTarget,
            },
            "SURFACE" => ValueSource::Registry {
                registry: ValueRegistry::WebSurface,
            },
            "GRAPH" => ValueSource::Registry {
                registry: ValueRegistry::Graph,
            },
            "SKILL" => ValueSource::Registry {
                registry: ValueRegistry::Skill,
            },
            "RULE" => ValueSource::Registry {
                registry: ValueRegistry::Rule,
            },
            "BRANCH" => ValueSource::Registry {
                registry: ValueRegistry::Branch,
            },
            _ => ValueSource::None,
        }
    }

    /// Does answering this source need the repository's value index?
    pub fn needs_index(&self) -> bool {
        matches!(self, ValueSource::Registry { .. })
    }
}

/// A registry of this repository that owns a set of identifiers. The completion engine
/// and the Cockpit's forms both ask for values by naming one of these; neither holds a
/// list of its own.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum ValueRegistry {
    /// Capability identities, from the capability registry.
    Capability,
    /// Command identities, from this graph.
    Command,
    /// Capability module identities.
    Module,
    /// Object kinds, from the kind schema.
    ObjectKind,
    /// Operational moments of the Why catalogue.
    Moment,
    /// Audiences of the Why catalogue.
    Audience,
    /// Operational areas of the Why catalogue.
    Area,
    /// Distribution targets.
    DistributionTarget,
    /// Web surfaces of the resolved topology.
    WebSurface,
    /// Derived graphs.
    Graph,
    /// Skills of the repository's layer.
    Skill,
    /// Rules of the repository's layer.
    Rule,
    /// Local git branches.
    Branch,
}

impl ValueRegistry {
    /// The key the value index files this registry's candidates under.
    pub fn key(self) -> &'static str {
        match self {
            ValueRegistry::Capability => "capability",
            ValueRegistry::Command => "command",
            ValueRegistry::Module => "module",
            ValueRegistry::ObjectKind => "object-kind",
            ValueRegistry::Moment => "moment",
            ValueRegistry::Audience => "audience",
            ValueRegistry::Area => "area",
            ValueRegistry::DistributionTarget => "distribution-target",
            ValueRegistry::WebSurface => "web-surface",
            ValueRegistry::Graph => "graph",
            ValueRegistry::Skill => "skill",
            ValueRegistry::Rule => "rule",
            ValueRegistry::Branch => "branch",
        }
    }

    /// Every registry, in a stable order.
    pub const ALL: &'static [ValueRegistry] = &[
        ValueRegistry::Capability,
        ValueRegistry::Command,
        ValueRegistry::Module,
        ValueRegistry::ObjectKind,
        ValueRegistry::Moment,
        ValueRegistry::Audience,
        ValueRegistry::Area,
        ValueRegistry::DistributionTarget,
        ValueRegistry::WebSurface,
        ValueRegistry::Graph,
        ValueRegistry::Skill,
        ValueRegistry::Rule,
        ValueRegistry::Branch,
    ];
}

/// One argument of a command: a positional or an option, with everything a projection
/// needs to render it and a completion engine needs to offer values for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArgumentSpec {
    /// The argument's identity within its command.
    pub name: String,
    /// `--transport`, without the dashes; absent for a positional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub long: Option<String>,
    /// `-t`, without the dash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short: Option<char>,
    /// Given by position rather than by flag.
    pub positional: bool,
    /// Must be given.
    pub required: bool,
    /// Takes more than one value.
    pub variadic: bool,
    /// Accepted by every command under the one that declares it.
    pub global: bool,
    /// Takes a value at all; a flag does not.
    pub takes_value: bool,
    /// The one-line help, from the declaration.
    pub help: String,
    /// The placeholder for the value, `PATH`; the name the value source is derived from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// The default value(s); empty when there is none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub defaults: Vec<String>,
    /// Where candidate values come from.
    pub values: ValueSource,
    /// How sensitive the value is.
    pub sensitivity: Sensitivity,
}

/// How a command is run. The graph describes execution; it never carries the code, and it
/// never carries a shell string a surface would have to re-parse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Execution {
    /// The Rust executable, with these words after it.
    Native {
        /// The words after the executable's own name.
        argv: Vec<String>,
    },
    /// The shell tool, with these words after it.
    Shell {
        /// The words after `bin/majordomus`.
        argv: Vec<String>,
    },
    /// Something else this repository ships, run in order. Each step is a whole argument
    /// vector: the program and its arguments, never a line for a shell to split.
    External {
        /// The steps, in order; each is `[program, arg, ...]`.
        steps: Vec<Vec<String>>,
    },
}

impl Execution {
    /// The program a surface must invoke, for the cycle check and for the report.
    pub fn executor(&self) -> &str {
        match self {
            Execution::Native { .. } => "majordomus (rust)",
            Execution::Shell { .. } => "bin/majordomus",
            Execution::External { steps } => steps
                .first()
                .and_then(|s| s.first())
                .map(String::as_str)
                .unwrap_or("(nothing)"),
        }
    }

    /// Every program this execution invokes, for the cycle check.
    pub fn programs(&self) -> Vec<&str> {
        match self {
            Execution::Native { .. } | Execution::Shell { .. } => vec![self.executor()],
            Execution::External { steps } => steps
                .iter()
                .filter_map(|s| s.first().map(String::as_str))
                .collect(),
        }
    }
}

/// Where a command's description came from, so that every claim in a projection can be
/// traced to a file a reader can open. Never an absolute path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "source", rename_all = "kebab-case")]
#[schemars(rename = "CommandProvenance")]
pub enum Provenance {
    /// Walked out of the clap declaration in this file.
    Clap {
        /// Repository-relative path of the declaration.
        path: String,
    },
    /// Read from the shell tool's command registry.
    Registry {
        /// Repository-relative path of the registry file.
        path: String,
    },
    /// Read from one declared workflow object.
    Declared {
        /// Repository-relative path of the object.
        path: String,
    },
}

impl Provenance {
    /// The repository-relative path of the source.
    pub fn path(&self) -> &str {
        match self {
            Provenance::Clap { path }
            | Provenance::Registry { path }
            | Provenance::Declared { path } => path,
        }
    }
}

/// A name a command answers to besides its own, declared once beside the command and
/// honoured by every projection that can carry an alias.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Alias {
    /// The name.
    pub name: String,
    /// Why it exists: a rename this alias keeps working, an abbreviation, a historical
    /// spelling. One line, shown wherever the alias is.
    pub reason: String,
}

/// A command that still runs and should not be reached for. One declaration; the command
/// line's warning, the Just description, the completion entry, the reference and the
/// Cockpit all render it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Deprecation {
    /// What to use instead, as a command identity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replaced_by: Option<CommandId>,
    /// One line: why, and what changes.
    pub note: String,
}

/// The surface names a command is known by. Every one of them is computed by
/// [`super::projection`] from the identity and the graph as a whole; nothing here is
/// written by hand, and a surface that wants a name asks for this rather than deriving
/// one of its own.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Projections {
    /// The words a person types after the executable, when the command has a command line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli: Option<Vec<String>>,
    /// The generated Just recipe's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub just: Option<String>,
    /// The Cockpit's action name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cockpit: Option<String>,
    /// The route of the command's page in the generated reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
    /// Aliases the Just projection also answers to, resolved and free of collisions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub just_aliases: Vec<String>,
}

/// One documented example of a command: the argument vector it runs and what it shows.
/// Rendered by every projection from this one value, never written out a second time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Example {
    /// Unique across the whole graph.
    pub id: String,
    /// One line: what this example shows.
    pub title: String,
    /// The arguments, without the executable's own name.
    pub argv: Vec<String>,
    /// The command line a person copies, rendered from `argv`.
    pub command: String,
}

/// The canonical descriptor of one command. Everything a projection may say about a
/// command is here, and everything here came from exactly one declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandNode {
    /// The canonical identity.
    pub id: CommandId,
    /// The executable that owns it.
    pub program: Program,
    /// The path words, without the program.
    pub path: Vec<String>,
    /// The one-line description every projection shows.
    pub summary: String,
    /// The long description, when the declaration has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The usage line, derived once here.
    pub usage: String,
    /// Whether the command does something on its own, rather than only grouping others.
    pub runnable: bool,
    /// The arguments, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arguments: Vec<ArgumentSpec>,
    /// How it runs.
    pub execution: Execution,
    /// What it does to the world.
    pub effect: EffectClass,
    /// Whether it needs a terminal.
    pub interactivity: Interactivity,
    /// Who may see it.
    pub visibility: Visibility,
    /// Whether it can run here, and why not.
    pub availability: Availability,
    /// Where its description came from.
    pub provenance: Provenance,
    /// The names it also answers to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<Alias>,
    /// Whether it is on the way out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecation: Option<Deprecation>,
    /// Free tags, for grouping and for the entry recommendation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The documented examples.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<Example>,
    /// The names every surface knows it by, derived.
    pub projections: Projections,
}

impl CommandNode {
    /// Is this command offered to people who are listing what they can run?
    pub fn is_listed(&self) -> bool {
        self.visibility == Visibility::Public && self.runnable
    }

    /// The argument matching a word as typed on a command line (`--format`, `-f`).
    pub fn argument_for_word(&self, word: &str) -> Option<&ArgumentSpec> {
        let bare = word.trim_start_matches('-');
        let bare = bare.split('=').next().unwrap_or(bare);
        self.arguments.iter().find(|a| {
            a.long.as_deref() == Some(bare)
                || (bare.chars().count() == 1 && a.short == bare.chars().next())
        })
    }
}

/// A finding of the graph's own build: something a contributor got wrong, a collision, a
/// projection that could not be named. Fatal findings refuse the graph; the rest are
/// reported and the graph stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Diagnostic {
    /// A stable code a test can name.
    pub code: String,
    /// Whether the graph is refused because of it.
    pub fatal: bool,
    /// What is wrong, naming both ends when two things collide.
    pub message: String,
    /// The commands involved, in identity order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<CommandId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_grammar() {
        assert!(CommandId::parse("native.bench.coverage").is_ok());
        assert!(CommandId::parse("workflow.site-build").is_ok());
        for bad in ["", "native", "native.", "nope.thing", "native.Bench", "native.-x"] {
            assert!(CommandId::parse(bad).is_err(), "{bad:?} accepted");
        }
        let id = CommandId::parse("native.bench.coverage").unwrap();
        assert_eq!(id.program(), Program::Native);
        assert_eq!(id.path(), vec!["bench".to_string(), "coverage".to_string()]);
    }

    #[test]
    fn identity_is_built_from_program_and_path() {
        let id = CommandId::new(Program::Shell, &["doctor".to_string()]);
        assert_eq!(id.as_str(), "shell.doctor");
        assert!(CommandId::parse(id.as_str()).is_ok());
    }

    #[test]
    fn the_shell_registry_vocabulary_maps_onto_one_effect_model() {
        for (class, effect) in [
            ("read-only", EffectClass::ReadOnly),
            ("state-mutating", EffectClass::LocalState),
            ("generated-output-mutating", EffectClass::GeneratedOutput),
        ] {
            assert_eq!(EffectClass::of_shell_class(class), Some(effect));
        }
        assert!(EffectClass::ReadOnly.is_read_only());
        assert!(!EffectClass::ReadOnly.needs_confirmation());
        assert!(EffectClass::Destructive.needs_confirmation());
    }

    #[test]
    fn a_placeholder_names_its_value_source_and_an_unknown_one_offers_nothing() {
        assert!(matches!(
            ValueSource::of_placeholder("MOMENT"),
            ValueSource::Registry {
                registry: ValueRegistry::Moment
            }
        ));
        assert!(matches!(
            ValueSource::of_placeholder("PATH"),
            ValueSource::Path {
                directories_only: false
            }
        ));
        assert_eq!(ValueSource::of_placeholder("NOPE"), ValueSource::None);
        assert!(!ValueSource::None.needs_index());
        assert!(ValueSource::of_placeholder("RULE").needs_index());
    }

    #[test]
    fn precedence_is_fixed_so_a_projection_name_never_depends_on_discovery_order() {
        let mut programs = vec![Program::Workflow, Program::Native, Program::Shell];
        programs.sort_by_key(|p| p.precedence());
        assert_eq!(
            programs,
            vec![Program::Native, Program::Shell, Program::Workflow]
        );
    }
}
