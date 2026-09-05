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
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Serve the repository's AI layer to an MCP client over stdio (read-only)
    Mcp(McpArgs),
    /// Serve the same capabilities over HTTP on the loopback interface, with /openapi.json and /docs (read-only)
    Serve(ServeArgs),
    /// Introspect the capability registry: what exists, where it came from, how it is exposed
    Capabilities(CapabilitiesArgs),
    /// Write the committed projections of the registry (docs/generated), or check that they are current
    Generate(GenerateArgs),
}

/// Output shape for commands that print to a person or a script. `mcp` speaks its own
/// protocol and does not use it; `mcp --inspect` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum OutputFormat {
    #[default]
    Text,
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
pub struct McpArgs {
    #[command(flatten)]
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
}

/// The default port of `serve`.
pub const DEFAULT_PORT: u16 = 8741;

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[command(flatten)]
    pub repo: RepoArgs,

    /// Interface to bind; loopback unless you say otherwise
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to bind; 0 picks a free one and the address is logged on stderr
    #[arg(long, default_value_t = DEFAULT_PORT)]
    pub port: u16,
}

#[derive(Debug, Args)]
pub struct CapabilitiesArgs {
    #[command(flatten)]
    pub repo: RepoArgs,

    #[command(subcommand)]
    pub command: CapabilitiesCommand,
}

/// Which schema of a capability to print.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum SchemaSide {
    #[default]
    Input,
    Output,
}

#[derive(Debug, Subcommand)]
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
        format: OutputFormat,
    },
    /// One capability by canonical id: schemas, provenance, every projection
    Describe {
        id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    /// The canonical input or output JSON Schema of one capability
    Schema {
        id: String,
        #[arg(long, value_enum, default_value_t = SchemaSide::Input)]
        side: SchemaSide,
    },
    /// Build the registry and every projection; exit 10 with every violation named
    Validate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum GenerateTarget {
    #[default]
    All,
    Openapi,
    Docs,
    /// The shell tool's allow-lists under share/allow, derived from the schemas
    Allow,
}

#[derive(Debug, Args)]
pub struct GenerateArgs {
    #[command(flatten)]
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
