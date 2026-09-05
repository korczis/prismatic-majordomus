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
    /// The repository scope: what a worker reads and what it never reads; with paths, whether each is in or out and why
    Scope(ScopeArgs),
}

#[derive(Debug, Args)]
/// `majordomus scope`.
pub struct ScopeArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    /// Repository-relative paths to judge; none prints the declaration and the tally
    pub paths: Vec<String>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    /// Output shape
    pub format: OutputFormat,

    /// Exit 10 when any path given is out of the scope
    #[arg(long)]
    pub check: bool,
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
// value set above. What clap cannot carry — the examples a reader copies — is declared
// below, in the same file, in typed Rust. Both halves are walked into `CommandDoc` by
// `cli::tree()`, and every projection of the native command line is a rendering of that
// tree: `--help`, `docs/generated/cli.md`, `docs/generated/cli.json`, the site dataset and
// the routes under /docs/cli/. `cli::validate` is the contract that keeps the two halves
// complete, and the example tests execute exactly the argv shown below.

mod docs;
mod validate;

pub use docs::tree;
pub use docs::{
    document, render, route, ArgDoc, CliDocument, CommandDoc, ExampleView, PossibleValueDoc,
    SetupView, DECLARATION, ROUTE_PREFIX, SCHEMA,
};
pub use validate::{parse, validate, Violation};

/// What an example's run must show for the example to be true. Small on purpose: enough to
/// prove that the command line printed in the documentation does what the documentation
/// says, and no more. A new variant is a new kind of evidence, not a new test framework.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// The command exits 0.
    Success,
    /// The command exits with exactly this code.
    ExitCode(u8),
    /// The command exits 0 and its stdout contains every fragment.
    StdoutContains(&'static [&'static str]),
    /// The command exits 0 and its stdout is one JSON document in which every JSON Pointer
    /// resolves (RFC 6901, `/registry/fingerprint`).
    Json(&'static [&'static str]),
    /// A server: it starts, logs the address it bound, answers a GET on this path with a
    /// 2xx status, and exits 0 when it is asked to stop.
    HttpReady(&'static str),
    /// The stdio MCP server: it starts, answers `initialize` and `tools/list` with JSON-RPC
    /// frames on stdout and nothing else, and exits 0 at end of input.
    McpReady,
}

impl Expect {
    /// What the example test asserts, in one phrase, for the reference to print.
    pub fn describe(self) -> String {
        match self {
            Expect::Success => "exits 0".to_string(),
            Expect::ExitCode(c) => format!("exits {c}"),
            Expect::StdoutContains(f) => format!("exits 0; prints {}", f.join(", ")),
            Expect::Json(p) => format!(
                "exits 0; prints one JSON document carrying {}",
                p.join(", ")
            ),
            Expect::HttpReady(path) => {
                format!("binds a port, answers GET {path}, exits 0 when stopped")
            }
            Expect::McpReady => {
                "answers initialize and tools/list on stdio, and exits 0 at end of input"
                    .to_string()
            }
        }
    }
}

/// One documented example of one command: what it shows, the argument vector it runs, what
/// must happen when it runs, and anything that has to be done first. The argument vector is
/// the canonical form; the command line a reader copies is rendered from it, never written
/// out a second time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExampleDoc {
    /// Unique across the whole command line; the anchor of the example on its page.
    pub id: &'static str,
    /// One line: what this example shows.
    pub title: &'static str,
    /// What it does and what comes back.
    pub description: &'static str,
    /// The arguments, without the executable's own name.
    pub argv: &'static [&'static str],
    /// Commands run in the same repository before it, in order; usually empty.
    pub setup: &'static [&'static [&'static str]],
    /// What the run must show.
    pub expect: Expect,
}

/// The examples of one command, by the command's path without `majordomus`. The root is the
/// empty string. `cli::validate` refuses a set whose command the clap declaration does not
/// have, a command that can be run and has no set, and an example whose argv this crate's
/// own parser does not accept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandExamples {
    /// `bench baseline update`; the empty string is the root.
    pub command: &'static str,
    /// Its examples, in the order the reference prints them.
    pub examples: &'static [ExampleDoc],
}

/// Every documented example of the native command line.
///
/// This is the canonical declaration: `docs/generated/cli.md`, `docs/generated/cli.json`,
/// the site dataset and every page under `/docs/cli/` render these entries, and
/// `apps/majordomus-cli/tests/cli_examples.rs` runs them against the built executable in a
/// disposable repository. Adding a command without adding its example does not pass
/// `cli::validate`, and therefore does not pass the crate's tests or CI.
pub const EXAMPLES: &[CommandExamples] = &[
    CommandExamples {
        command: "mcp",
        examples: &[
            ExampleDoc {
                id: "mcp-inspect",
                title: "See what would be served, without serving it",
                description: "Builds the registry and the index of the repository in the working directory and prints the repository, the capabilities, the objects and every diagnostic, then exits. Nothing is served and nothing is written.",
                argv: &["mcp", "--inspect"],
                setup: &[],
                expect: Expect::StdoutContains(&["repository", "capabilities"]),
            },
            ExampleDoc {
                id: "mcp-inspect-json",
                title: "The same, as one JSON document for a script",
                description: "The shape `--inspect` prints for a person, as JSON: the repository, its discovery mode, the capabilities and the diagnostics, deterministic and safe to diff.",
                argv: &["mcp", "--inspect", "--format", "json"],
                setup: &[],
                expect: Expect::Json(&["/repository/repository/root", "/tools"]),
            },
            ExampleDoc {
                id: "mcp-stdio",
                title: "Serve one MCP client on stdio",
                description: "The form an MCP client spawns: JSON-RPC frames in on stdin, frames out on stdout, logs on stderr, and the session ends at end of input. `--standalone` keeps this process to itself: no shared server, no HTTP, nothing written anywhere.",
                argv: &["mcp", "--standalone"],
                setup: &[],
                expect: Expect::McpReady,
            },
        ],
    },
    CommandExamples {
        command: "serve",
        examples: &[ExampleDoc {
            id: "serve-ephemeral-port",
            title: "Serve the same capabilities over HTTP on a free port",
            description: "Port 0 asks the operating system for a free port; the address is logged on stderr. The document at /openapi.json is the same one `majordomus generate` commits, and /docs is the Swagger UI over it.",
            argv: &["serve", "--port", "0"],
            setup: &[],
            expect: Expect::HttpReady("/openapi.json"),
        }],
    },
    CommandExamples {
        command: "capabilities list",
        examples: &[
            ExampleDoc {
                id: "capabilities-list-cli",
                title: "Which capabilities the command line itself dispatches to",
                description: "One line per capability exposed through the `cli` projection, with the projections of each. The registry answers this; no list of capabilities is written in the command line's own declaration.",
                argv: &["capabilities", "list", "--exposure", "cli"],
                setup: &[],
                expect: Expect::StdoutContains(&["capabilities.list", "capabilities.describe"]),
            },
            ExampleDoc {
                id: "capabilities-list-json",
                title: "Every capability as one JSON document",
                description: "The whole registry for a script: each capability with its kind, its provenance and every projection it has.",
                argv: &["capabilities", "list", "--format", "json"],
                setup: &[],
                expect: Expect::Json(&["/capabilities"]),
            },
        ],
    },
    CommandExamples {
        command: "capabilities describe",
        examples: &[ExampleDoc {
            id: "capabilities-describe-objects-get",
            title: "One capability in full, by its canonical id",
            description: "Its kind, its input and output schemas, where it was composed, and every projection of it: the MCP tool or resource, the HTTP route, the CLI path.",
            argv: &["capabilities", "describe", "objects.get"],
            setup: &[],
            expect: Expect::StdoutContains(&["objects.get", "GET /api/v1/object"]),
        }],
    },
    CommandExamples {
        command: "capabilities schema",
        examples: &[ExampleDoc {
            id: "capabilities-schema-output",
            title: "The canonical output schema of a capability",
            description: "The JSON Schema the MCP and OpenAPI projections are derived from; `--side input` prints the schema of what the capability accepts.",
            argv: &["capabilities", "schema", "objects.get", "--side", "output"],
            setup: &[],
            expect: Expect::Json(&["/title"]),
        }],
    },
    CommandExamples {
        command: "capabilities validate",
        examples: &[ExampleDoc {
            id: "capabilities-validate",
            title: "Prove the registry and every projection of it",
            description: "Builds the registry, the MCP and HTTP surfaces, the OpenAPI document, the command line's documentation and the benchmark coverage, and names every failure. Exit 10 when anything is unmet.",
            argv: &["capabilities", "validate"],
            setup: &[],
            expect: Expect::StdoutContains(&["validate: 0 failure(s)", "OK   cli"]),
        }],
    },
    CommandExamples {
        command: "generate",
        examples: &[
            ExampleDoc {
                id: "generate-all",
                title: "Write every committed projection",
                description: "The OpenAPI document, the capability reference, the command-line reference and its JSON, the registry manifest, the benchmark matrix, the shell tool's allow-lists, the provider bootstraps and the site's registry dataset — all from the one registry and the one clap declaration.",
                argv: &["generate"],
                setup: &[],
                expect: Expect::Success,
            },
            ExampleDoc {
                id: "generate-check",
                title: "Refuse a tree whose projections are stale",
                description: "Writes nothing and compares instead: exit 0 when every committed projection is what the sources produce, exit 10 with each stale file named. This is the form CI runs.",
                argv: &["generate", "--check"],
                setup: &[&["generate"]],
                expect: Expect::Success,
            },
            ExampleDoc {
                id: "generate-one-target",
                title: "One target only",
                description: "Each target can be written on its own while a change is iterated on; `majordomus generate` with no target writes all of them.",
                argv: &["generate", "openapi"],
                setup: &[],
                expect: Expect::Success,
            },
        ],
    },
    CommandExamples {
        command: "bench",
        examples: &[ExampleDoc {
            id: "bench-direct-quick",
            title: "Time the capabilities in process",
            description: "The quick profile takes few samples, and `--transport direct` measures the executor without spawning a server. Nothing is written under .ai/local/ with `--no-write`.",
            argv: &[
                "bench",
                "--transport",
                "direct",
                "--profile",
                "quick",
                "--no-write",
                "--format",
                "json",
            ],
            setup: &[],
            expect: Expect::Json(&["/results", "/profile"]),
        }],
    },
    CommandExamples {
        command: "bench coverage",
        examples: &[
            ExampleDoc {
                id: "bench-coverage-json",
                title: "Every required benchmark target and whether it is covered",
                description: "The denominator is generated from the registry: every executable capability, on every transport it is exposed on, plus the transports' own operations.",
                argv: &["bench", "coverage", "--format", "json"],
                setup: &[],
                expect: Expect::Json(&["/lines", "/tallies"]),
            },
            ExampleDoc {
                id: "bench-coverage-check",
                title: "Fail when a target is missing",
                description: "Exit 10 when any required target is uncovered or waived, so a capability that nothing times cannot be merged.",
                argv: &["bench", "coverage", "--check"],
                setup: &[],
                expect: Expect::Success,
            },
        ],
    },
    CommandExamples {
        command: "bench baseline update",
        examples: &[ExampleDoc {
            id: "bench-baseline-update-quick",
            title: "Record this platform's accepted baseline",
            description: "Runs the benchmarks and writes the result under .ai/repo/benchmarks/rust/ as a tracked, reviewable file. The full profile is the default; the quick profile is for trying the path out. A dirty work tree is refused unless --allow-dirty says otherwise.",
            argv: &[
                "bench",
                "baseline",
                "update",
                "--profile",
                "quick",
                "--allow-dirty",
            ],
            setup: &[],
            expect: Expect::Success,
        }],
    },
    CommandExamples {
        command: "scope",
        examples: &[
            ExampleDoc {
                id: "scope-declaration",
                title: "What a worker reads of this repository",
                description: "With no path, the declaration itself and the tally: how many tracked files are in the scope and how many are out.",
                argv: &["scope"],
                setup: &[],
                expect: Expect::Success,
            },
            ExampleDoc {
                id: "scope-paths-json",
                title: "Judge paths, and say which rule decided",
                description: "For each path: in or out, and the rule that decided it. `--check` exits 10 when any path given is out, which is how a hook refuses to read one.",
                argv: &["scope", "docs/CLI.md", "--format", "json"],
                setup: &[],
                expect: Expect::Json(&["/0/verdict", "/0/rule"]),
            },
        ],
    },
];
