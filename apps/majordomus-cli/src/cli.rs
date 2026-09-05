//! The command line, declared with clap's derive API. Every command starts from the same
//! [`RepoArgs`]; `capabilities` reaches the registry through the registry's own
//! introspection capabilities, so no list of capabilities lives here.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// The exit code for a usage error, per the exit-code contract.
pub const EXIT_USAGE: u8 = 2;

#[derive(Debug, Parser)]
#[command(
    name = "majordomus",
    version,
    about = "Majordomus control plane: a data-driven MCP server over the repository's .ai/ layer",
    long_about = "The Rust executable of Majordomus. It reads the repository's provider-neutral \
AI layer under .ai/ and serves it, read-only, to MCP clients over stdio.\n\n\
The task lifecycle (init, start, check, finish, doctor, ...) is the shell tool bin/majordomus \
in the same repository; this executable does not implement those commands."
)]
/// The command line: one of the commands below.
pub struct Cli {
    #[command(subcommand)]
    /// The command to run.
    pub command: Command,
}

#[derive(Debug, Subcommand)]
/// The commands. A command listed here is implemented; nothing is advertised ahead of its behaviour.
pub enum Command {
    /// Serve the repository's AI layer to an MCP client over stdio (read-only)
    Mcp(McpArgs),
    /// Serve the same capabilities over HTTP on the loopback interface, with /openapi.json and /docs (read-only)
    Serve(ServeArgs),
    /// Introspect the capability registry: what exists, where it came from, how it is exposed
    Capabilities(CapabilitiesArgs),
    /// Write the committed projections of the registry (docs/generated), or check that they are current
    Generate(GenerateArgs),
    /// Time every externally callable operation (each capability directly, over MCP and over HTTP, and the transports' own operations), report coverage, compare with the accepted baseline
    Bench(BenchArgs),
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
/// `majordomus bench`.
pub struct BenchArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// `coverage` or `baseline`; none runs the benchmarks.
    pub command: Option<BenchCommand>,

    /// Only targets of this capability id, or whose key starts with this text
    pub id: Option<String>,

    /// Only this transport
    #[arg(long, value_enum, default_value_t = TransportArg::All)]
    pub transport: TransportArg,

    /// How much to measure
    #[arg(long, value_enum, default_value_t = ProfileArg::Quick)]
    pub profile: ProfileArg,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    /// Output shape
    pub format: OutputFormat,

    /// Compare with the accepted baseline of this platform under .ai/repo/benchmarks/rust/policy.yaml; exit 10 on a regression
    #[arg(long)]
    pub check: bool,

    /// Do not write the result under .ai/local/benchmarks/
    #[arg(long)]
    pub no_write: bool,
}

#[derive(Debug, Subcommand)]
/// The `bench` subcommands.
pub enum BenchCommand {
    /// Every required target and whether it is covered; the denominator is generated from the registry
    Coverage {
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        /// Output shape
        format: OutputFormat,
        /// Exit 10 when any required target is missing or waived
        #[arg(long)]
        check: bool,
    },
    /// The accepted baseline of this platform under .ai/repo/benchmarks/rust/
    Baseline {
        #[command(subcommand)]
        /// What to do with it.
        command: BaselineCommand,
    },
}

#[derive(Debug, Subcommand)]
/// The `bench baseline` subcommands.
pub enum BaselineCommand {
    /// Run the benchmarks and record them as this platform's baseline (a reviewable, tracked file)
    Update {
        /// How much to measure
        #[arg(long, value_enum, default_value_t = ProfileArg::Full)]
        profile: ProfileArg,
        /// Record even from a dirty work tree
        #[arg(long)]
        allow_dirty: bool,
    },
}

/// The transports `bench` can be limited to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum TransportArg {
    /// Every target.
    #[default]
    All,
    /// Capabilities through the executor, in process.
    Direct,
    /// Capabilities through a real `majordomus mcp` child.
    Mcp,
    /// Capabilities over a real loopback socket.
    Http,
    /// The transports' own operations only.
    System,
}

/// How much `bench` measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum ProfileArg {
    /// Fast developer feedback: few samples.
    #[default]
    Quick,
    /// Stable evidence: many samples, many cold spawns.
    Full,
    /// Conservative: structural gates plus a modest measurement.
    Ci,
}

impl ProfileArg {
    /// The name the benchmark module knows.
    pub fn name(self) -> &'static str {
        match self {
            ProfileArg::Quick => "quick",
            ProfileArg::Full => "full",
            ProfileArg::Ci => "ci",
        }
    }
}

/// Output shape for commands that print to a person or a script. `mcp` speaks its own
/// protocol and does not use it; `mcp --inspect` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum OutputFormat {
    #[default]
    /// Lines for a person.
    Text,
    /// One JSON document, deterministic.
    Json,
}

/// How files are enumerated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum DiscoveryMode {
    /// Tracked files, through the version-control index (the layer's contract)
    #[default]
    Vcs,
    /// A walk of the work tree with the same glob semantics; untracked files included
    Filesystem,
}

/// The transports available. One today; the option exists so that a second one is an
/// addition, not a redesign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum Transport {
    #[default]
    /// One JSON-RPC frame per line on stdin and stdout.
    Stdio,
}

/// Where and how the repository is read; shared by every command that reads it.
#[derive(Debug, Args, Default)]
pub struct RepoArgs {
    /// Start the search for the repository root here (default: the current directory)
    #[arg(long, value_name = "PATH", global = true)]
    pub repo: Option<PathBuf>,

    /// How declarative files are enumerated
    #[arg(long, value_enum, default_value_t = DiscoveryMode::Vcs, global = true)]
    pub discovery: DiscoveryMode,

    /// Refuse to proceed when any file of the layer carries an error diagnostic
    #[arg(long, global = true)]
    pub strict: bool,

    /// The tool distribution's share directory (kinds.yaml, schemas/); default: $MAJORDOMUS_SHARE,
    /// then the repository's own share/, then the one beside the executable
    #[arg(long, value_name = "DIR", global = true)]
    pub share: Option<PathBuf>,
}

#[derive(Debug, Args)]
/// `majordomus mcp`.
pub struct McpArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    /// Print what would be served, and every diagnostic, then exit without serving
    #[arg(long)]
    pub inspect: bool,

    /// Output shape of --inspect
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,

    /// The transport to serve on
    #[arg(long, value_enum, default_value_t = Transport::Stdio)]
    pub transport: Transport,

    /// Serve this client alone: no shared server, no HTTP, no Swagger UI, no peers, and
    /// nothing written anywhere. The default is the shared server (below)
    #[arg(long)]
    pub standalone: bool,

    /// Interface the shared server binds when this process is the one that starts it
    #[arg(long, default_value = "127.0.0.1", value_name = "HOST")]
    pub http_host: String,

    /// Port the shared server binds when this process starts it; when it is taken, a free
    /// port is used instead and the URL is logged on stderr either way
    #[arg(long, default_value_t = DEFAULT_PORT, value_name = "PORT")]
    pub http_port: u16,
}

/// The default port of the HTTP projection: `serve`, and the shared server `mcp` starts.
pub const DEFAULT_PORT: u16 = 8741;

#[derive(Debug, Args)]
/// `majordomus serve`.
pub struct ServeArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    /// Interface to bind; loopback unless you say otherwise
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to bind; 0 picks a free one and the address is logged on stderr
    #[arg(long, default_value_t = DEFAULT_PORT)]
    pub port: u16,
}

#[derive(Debug, Args)]
/// `majordomus capabilities`.
pub struct CapabilitiesArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// Which introspection.
    pub command: CapabilitiesCommand,
}

/// Which schema of a capability to print.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum SchemaSide {
    #[default]
    /// The schema of the input.
    Input,
    /// The schema of the output.
    Output,
}

#[derive(Debug, Subcommand)]
/// The introspection commands; `list` and `describe` dispatch through the registry's CLI exposure.
pub enum CapabilitiesCommand {
    /// Every capability, one line each, with its projections
    List {
        /// Only this kind: query or resource
        #[arg(long)]
        kind: Option<String>,
        /// Only capabilities exposed through this projection: mcp, http or cli
        #[arg(long)]
        exposure: Option<String>,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        /// Output shape.
        format: OutputFormat,
    },
    /// One capability by canonical id: schemas, provenance, every projection
    Describe {
        /// The canonical id.
        id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        /// Output shape.
        format: OutputFormat,
    },
    /// The canonical input or output JSON Schema of one capability
    Schema {
        /// The canonical id.
        id: String,
        #[arg(long, value_enum, default_value_t = SchemaSide::Input)]
        /// Input or output.
        side: SchemaSide,
    },
    /// Build the registry and every projection; exit 10 with every violation named
    Validate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
/// What `generate` writes.
pub enum GenerateTarget {
    #[default]
    /// Every target.
    All,
    /// `docs/generated/openapi.json`.
    Openapi,
    /// `docs/generated/capabilities.md` and `docs/generated/modules/<id>.md`.
    Docs,
    /// `docs/generated/benchmarks.md`: every benchmark target and the coverage
    Benchmarks,
    /// `docs/generated/registry.json`: the builtin registry as data
    Registry,
    /// The shell tool's allow-lists under share/allow, derived from the schemas
    Allow,
    /// The provider bootstraps the policy declares (AGENTS.md, CLAUDE.md, ...)
    Providers,
    /// site/data/registry/registry.json, the registry dataset the site renders
    Site,
}

#[derive(Debug, Args)]
/// `majordomus generate`.
pub struct GenerateArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    /// What to generate
    #[arg(value_enum, default_value_t = GenerateTarget::All)]
    pub target: GenerateTarget,

    /// Compare with what is on disk and exit 10 when stale; write nothing
    #[arg(long)]
    pub check: bool,

    /// Write under this directory instead of the repository root (docs/generated is appended)
    #[arg(long, value_name = "DIR")]
    pub out: Option<PathBuf>,
}

// ------------------------------------------------------------------ the command line as data
//
// clap is the one declaration of the command line: every command, argument, default and
// value set above. The reference of the native CLI (`docs/generated/cli.md`, the site's
// page for it) is rendered from this walk of the built `Command`, never from `--help`
// prose and never from a list kept beside the declaration.

/// One command of the executable's command line, as clap declares it, with the commands
/// under it. The root is `majordomus`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CommandDoc {
    /// The full path, `["majordomus", "bench", "coverage"]`.
    pub path: Vec<String>,
    /// The one-line description.
    pub about: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The long description, when the command has one.
    pub long_about: Option<String>,
    /// The arguments in declaration order: positionals and options, `--help` and
    /// `--version` excluded.
    pub args: Vec<ArgDoc>,
    /// The subcommands in declaration order.
    pub subcommands: Vec<CommandDoc>,
}

/// One argument of a command.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ArgDoc {
    /// The argument's id, `transport`.
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// `--transport`, without the dashes.
    pub long: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// `-t`, without the dash.
    pub short: Option<char>,
    /// Given by position rather than by flag.
    pub positional: bool,
    /// The help text.
    pub help: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The placeholder for the value, `PATH`.
    pub value_name: Option<String>,
    /// Whether the argument takes a value at all (a flag does not).
    pub takes_value: bool,
    /// Must be given.
    pub required: bool,
    /// Accepted by every command under the one that declares it.
    pub global: bool,
    /// The values a value-enum argument accepts, with the help of each; empty for any
    /// other argument. Always present, so a template can ask for its length.
    pub possible_values: Vec<PossibleValueDoc>,
    /// The default value(s), as clap renders them; empty when there is none.
    pub defaults: Vec<String>,
}

/// One value of a value-enum argument.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PossibleValueDoc {
    /// The value as typed.
    pub name: String,
    /// Its help, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

/// The whole command line, from the built clap declaration. Deterministic: clap keeps
/// declaration order, and nothing here depends on the environment.
pub fn tree() -> CommandDoc {
    use clap::CommandFactory;
    let mut root = Cli::command();
    root.build();
    walk(&root, Vec::new())
}

fn walk(cmd: &clap::Command, mut path: Vec<String>) -> CommandDoc {
    path.push(cmd.get_name().to_string());
    let args = cmd
        .get_arguments()
        .filter(|a| !matches!(a.get_id().as_str(), "help" | "version"))
        .map(|a| ArgDoc {
            name: a.get_id().to_string(),
            long: a.get_long().map(str::to_string),
            short: a.get_short(),
            positional: a.is_positional(),
            help: a.get_help().map(|h| h.to_string()).unwrap_or_default(),
            value_name: a
                .get_value_names()
                .and_then(|names| names.first())
                .map(|n| n.to_string()),
            takes_value: a.get_action().takes_values(),
            required: a.is_required_set(),
            global: a.is_global_set(),
            possible_values: a
                .get_possible_values()
                .into_iter()
                .filter(|v| !v.is_hide_set())
                .map(|v| PossibleValueDoc {
                    name: v.get_name().to_string(),
                    help: v.get_help().map(|h| h.to_string()),
                })
                .collect(),
            defaults: a
                .get_default_values()
                .iter()
                .map(|v| v.to_string_lossy().into_owned())
                .collect(),
        })
        .collect();
    // clap adds a `help` subcommand to every command that has subcommands; it documents
    // nothing of its own and is left out, as `--help` is among the arguments
    let subcommands = cmd
        .get_subcommands()
        .filter(|s| !s.is_hide_set() && s.get_name() != "help")
        .map(|s| walk(s, path.clone()))
        .collect();
    CommandDoc {
        path,
        about: cmd.get_about().map(|a| a.to_string()).unwrap_or_default(),
        long_about: cmd.get_long_about().map(|a| a.to_string()),
        args,
        subcommands,
    }
}

impl CommandDoc {
    /// Every command, this one first, then the subcommands depth first.
    pub fn flatten(&self) -> Vec<&CommandDoc> {
        let mut out = vec![self];
        for s in &self.subcommands {
            out.extend(s.flatten());
        }
        out
    }
}
