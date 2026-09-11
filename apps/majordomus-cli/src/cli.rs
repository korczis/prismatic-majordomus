//! The command line, declared with clap's derive API. Every command starts from the same
//! [`RepoArgs`]; `capabilities` reaches the registry through the registry's own
//! introspection capabilities, so no list of capabilities lives here.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::capability::builtin::Checkouts;

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
    /// Serve the same capabilities over HTTP on the loopback interface, with the home page, /openapi.json, /swagger and the documentation under /docs/ (read-only)
    Serve(ServeArgs),
    /// Introspect the capability registry: what exists, where it came from, how it is exposed
    Capabilities(CapabilitiesArgs),
    /// Write the committed projections of the registry (docs/generated), or check that they are current
    Generate(GenerateArgs),
    /// Time every externally callable operation (each capability directly, over MCP and over HTTP, and the transports' own operations), report coverage, compare with the accepted baseline
    Bench(BenchArgs),
    /// The repository scope: what a worker reads and what it never reads; with paths, whether each is in or out and why
    Scope(ScopeArgs),
    /// The repository's web surfaces: what is exposed, where it is mounted, what produced it, and whether the topology is valid
    Web(WebArgs),
    /// The operational moments this tool answers: the catalogue, one moment, the audiences and areas, a diagnosis of your own week, and the catalogue's own validation
    Why(WhyArgs),
    /// One issue or one milestone as an executable development scope: what was authored, where the plan graph puts it, what happened to it, whether a worker may start it — with the origin of every value on the value
    Devtask(DevtaskArgs),
    /// How this project is packaged, published and installed: the platforms, the artifact names, the installer, the releases
    Distribution(DistributionArgs),
    /// What this checkout is: the project, version control, the toolchains it declares, what the layer holds, the workflows, the provider projections and the local services
    Env(EnvArgs),
    /// Every command this repository offers, from whichever program offers it: the graph, one command, where each one is projected, and the workflow bridge derived from it
    Commands(CommandsArgs),
    /// Completion for any surface, answered from the command graph: the candidates a shell asks for, and the one-time integration that asks
    Completion(CompletionArgs),
    /// The branch-to-worktree topology: where every linked worktree belongs (`<repo>-wt/<branch>`), where each one is, and the lifecycle — create, migrate, repair, guard
    #[command(alias = "wt")]
    Worktree(WorktreeArgs),
    /// The product: what this repository's tool does for a person, as the features under the layer declare it, with every surface, count and moment derived; the matrix of features against interfaces; the providers; and the model's own validation
    Product(ProductArgs),
    /// What this project has shipped and what it would ship next: the changelog derived from the layer's own records, the version the two writers state, and the one command that raises both
    Release(ReleaseArgs),
    /// What this executable's own public surface is held to: documentation, executable examples, module coverage, and every command accounted for against the capability registry
    Quality(QualityArgs),
    /// Run a capability as an execution and follow it: its steps, its progress and its output as they happen
    Run(RunArgs),
    /// The executions of the server serving this repository: what has run, what is running, and what each one said
    Executions(ExecutionsArgs),
    /// The context a development session should be given, compiled from the repository: for an issue, a milestone, an intent or a set of paths, what is selected and why, what was left out and why, what collapsed into what, and the budget
    Devcontext(DevcontextArgs),
    /// The mesh: the nodes this repository's running server has discovered on the network, this machine's node identity, and the self-check that proves the prerequisites on this machine alone
    Mesh(MeshArgs),
    /// The model catalogue the distribution declares, and the explainable routing over it: vendors, canonical model references, typed capabilities, and which model a stated need selects — with why, for every candidate
    Models(ModelsArgs),
    /// What actually ran and what it proves: every claim of the matrix against the runs recorded for it, one claim's proof, one test's claims, and the recording of a run that happened
    Evidence(EvidenceArgs),
    /// Every rule against the proof there is for it: what each one names, whether it is in the tree, whether a runner drives it, whether anything ran, and whether what ran is older than what it is about
    Rules(RulesArgs),
}

#[derive(Debug, Args)]
/// `majordomus models`.
pub struct ModelsArgs {
    #[command(subcommand)]
    /// `list`, `route`.
    pub command: ModelsCommand,
}

#[derive(Debug, Subcommand)]
/// The `models` subcommands.
pub enum ModelsCommand {
    /// Every declared vendor and model, optionally narrowed; the order is the declaration's, which is routing's preference order
    List(ModelsListArgs),
    /// Which model a stated need selects, the fallback chain behind it, and why every excluded model fell out
    Route(ModelsRouteArgs),
}

#[derive(Debug, Args)]
/// `majordomus models list`.
pub struct ModelsListArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[arg(long)]
    /// Only this vendor.
    pub vendor: Option<String>,

    #[arg(long)]
    /// Only models declaring this capability word.
    pub capability: Option<String>,

    #[arg(long)]
    /// One model, by canonical id or alias.
    pub id: Option<String>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    /// `text` for a person, `json` for a machine; both render the same answer.
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
/// `majordomus models route`.
pub struct ModelsRouteArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[arg(long)]
    /// Capability words the model must declare, comma-separated: `vision,tools`.
    pub require: Option<String>,

    #[arg(long)]
    /// The least context window, tokens.
    pub min_context: Option<u64>,

    #[arg(long)]
    /// Only this vendor.
    pub vendor: Option<String>,

    #[arg(long)]
    /// Only local inference.
    pub local_only: bool,

    #[arg(long)]
    /// A model named outright, by canonical id or alias; still checked against the other requirements.
    pub model: Option<String>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    /// `text` for a person, `json` for a machine; both render the same answer.
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
/// `majordomus mesh`.
pub struct MeshArgs {
    #[command(subcommand)]
    /// `status`, `nodes`, `identity`, `doctor`.
    pub command: MeshCommand,
}

#[derive(Debug, Subcommand)]
/// The `mesh` subcommands.
pub enum MeshCommand {
    /// Whether the mesh runs in this checkout's server and why not when it does not, with every provider's state and the registry's tallies
    Status(MeshQueryArgs),
    /// Every node the running server has observed, deduplicated by node identity, with trust, presence, endpoints and provenance
    Nodes(MeshQueryArgs),
    /// This machine's node identity, public half only; absent is an answer, not an error
    Identity(MeshQueryArgs),
    /// Prove the mesh prerequisites on this machine alone: declaration, identity, sockets, multicast, broadcast, and the protocol end to end
    Doctor(MeshQueryArgs),
}

#[derive(Debug, Args)]
/// One read-only `mesh` question.
pub struct MeshQueryArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    /// `text` for a person, `json` for a machine; both render the same answer.
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
/// `majordomus product`. The filters and the output shape are global, so they read the way
/// a person writes them — `product list --featured` — and are declared once.
pub struct ProductArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// `list`, `show`, `matrix`, `providers` or `validate`; none lists.
    pub command: Option<ProductCommand>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,

    /// Only the features the homepage shows
    #[arg(long, global = true)]
    pub featured: bool,
    /// Include drafts and deprecated features, not only the stable ones
    #[arg(long, global = true)]
    pub all: bool,
    /// Only features serving this operational area of the why catalogue
    #[arg(long, global = true)]
    pub area: Option<String>,
    /// Only features made of this capability module
    #[arg(long, global = true)]
    pub module: Option<String>,
    /// Only features made of this shell command
    #[arg(long = "names-command", global = true)]
    pub names_command: Option<String>,
    /// Only features exposed through this surface: cli, api, mcp, cockpit or docs
    #[arg(long, global = true)]
    pub surface: Option<String>,
    /// Case-insensitive text over identities, titles, headlines, summaries, tags and bodies
    #[arg(long, short = 'q', global = true)]
    pub query: Option<String>,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus product`.
pub enum ProductCommand {
    /// Every feature, narrowed by any filter, with the surfaces derived for each
    List,
    /// One feature in full: what it is made of, resolved, and everything derived from that
    Show {
        /// The feature's id, which is also its slug and its route
        id: String,
    },
    /// Every feature against every interface, and every module, command and kind against the features that name it
    Matrix,
    /// Every provider the tool has an adapter for, with what this repository does with it
    Providers,
    /// Every finding over the model; exit 10 when any is an error
    Validate,
}

#[derive(Debug, Args)]
/// `majordomus env`.
pub struct EnvArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// What to do with the snapshot; none prints it.
    pub command: Option<EnvCommand>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus env`.
pub enum EnvCommand {
    /// The whole snapshot, resolved in full: what the layer holds is counted, and the cache the banner reads is written
    Status,
    /// Render the snapshot for a terminal. Goes to standard error, never standard output, because direnv reads standard output as the environment it is setting
    Banner {
        /// How much to show: `auto`, `full`, `compact` or `off`. Without it, MAJORDOMUS_BANNER decides, and without that, `auto` — which is silent when nothing is watching, shows the whole box when the repository has something new to say, and the two-line form when it does not
        #[arg(long, value_name = "MODE")]
        mode: Option<String>,
        /// Draw as if the terminal were this wide, whatever it is
        #[arg(long, value_name = "COLUMNS")]
        width: Option<usize>,
    },
    /// The variable assignments a shell in this repository benefits from, for `eval`. Assignments only: no command, no side effect
    Export {
        /// The shell to write for: `direnv`, `bash`, `zsh`, `sh`, `ksh` or `fish`
        #[arg(long = "shell", value_name = "SHELL", default_value = "direnv")]
        shell: String,
        /// Also draw the banner, to standard error, from the same snapshot. What an adapter asks for: one process on the path a shell takes on every entry, rather than two that each pay for a `git status`
        #[arg(long)]
        banner: bool,
        /// With --banner, how much to show; MAJORDOMUS_BANNER decides without it
        #[arg(long, value_name = "MODE", requires = "banner")]
        mode: Option<String>,
        /// Also refresh the workflow bridge under .ai/local/cache/ when a declaration behind it has changed. A few `stat` calls when nothing has; never a build, never a network call
        #[arg(long)]
        bridge: bool,
    },
    /// Enter the repository: the assignments a shell here benefits from on standard output, the banner on standard error, the workflow bridge refreshed when a declaration behind it moved, and the runtime ensured — the whole of what entering this repository is, as one call, so that no person and no agent has to remember a sequence. Never builds, never reaches a remote network, and never waits for a server it started to answer
    Enter {
        /// The shell to write for: `direnv`, `bash`, `zsh`, `sh`, `ksh` or `fish`
        #[arg(long = "shell", value_name = "SHELL", default_value = "direnv")]
        shell: String,
        /// How much banner to draw; MAJORDOMUS_BANNER decides without it
        #[arg(long, value_name = "MODE", conflicts_with = "no_banner")]
        mode: Option<String>,
        /// Do not draw the banner
        #[arg(long = "no-banner")]
        no_banner: bool,
        /// Do not refresh the workflow bridge
        #[arg(long = "no-bridge")]
        no_bridge: bool,
        /// Do not ensure the runtime: export, draw and refresh only. What MAJORDOMUS_RUNTIME=off says, as an argument
        #[arg(long = "no-runtime")]
        no_runtime: bool,
        /// Wait this many seconds for a server this call started to answer. Zero — the default, and what a shell prompt asks for — returns as soon as one has been started, and the entry file's watch over the lease brings the address in when it is published
        #[arg(long, value_name = "SECONDS", default_value_t = 0)]
        wait: u64,
    },
    /// Where each value came from: the file, command or constant that decided it, the resolver that read it, and how far it can be trusted
    Explain {
        /// One field in dotted form (`vcs.branch`, `layer.objects`), or a prefix; every field when absent
        #[arg(value_name = "FIELD")]
        field: Option<String>,
    },
}

#[derive(Debug, Args)]
/// `majordomus release`. The read half is derived and the write half is one command, so
/// that raising a version is a thing that happens once rather than in two files by hand.
pub struct ReleaseArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// What to answer; none prints the changelog.
    pub command: Option<ReleaseCommand>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// How to render the answer.
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus release`.
pub enum ReleaseCommand {
    /// The changelog, composed from the layer's release records, the decisions dated inside each release's window, and the conventional commits in its range
    Changelog {
        /// One version or `unreleased`; every section when absent
        #[arg(value_name = "VERSION")]
        version: Option<String>,
    },
    /// The version the two writers state, whether they agree, and the bump the commits since the last release imply
    Version,
    /// Raise the version in both places at once, to the bump the commits imply or to one you name
    Bump {
        /// Raise by this much instead of by what the commits imply
        #[arg(long, value_name = "LEVEL")]
        level: Option<String>,
        /// Set exactly this version, instead of raising the current one
        #[arg(long, value_name = "VERSION", conflicts_with = "level")]
        exact: Option<String>,
        /// Say what would change and write nothing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Args)]
/// `majordomus quality`. One subcommand today; declared as a group so that a second
/// measurement joins it rather than crowding the root.
pub struct QualityArgs {
    #[command(subcommand)]
    /// `report`.
    pub command: QualityCommand,
}

#[derive(Debug, Subcommand)]
/// The `quality` subcommands.
pub enum QualityCommand {
    /// Measure the crate and report every finding, with the rule it breaks and what to do about it
    Report(QualityReportArgs),
}

#[derive(Debug, Args)]
/// `majordomus quality report`.
pub struct QualityReportArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    /// Output shape
    pub format: OutputFormat,

    /// Only findings carrying this code, e.g. RUST_PUBLIC_MISSING_EXAMPLE
    #[arg(long)]
    pub code: Option<String>,

    /// Only findings under this repository-relative path prefix
    #[arg(long)]
    pub path: Option<String>,

    /// Print the counts and leave the findings out
    #[arg(long)]
    pub summary: bool,

    /// Show the findings the baseline already accepts, which are left out by default
    #[arg(long)]
    pub include_baselined: bool,

    /// Record today's findings as the accepted baseline, so the debt can shrink and cannot grow
    #[arg(long)]
    pub write_baseline: bool,
}

#[derive(Debug, Args)]
/// `majordomus run`. One capability, run as an execution in this process, followed to its
/// end.
///
/// It runs here rather than on the shared server because following it is the point: the
/// steps and the progress are printed as the handler reports them, over a subscription to
/// this process's own store rather than by asking anything repeatedly. The capability, the
/// executor and the events are the same ones a browser sees; only the audience differs.
pub struct RunArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    /// The capability to run, by its canonical id (`health.report`, `objects.verify`)
    pub capability: String,

    /// Its input, as one JSON object; the capability's input schema is what validates it
    #[arg(long, value_name = "JSON")]
    pub input: Option<String>,

    /// Print the events as they arrive on stderr; on by default when stderr is a terminal
    #[arg(long)]
    pub follow: bool,

    /// Print nothing but the final output
    #[arg(long)]
    pub quiet: bool,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Args)]
/// `majordomus executions`. What the server serving this repository has run.
///
/// An execution lives in the process that accepted it, so these read the shared server
/// this repository's lease names, and fall back to this process — which, in a one-shot
/// command, has run nothing — when no server answers.
pub struct ExecutionsArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// `list`, `show`, `events`, `cancel` or `protocol`; none lists.
    pub command: Option<ExecutionsCommand>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus executions`.
pub enum ExecutionsCommand {
    /// Every execution the server remembers, newest first
    List {
        /// Only executions in this state (queued, running, cancelling, succeeded, failed, cancelled)
        #[arg(long)]
        state: Option<String>,
        /// Only executions of this capability
        #[arg(long)]
        capability: Option<String>,
    },
    /// One execution in full: its state, its steps, its diagnostics and what it produced
    Show {
        /// The execution's id
        id: String,
    },
    /// One execution's retained events, oldest first
    Events {
        /// The execution's id
        id: String,
        /// Only events after this sequence number
        #[arg(long)]
        after: Option<u64>,
    },
    /// Ask an execution to stop
    Cancel {
        /// The execution's id
        id: String,
    },
    /// The live channel's contract: where it is, what it writes, and the schema of each message
    Protocol,
}

#[derive(Debug, Args)]
/// `majordomus evidence`. The output shape is global, so it reads the way a person writes
/// it — `evidence show --findings --format json` — and is declared once.
///
/// ```
/// use clap::Parser;
/// use majordomus_cli::cli::{Cli, Command, EvidenceArgs, EvidenceCommand, OutputFormat};
///
/// let cli = Cli::try_parse_from([
///     "majordomus", "evidence", "show", "--findings", "--format", "json",
/// ])
/// .unwrap();
/// let args: EvidenceArgs = match cli.command {
///     Command::Evidence(args) => args,
///     other => panic!("expected `evidence`, parsed {other:?}"),
/// };
/// // `--format` is declared once and reaches every subcommand, so it parses where a
/// // person writes it rather than only before the subcommand
/// assert!(matches!(args.format, OutputFormat::Json));
/// assert!(matches!(args.command, EvidenceCommand::Show { findings: true, .. }));
///
/// // the group runs nothing of its own: every runnable path here is a capability's
/// assert!(Cli::try_parse_from(["majordomus", "evidence"]).is_err());
/// ```
pub struct EvidenceArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// `show`, `claim`, `proves` or `record`. Required: the group runs nothing of its own,
    /// so that every runnable path here is one a capability declares
    /// (`.ai/repo/projection-baseline.txt` may only shrink).
    pub command: EvidenceCommand,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus evidence`: the whole matrix, one claim, one test, and
/// the recorder.
///
/// `show`, `claim` and `proves` are the command line of `evidence.report`,
/// `evidence.claim` and `evidence.test`; `record` is the command line of
/// `evidence.record`, which is a command line and nothing else because it writes a tracked
/// file and this server is read-only. The capability is `evidence.test` and the command is
/// `proves` — `test` is a word the fish completion adapter refuses, so the command line
/// spells the relation with the verb rather than renaming the identity.
///
/// ```
/// use clap::Parser;
/// use majordomus_cli::cli::{Cli, Command, EvidenceCommand};
///
/// fn parse(args: &[&str]) -> EvidenceCommand {
///     let cli = Cli::try_parse_from(args.iter().copied()).unwrap();
///     let Command::Evidence(args) = cli.command else { panic!("evidence") };
///     args.command
/// }
///
/// assert!(matches!(
///     parse(&["majordomus", "evidence", "show", "--state", "stale"]),
///     EvidenceCommand::Show { state: Some(s), check: false, .. } if s == "stale"
/// ));
/// assert!(matches!(
///     parse(&["majordomus", "evidence", "claim", "evidence-both-directions"]),
///     EvidenceCommand::Claim { id } if id == "evidence-both-directions"
/// ));
/// assert!(matches!(
///     parse(&["majordomus", "evidence", "proves", "crate:why"]),
///     EvidenceCommand::Proves { id } if id == "crate:why"
/// ));
/// assert!(Cli::try_parse_from(["majordomus", "evidence", "test", "crate:why"]).is_err());
///
/// // recording reads a report a runner already wrote; it runs no test
/// assert!(matches!(
///     parse(&["majordomus", "evidence", "record", "--suite", "tmp/report.tsv"]),
///     EvidenceCommand::Record { suite: Some(p), origin: None, .. } if p.ends_with("report.tsv")
/// ));
/// ```
pub enum EvidenceCommand {
    /// Every claim against the evidence recorded for it
    Show {
        /// Only claims in this proof state (proven, inputs_unchanged, stale, failing, not_run, unrunnable, no_test)
        #[arg(long)]
        state: Option<String>,
        /// Only claims declaring this status (guaranteed, advisory, planned, rejected)
        #[arg(long)]
        status: Option<String>,
        /// Only the claims whose declared status the evidence does not support
        #[arg(long)]
        findings: bool,
        /// Exit 10 when a claim declares a guarantee the evidence does not support
        #[arg(long)]
        check: bool,
    },
    /// One claim: its proof state, the execution behind it, and how to reproduce it
    Claim {
        /// The claim id, as docs/CLAIMS.yaml spells it
        id: String,
    },
    /// One test: its latest execution and every claim it proves
    Proves {
        /// `suite:<case>`, `crate:<binary>`, or the path a claim names it with
        id: String,
    },
    /// Record a run that happened into the ledger
    Record {
        /// The runner's TSV report (`MJ_TEST_REPORT=<file> bash test/run.sh`)
        #[arg(long)]
        suite: Option<PathBuf>,
        /// A file holding `cargo test`'s output, for the crate's own integration tests
        #[arg(long)]
        crate_output: Option<PathBuf>,
        /// Where the run happened: local (the default), ci or release
        #[arg(long)]
        origin: Option<String>,
    },
}

#[derive(Debug, Args)]
/// `majordomus rules`. The rule corpus against the proof there is for it: what each rule
/// names, whether it is in the tree, whether a runner drives it, whether anything ran, and
/// whether what ran is older than what it is about.
///
/// Distinct from `majordomus doctrine`, which asks whether the repository satisfies a rule
/// right now. That is a question about the tree; this is a question about the rule.
/// # Example
///
/// ```
/// use majordomus_cli::cli::{Cli, Command, RulesArgs};
/// use clap::Parser;
/// let cli = Cli::try_parse_from(["majordomus", "rules", "report", "--findings"]).unwrap();
/// let Command::Rules(args) = cli.command else { panic!("not the rules command") };
/// let _: RulesArgs = args;
/// ```
pub struct RulesArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// `report`, `show` or `proves`. Required: the group runs nothing of its own, so that
    /// every runnable path here is one a capability declares
    /// (`.ai/repo/projection-baseline.txt` may only shrink).
    pub command: RulesCommand,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus rules`.
/// # Example
///
/// ```
/// use majordomus_cli::cli::{Cli, Command, RulesCommand};
/// use clap::Parser;
/// let cli = Cli::try_parse_from(["majordomus", "rules", "show", "project.x"]).unwrap();
/// let Command::Rules(args) = cli.command else { panic!("not the rules command") };
/// assert!(matches!(args.command, RulesCommand::Show { .. }));
/// ```
pub enum RulesCommand {
    /// Every rule against the proof there is for it
    Report {
        /// Only rules in this proof state (proven, inputs_unchanged, stale, gated, failing, not_run, reviewed, unrunnable, dangling, unproven)
        #[arg(long)]
        state: Option<String>,
        /// Only rules of this class (blocking, advisory)
        #[arg(long)]
        class: Option<String>,
        /// Only rules of this namespace (project, majordomus)
        #[arg(long)]
        namespace: Option<String>,
        /// Only the rules whose declared class the proof does not support
        #[arg(long)]
        findings: bool,
        /// Exit 10 when a rule declares a class the proof does not support
        #[arg(long)]
        check: bool,
    },
    /// One rule: what proves it, what it depends on, and what is missing
    Show {
        /// The rule id, with or without its version
        id: String,
    },
    /// One test: every rule it proves, and the rules that would be left with none
    Proves {
        /// `suite:<case>`, `crate:<binary>`, or the path a rule names it with
        id: String,
    },
}

#[derive(Debug, Args)]
/// `majordomus why`. The facets and the output shape are global, so they read the way a
/// person writes them — `why list --audience solo-builder` — and are declared once.
pub struct WhyArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// `list`, `show`, `audiences`, `areas`, `diagnose` or `validate`; none lists.
    pub command: Option<WhyCommand>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,

    /// Only moments this audience recognises
    #[arg(long, global = true)]
    pub audience: Option<String>,
    /// Only moments in this operational area
    #[arg(long, global = true)]
    pub area: Option<String>,
    /// Only moments carrying this tag
    #[arg(long, global = true)]
    pub tag: Option<String>,
    /// Only moments of this severity
    #[arg(long, global = true)]
    pub severity: Option<String>,
    /// Only moments of this frequency
    #[arg(long, global = true)]
    pub frequency: Option<String>,
    /// Only moments at this stage of work
    #[arg(long, global = true)]
    pub lifecycle: Option<String>,
    /// Only moments naming this capability of the executable
    #[arg(long, global = true)]
    pub capability: Option<String>,
    /// Only moments naming this command
    #[arg(long = "names-command", global = true)]
    pub names_command: Option<String>,
    /// Only the moments the homepage features
    #[arg(long, global = true)]
    pub featured: bool,
    /// Include drafts and deprecated moments, not only the public ones
    #[arg(long, global = true)]
    pub all: bool,
    /// Case-insensitive text over identities, titles, hooks, summaries, tags, aliases, signals, examples and bodies
    #[arg(long, short = 'q', global = true)]
    pub query: Option<String>,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus why`.
pub enum WhyCommand {
    /// Every operational moment, narrowed by any facet the catalogue reports
    List,
    /// One moment in full, with every relation derived from its metadata
    Show {
        /// The moment's id, which is also its slug and its route
        id: String,
    },
    /// Every audience, with the moments that name it
    Audiences,
    /// Every operational area, with the moments that fall under it
    Areas,
    /// What the symptoms you recognise imply: the areas they weigh towards and the mechanisms that answer them
    Diagnose {
        /// A signal id or a moment id; repeat for each one you recognise. Without any, the questionnaire is printed.
        #[arg(long = "signal")]
        signals: Vec<String>,
    },
    /// Every finding over the catalogue; exit 10 when any is an error
    Validate,
}

#[derive(Debug, Args)]
/// `majordomus devtask`. The output shape is global, so a person writes it where it reads
/// naturally — `devtask issue I0901 --format json` — and it is declared once.
pub struct DevtaskArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// `issue` or `milestone`.
    pub command: DevtaskCommand,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus devtask`.
pub enum DevtaskCommand {
    /// One issue as an executable development task, every field carrying where it came from
    Issue {
        /// The issue id, as the canonical model spells it. An id the model does not declare is answered, not refused.
        id: String,
        /// Answer from the canonical records alone, without consulting git — a deterministic answer that is the same on every machine
        #[arg(long = "no-git")]
        no_git: bool,
    },
    /// One milestone as an executable dependency graph: ready, blocked, parallelizable, critical blockers, cycles
    Milestone {
        /// The milestone id, as the canonical model spells it
        id: String,
    },
}

#[derive(Debug, Args)]
/// `majordomus distribution`.
pub struct DistributionArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// What to ask of the model; none shows it.
    pub command: Option<DistributionCommand>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus distribution`.
pub enum DistributionCommand {
    /// The model: the install command, where an installation goes, and every declared target
    Show,
    /// Whether the advertised one-line installation works right now, and what is missing when it does not
    Status,
    /// Every invariant of the model and of the release records; exit 10 with each violation named
    Validate,
    /// Every declared target, one line each, with the artifact name it derives
    Targets,
    /// The release build matrix, as the release workflow reads it
    Matrix,
    /// The archive name and root directory a target and a tag derive
    Artifact {
        /// A target's id or its Rust target triple
        #[arg(long, value_name = "TARGET")]
        target: String,
        /// The tag, `v` and a version
        #[arg(long, value_name = "TAG")]
        tag: String,
    },
    /// Every recorded release, newest first, and the one an unpinned installation resolves to
    Releases,
    /// The public metadata one release record publishes, rendered from the record alone
    Metadata {
        /// A release record; the file the release pipeline writes under .ai/repo/releases/
        #[arg(long, value_name = "FILE")]
        record: PathBuf,
    },
    /// What this executable is: version, target triple, profile, commit
    Build,
}

#[derive(Debug, Args)]
/// `majordomus web`.
pub struct WebArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// What to do with the topology; none lists it.
    pub command: Option<WebCommand>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,

    /// Only these surfaces, by discovered id (repeat or separate with commas)
    #[arg(long, value_delimiter = ',', global = true)]
    pub only: Vec<String>,

    /// Every surface except these, by discovered id
    #[arg(long, value_delimiter = ',', global = true)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Subcommand)]
/// The reports this executable can render. Each reads evidence a run already produced and
/// writes a surface; none of them runs anything or decides what passed.
pub enum ReportCommand {
    /// The test run: the behavioural cases' report, and the crate's own totals
    Tests {
        /// The runner's TSV report (`MJ_TEST_REPORT=<file> bash test/run.sh`)
        #[arg(long)]
        suite: PathBuf,
        /// The output of `cargo test`, for its totals
        #[arg(long)]
        crate_output: Option<PathBuf>,
    },
    /// The benchmark run: a results document, or the accepted baseline
    Benchmarks {
        /// A results document from `majordomus bench`, or a baseline under the layer
        #[arg(long)]
        from: PathBuf,
    },
    /// The UI conformance audit, rendered as a section of the test surface (/tests/ui)
    Ui {
        /// A results document from `scripts/ui audit`
        #[arg(long)]
        from: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus web`.
pub enum WebCommand {
    /// Every discovered surface: id, kind, mount, producer
    List,
    /// Why each surface exists and where each of its values came from
    Explain {
        /// Only this surface; none explains every one
        id: Option<String>,
    },
    /// Check the topology's invariants; exit 10 on any error finding
    Validate {
        /// Also require every static surface's directory and index to exist
        #[arg(long)]
        artifacts: bool,
    },
    /// Write the resolved topology to the generated manifest
    Manifest,
    /// Render a generated report into its own surface under the generated web root
    Report {
        #[command(subcommand)]
        /// Which report to render.
        report: ReportCommand,
    },
    /// Compose every published surface into one publishable tree
    Compose {
        /// Where to write it; the default is target/site
        #[arg(long)]
        destination: Option<String>,
    },
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
#[derive(Debug, Args, Default, Clone)]
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
/// `majordomus worktree` (alias `wt`). The output shape is global, so it reads the way a
/// person writes it — `worktree list --format json` — and is declared once.
pub struct WorktreeArgs {
    #[command(flatten)]
    /// Where the repository is found. The index is not built: nothing about the topology is in it.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// The subcommand; none is `status`.
    pub command: Option<WorktreeCommand>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus worktree`.
pub enum WorktreeCommand {
    /// Where this call is — branch, worktree, canonical or not, uncommitted work — and how many errors the whole topology carries; exit 10 when this worktree is out of place
    Status,
    /// Every registered worktree with its standing, one line each; exit 10 when the topology has an error
    List,
    /// The whole topology: repository, container, trunk, every worktree, every branch without a worktree, every diagnostic; exit 10 when it has an error
    Topology,
    /// Print the container every linked worktree belongs under, and nothing else: `cd "$(majordomus worktree root)"`
    Root,
    /// Print the canonical path of a branch, and nothing else: `cd "$(majordomus worktree path feature/x)"`. Derived from the name; the branch need not exist
    Path {
        /// The branch, full name
        branch: String,
    },
    /// One branch: its canonical path, whether it exists, what occupies the path, the worktree holding it, and what stands in the way
    Inspect {
        /// The branch, full name
        branch: String,
    },
    /// Create the canonical worktree of a branch, creating the branch from --base (default: the trunk) when it does not exist. The path is derived; none may be given
    Create {
        /// The branch, full name (`feature/improve-cli`)
        #[arg(value_name = "BRANCH", required_unless_present = "issue")]
        branch: Option<String>,

        /// Start a new branch from this ref. Never fetched: it must resolve locally
        #[arg(long, value_name = "REF")]
        base: Option<String>,

        /// Name the branch after this issue of .ai/repo/project/issues: `feature/<id>-<slug>`, the form the topology reads the issue back from
        #[arg(long, value_name = "ID", conflicts_with = "branch")]
        issue: Option<String>,
    },
    /// The canonical worktree of a branch: created when absent, answered when present, refused when the branch is checked out somewhere else
    Ensure {
        /// The branch, full name
        branch: String,

        /// Start a new branch from this ref (default: the trunk)
        #[arg(long, value_name = "REF")]
        base: Option<String>,
    },
    /// Bring every misplaced worktree to its canonical path, dirty state included, with a fingerprint taken before and after each move; --plan shows the steps and changes nothing
    Migrate {
        /// Show the plan and change nothing
        #[arg(long)]
        plan: bool,

        /// The same as --plan
        #[arg(long)]
        dry_run: bool,

        /// When a move crosses filesystems, copy the tree, repair git's link, verify the copy against a manifest of every entry, and only then remove the original
        #[arg(long)]
        allow_copy: bool,

        /// Only these branches
        #[arg(long, value_name = "BRANCH")]
        only: Vec<String>,

        /// Also move the scratch checkouts of sessions (under the temporary directory or .claude/worktrees), which are otherwise reported and left alone
        #[arg(long)]
        include_ephemeral: bool,
    },
    /// Every error of the topology, and nothing else; exit 10 when there is one
    Validate,
    /// Every diagnostic of the topology, errors, warnings and facts, each with its code and remedy; exit 10 when there is an error
    Doctor,
    /// May a mutation proceed from here? Exit 0 in a canonical worktree, in the primary checkout on the trunk, or detached; exit 10 with the reason otherwise. What the pre-commit hook asks
    Guard {
        /// Print nothing on success
        #[arg(long, short)]
        quiet: bool,
    },
    /// Drop git's registrations of worktrees whose directories are gone, and repair the administrative links of the ones that exist. Deletes no directory
    Repair {
        /// Report what would be dropped and change nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// Remove one linked worktree by branch or path. Never the primary checkout, never a branch, never uncommitted work without --force
    Remove {
        /// An exact branch name or an exact path
        selector: String,

        /// Remove it even though it holds uncommitted work or is locked
        #[arg(long)]
        force: bool,
    },
    /// The branches merged into the trunk whose worktree is clean or absent: what could be removed. Removes nothing
    Cleanup,
    /// Every local branch, one per line, for a shell completion that wants the live set
    Branches {
        /// Only branches with no worktree
        #[arg(long)]
        without_worktree: bool,
    },
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

    #[command(subcommand)]
    /// `status`, `ensure` or `stop`; none serves.
    pub command: Option<ServeCommand>,

    /// Interface to bind; loopback unless you say otherwise
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to bind; 0 picks a free one and the address is logged on stderr
    #[arg(long, default_value_t = DEFAULT_PORT)]
    pub port: u16,

    /// When the port is taken, bind a free one instead and log both; without this a taken port is an error
    #[arg(long)]
    pub fallback: bool,

    /// Stop when no peer has been attached for this many seconds; 0 runs until stopped. What a server no client owns is started with
    #[arg(long, value_name = "SECONDS", default_value_t = 0)]
    pub idle: u64,

    /// Bind the address this deployment object declares (`.ai/repo/deployments/<ID>.yaml`)
    /// instead of the local default. What a hosted process is started with; the address is
    /// the object's, not this command line's
    #[arg(long, value_name = "ID", conflicts_with_all = ["host", "port"])]
    pub deployment: Option<String>,
}

/// How long a server started by `serve ensure` outlives its last peer, in seconds: long
/// enough that an agent's next attach finds it, short enough that a checkout nobody works
/// in does not keep a process.
pub const DEFAULT_IDLE_SECONDS: u64 = 900;

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus serve`: the server's lifecycle as something a person or a
/// hook converges on rather than remembers. `status` is the projection of `server.status`;
/// `ensure` and `stop` are process lifecycle, which no capability served by the process
/// could be.
///
/// ```
/// use clap::Parser;
/// use majordomus_cli::capability::builtin::Checkouts;
/// use majordomus_cli::cli::{Cli, Command, ServeCommand, DEFAULT_IDLE_SECONDS};
///
/// let cli = Cli::try_parse_from(["majordomus", "serve", "ensure", "--idle", "5"]).unwrap();
/// let Command::Serve(args) = cli.command else { panic!("serve") };
/// assert!(matches!(args.command, Some(ServeCommand::Ensure { idle: 5, .. })));
///
/// // without a subcommand, `serve` serves; the defaults are the documented ones
/// let cli = Cli::try_parse_from(["majordomus", "serve", "ensure"]).unwrap();
/// let Command::Serve(args) = cli.command else { panic!("serve") };
/// assert!(matches!(args.command, Some(ServeCommand::Ensure { idle, .. }) if idle == DEFAULT_IDLE_SECONDS));
/// let cli = Cli::try_parse_from(["majordomus", "serve"]).unwrap();
/// let Command::Serve(args) = cli.command else { panic!("serve") };
/// assert!(args.command.is_none() && args.idle == 0 && !args.fallback);
///
/// // `status` asks the capability's own question, and the flag is that input's field:
/// // saying nothing asks about every checkout of the repository, as it always did
/// let cli = Cli::try_parse_from(["majordomus", "serve", "status"]).unwrap();
/// let Command::Serve(args) = cli.command else { panic!("serve") };
/// assert!(matches!(args.command, Some(ServeCommand::Status { checkouts: Checkouts::Repository, .. })));
/// let cli = Cli::try_parse_from(["majordomus", "serve", "status", "--checkouts", "this"]).unwrap();
/// let Command::Serve(args) = cli.command else { panic!("serve") };
/// assert!(matches!(args.command, Some(ServeCommand::Status { checkouts: Checkouts::This, .. })));
/// ```
pub enum ServeCommand {
    /// Where this checkout's server stands — absent, starting, ready, outdated or stale — and every server of the repository
    Status {
        /// Which checkouts to answer for: every checkout of the repository, or this one
        /// alone — which reads no other checkout's lease and probes no other server
        #[arg(long, value_enum, default_value_t = Checkouts::default())]
        checkouts: Checkouts,
        /// Output shape
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    /// Make sure a ready server serves this checkout: start one when there is none or the lease is stale, wait for one that is starting, and report where it stands
    Ensure {
        /// The port the started server asks for first; a taken one is replaced by a free one
        #[arg(long, default_value_t = DEFAULT_PORT)]
        port: u16,
        /// The started server stops when no peer has been attached for this many seconds
        #[arg(long, value_name = "SECONDS", default_value_t = DEFAULT_IDLE_SECONDS)]
        idle: u64,
        /// How long to wait for a server to become ready before reporting what stands
        #[arg(long, value_name = "SECONDS", default_value_t = 20)]
        wait: u64,
        /// Output shape
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    /// Stop this checkout's server — the one its lease names, when it answers for this checkout — and wait for the lease to go
    Stop {
        /// How long to wait for the server to end
        #[arg(long, value_name = "SECONDS", default_value_t = 10)]
        wait: u64,
    },
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
    /// Where each capability is projected, and every claim its surface does not answer
    Projections {
        /// Only capabilities composed in this module
        #[arg(long)]
        module: Option<String>,
        /// Only the capabilities whose declared exposures are not all answered
        #[arg(long)]
        unmet: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        /// Output shape.
        format: OutputFormat,
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
    /// `docs/generated/openapi.{json,yaml}`.
    Openapi,
    /// `docs/generated/capabilities.md`, `docs/generated/modules/<id>.md` and
    /// `docs/generated/cli.{md,json,yaml}`.
    Docs,
    /// `docs/generated/benchmarks.{md,json,yaml}`: every benchmark target and the coverage
    Benchmarks,
    /// `docs/generated/registry.{json,yaml}`: the builtin registry as data
    Registry,
    /// The shell tool's allow-lists under share/allow, derived from the schemas
    Allow,
    /// The provider bootstraps the policy declares (AGENTS.md, CLAUDE.md, ...)
    Providers,
    /// site/data/registry/registry.json, the registry dataset the site renders
    Site,
    /// docs/generated/artifacts.{json,yaml,md}: the index of every generated artifact
    Manifest,
    /// The installer, the installation guide, the release build matrix and the public
    /// release metadata, from share/distribution.yaml and .ai/repo/releases/
    Distribution,
    /// `docs/generated/web.json`: the resolved web topology the site's route reference renders
    Web,
    /// `docs/generated/changelog.{json,yaml,md}`: the changelog composed from the layer's
    /// release records, its decisions and the repository's commits
    Changelog,
    /// deploy/Dockerfile, .dockerignore and fly.toml, from the deployment objects
    Deployment,
    /// docs/generated/graph.json and its schema: the composed graph as data
    Graph,
    /// The design system's projections, from share/design/tokens.yaml: the stylesheets
    /// both Tailwind builds import, the tokens and the declaration compiled into the crate,
    /// every copy of the brand, site/data/registry/design.json and docs/generated/design.*
    Design,
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

#[derive(Debug, Args)]
/// `majordomus commands`.
pub struct CommandsArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// What to ask of the graph; none lists it.
    pub command: Option<CommandsCommand>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// What `majordomus commands` can be asked.
pub enum CommandsCommand {
    /// Every command, one line each: what it is, what running it changes, and where it is projected
    List(CommandsListArgs),
    /// One command in full: its arguments, its effect, what it needs, and every surface that carries it
    Show(CommandsShowArgs),
    /// Why one command appears where it does: the declaration it came from, the policy that placed it, and the reason for every surface that withholds it
    Explain(CommandsShowArgs),
    /// The whole graph as one document, with its fingerprint and every diagnostic
    Graph(CommandsGraphArgs),
    /// Materialise the workflow bridge from the graph, and refresh the cache the completion reads; writes nothing when the graph has not changed
    Bridge(CommandsBridgeArgs),
}

#[derive(Debug, Args)]
/// `majordomus commands list`.
pub struct CommandsListArgs {
    /// Only the commands of this program
    #[arg(long, value_enum)]
    pub origin: Option<CommandOrigin>,

    /// Only the commands whose effect is at most this
    #[arg(long, value_enum)]
    pub effect: Option<CommandEffect>,

    /// Only the commands matching this text, in their invocation, summary, tags or identity
    #[arg(long, value_name = "TEXT")]
    pub search: Option<String>,
}

#[derive(Debug, Args)]
/// `majordomus commands show` and `explain`.
pub struct CommandsShowArgs {
    /// The command's identity, `executable.worktree.status`
    pub id: String,
}

#[derive(Debug, Args)]
/// `majordomus commands graph`.
pub struct CommandsGraphArgs {
    /// Exit 10 when the graph carries an error
    #[arg(long)]
    pub check: bool,
}

#[derive(Debug, Args)]
/// `majordomus commands bridge`.
pub struct CommandsBridgeArgs {
    /// Exit 10 when the materialised bridge is not the one this graph projects; write nothing
    #[arg(long)]
    pub check: bool,
}

/// Which program a command belongs to, as a filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CommandOrigin {
    /// This executable
    Executable,
    /// The shell tool, bin/majordomus
    Tool,
    /// A workflow the repository declares
    Workflow,
}

/// What running a command changes, as a filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CommandEffect {
    /// Reads and answers
    ReadOnly,
    /// Writes only what no commit carries
    LocalMutation,
    /// Writes tracked files
    RepositoryMutation,
    /// Reaches the network with an effect
    NetworkMutation,
    /// Removes something
    Destructive,
}

#[derive(Debug, Args)]
/// `majordomus completion`.
pub struct CompletionArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// What to do; none prints the integration for the current shell.
    pub command: Option<CompletionCommand>,
}

#[derive(Debug, Subcommand)]
/// What `majordomus completion` can be asked.
pub enum CompletionCommand {
    /// The candidates for one command line, from the command graph. What a shell adapter calls on every TAB
    Query(CompletionQueryArgs),
    /// The shell integration to load once, which carries no command of its own and asks this executable for every candidate
    Init(CompletionInitArgs),
    /// Put that integration into the shell's startup file, between managed markers, so that no one maintains it by hand
    Install(CompletionInstallArgs),
}

#[derive(Debug, Args)]
/// `majordomus completion query`.
pub struct CompletionQueryArgs {
    /// Which surface the words are spelled for
    #[arg(long, value_enum, default_value_t = CompletionSurface::Cli)]
    pub surface: CompletionSurface,

    /// The index of the word the cursor is in; the default is a new word after the last
    #[arg(long, value_name = "N")]
    pub cursor: Option<usize>,

    /// Output shape
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub format: OutputFormat,

    /// The words of the command line, the program's own name first
    #[arg(trailing_var_arg = true, value_name = "WORD")]
    pub words: Vec<String>,
}

#[derive(Debug, Args)]
/// `majordomus completion init`.
pub struct CompletionInitArgs {
    /// Which shell to print the integration for
    #[arg(long, value_enum, default_value_t = CompletionShell::Zsh)]
    pub shell: CompletionShell,
}

/// The arguments of `completion install`.
#[derive(Debug, clap::Args)]
pub struct CompletionInstallArgs {
    /// Which shell to install for; decides the startup file when --rc is not given
    #[arg(long, value_enum, default_value_t = CompletionShell::Zsh)]
    pub shell: CompletionShell,
    /// The startup file to write, instead of the shell's usual one
    #[arg(long, value_name = "PATH")]
    pub rc: Option<std::path::PathBuf>,
    /// Take the block out again, leaving the rest of the file as it was
    #[arg(long)]
    pub remove: bool,
    /// Say what would change and write nothing
    #[arg(long)]
    pub dry_run: bool,
}

/// The surface a completion request is spelled for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CompletionSurface {
    /// The command line of either program
    Cli,
    /// The workflow runner
    Workflow,
}

/// A shell the integration is printed for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CompletionShell {
    /// zsh
    Zsh,
    /// bash
    Bash,
    /// fish
    Fish,
}

#[derive(Debug, Args)]
/// `majordomus devcontext`. Every subcommand is the projection of one `devcontext.*`
/// capability; the request flags are declared once and shared by `compile` and `explain`.
pub struct DevcontextArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// `compile`, `explain` or `policy`.
    pub command: DevcontextCommand,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// What to ask of the context compiler.
pub enum DevcontextCommand {
    /// Compile the context for a piece of work: every selected object with its provenance, the reason and the confidence, everything left out with the reason, what was deduplicated, and the per-tier budget; exit 10 when what may not be dropped already exceeds the budget
    Compile(DevcontextRequest),
    /// Why one canonical identifier is or is not in the context a request compiles to
    Explain {
        /// The canonical identifier, `majordomus://<kind>/<identity>`
        uri: String,
        #[command(flatten)]
        /// The request to judge it under.
        request: DevcontextRequest,
    },
    /// The compiler's own rules: the tiers, every edge of the composed graph and what is done with it, the selectors, the defaults
    Policy,
}

#[derive(Debug, Clone, Args)]
/// What to compile a context about; every flag is optional.
pub struct DevcontextRequest {
    /// An issue id (`I0301`) or its canonical identifier
    #[arg(long)]
    pub issue: Option<String>,
    /// A milestone id or slug, or its canonical identifier
    #[arg(long)]
    pub milestone: Option<String>,
    /// What the session is trying to do, in words; the only input the compiler infers from
    #[arg(long)]
    pub intent: Option<String>,
    /// A repository-relative path the work touches; repeat for each
    #[arg(long = "path")]
    pub paths: Vec<String>,
    /// A canonical identifier to seed with directly; repeat for each
    #[arg(long = "uri")]
    pub uris: Vec<String>,
    /// The ceiling in estimated tokens
    #[arg(long)]
    pub budget_tokens: Option<u64>,
    /// How far from a seed the walk goes
    #[arg(long)]
    pub max_depth: Option<usize>,
    /// Relevance below which an entry is reported rather than given, between 0 and 1
    #[arg(long)]
    pub floor: Option<f64>,
    /// Every blocking rule of the layer, not only the ones the work reaches
    #[arg(long)]
    pub all_blocking_rules: bool,
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

pub mod docs;
pub mod local;
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
        command: "product",
        examples: &[ExampleDoc {
            id: "product-default-list",
            title: "What the product does, as the layer declares it",
            description: "`product` with nothing after it lists the features, because listing is what a person wants when they ask what the tool is for. Every column is derived: the surfaces a feature is exposed through come from the modules, commands and kinds it names, never from the file.",
            argv: &["product"],
            setup: &[],
            expect: Expect::StdoutContains(&["SLUG", "SURFACES", "feature(s)"]),
        }],
    },
    CommandExamples {
        command: "product list",
        examples: &[
            ExampleDoc {
                id: "product-list",
                title: "Every stable feature, in presentation order",
                description: "Drafts are excluded unless `--all` is given; `--featured` narrows to the features the homepage shows. The filters are the facets the model derives — an area, a module, a command, a surface — so a module added to the executable is a filter without anything being registered.",
                argv: &["product", "list"],
                setup: &[],
                expect: Expect::StdoutContains(&["SLUG", "fixture-feature"]),
            },
            ExampleDoc {
                id: "product-list-json",
                title: "The same, as the shape the API and MCP answer with",
                description: "One domain model behind every projection: this document is what `GET /api/v1/product/features` returns and what the `majordomus_features` tool answers, with the counts, the fingerprint and the surfaces of every feature.",
                argv: &["product", "list", "--format", "json"],
                setup: &[],
                expect: Expect::Json(&["/counts/features", "/features/0/surfaces", "/fingerprint"]),
            },
        ],
    },
    CommandExamples {
        command: "product show",
        examples: &[ExampleDoc {
            id: "product-show",
            title: "One feature, with everything derived from what it names",
            description: "The record as its file declares it, then what nobody authored: the capabilities of its modules with their tools and routes, the commands with their summaries, the objects of its kinds counted, the rules with their class, the documents, the decisions, the claims with their status, the moments it answers, and the interfaces all of that adds up to.",
            argv: &["product", "show", "fixture-feature"],
            setup: &[],
            expect: Expect::StdoutContains(&["fixture-feature", "surfaces", "derived"]),
        }],
    },
    CommandExamples {
        command: "product matrix",
        examples: &[ExampleDoc {
            id: "product-matrix",
            title: "Every feature against every interface, and what no feature names",
            description: "One row per feature with a mark per surface, then every module of the executable, every public command and every kind of the layer with the features that name it. A row with no feature is a gap the product page cannot hide.",
            argv: &["product", "matrix"],
            setup: &[],
            expect: Expect::StdoutContains(&["FEATURE", "cli", "MODULE"]),
        }],
    },
    CommandExamples {
        command: "product providers",
        examples: &[ExampleDoc {
            id: "product-providers",
            title: "Every provider the tool has an adapter for",
            description: "One line per template the distribution ships, with the bootstraps this repository's policy renders through it, the client configuration it carries for the shared MCP server, and the hooks the policy wires. The set is the templates; nothing here is a list of vendors.",
            argv: &["product", "providers"],
            setup: &[],
            expect: Expect::StdoutContains(&["PROVIDER", "agents"]),
        }],
    },
    CommandExamples {
        command: "product validate",
        examples: &[ExampleDoc {
            id: "product-validate",
            title: "Check the model before anything projects it",
            description: "A reference that resolves to nothing, with the nearest candidate; a duplicate identity; a file name that disagrees with its id; a draft that is featured; a stable feature under its floors; and every module, command or kind no feature names. Exit 10 on any error.",
            argv: &["product", "validate"],
            setup: &[],
            expect: Expect::StdoutContains(&["feature(s)", "valid"]),
        }],
    },
    CommandExamples {
        command: "release",
        examples: &[ExampleDoc {
            id: "release-changelog",
            title: "What has shipped, and what has not",
            description: "`release` with nothing after it renders the changelog. Every line of it is derived — a section per release the layer records, its decisions the ADRs dated inside that release's window, its changes the conventional commits in its range — so there is no file anyone can forget to update.",
            argv: &["release"],
            setup: &[],
            expect: Expect::StdoutContains(&["Changelog"]),
        }],
    },
    CommandExamples {
        command: "release changelog",
        examples: &[ExampleDoc {
            id: "release-changelog-json",
            title: "The same document every other surface answers with",
            description: "What `GET /api/v1/changelog` returns, what the MCP resource `majordomus://changelog` carries, and what `majordomus generate changelog` writes into the reference. One value, four renderings.",
            argv: &["release", "changelog", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/schema", "/current", "/sections"]),
        }],
    },
    CommandExamples {
        command: "release version",
        examples: &[ExampleDoc {
            id: "release-version",
            title: "The version, and the one the commits imply",
            description: "The version is stated in two files for a reason the release script gives: an installed tree has no Cargo.toml and the crate is compiled before the shell tool exists, so neither can read the other at run time. This says what both state, whether they agree, and what the conventional commits since the last release imply the next one should be.",
            argv: &["release", "version", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/declared", "/agree", "/bump"]),
        }],
    },
    CommandExamples {
        command: "release bump",
        examples: &[ExampleDoc {
            id: "release-bump-dry-run",
            title: "Raising it, in both places, once",
            description: "The bump defaults to what the commits imply — a breaking change is major, a feature is minor, anything else is patch — and `--level` or `--exact` overrides that when a person means something the commits do not say. It writes both files and nothing else; `scripts/release-version --check` then proves the work of one writer rather than the memory of one person. A repository that declares no version — the example runs in one with no crate — cannot be raised, and says so with exit 12 rather than inventing a number to raise from.",
            argv: &["release", "bump", "--dry-run"],
            setup: &[],
            expect: Expect::ExitCode(12),
        }],
    },
    CommandExamples {
        command: "completion install",
        examples: &[ExampleDoc {
            id: "completion-install-dry-run",
            title: "The one line a person adds to their shell, added for them",
            description: "Writes the integration into the shell's startup file between `# >>> MAJORDOMUS >>>` markers: nothing outside them is touched, running it twice changes nothing, and `--remove` takes it out again. It is never a side effect of anything else — installing into a person's home directory is its own decision, so it is its own command. `--dry-run` says what would change and writes nothing.",
            argv: &["completion", "install", "--shell", "zsh", "--dry-run"],
            setup: &[],
            expect: Expect::Success,
        }],
    },
    CommandExamples {
        command: "env",
        examples: &[ExampleDoc {
            id: "env-status",
            title: "What this checkout is",
            description: "`env` with nothing after it resolves the whole snapshot: the project and its version, the repository and its layer, version control, the toolchains the repository declares, what the layer holds counted per kind, the workflows the runner describes, the provider projections against the policy that renders them, and the local services. This is the resolution that counts the layer, so it builds the index and writes the cache the banner reads.",
            argv: &["env"],
            setup: &[],
            expect: Expect::StdoutContains(&["project", "repository", "resolution"]),
        }],
    },
    CommandExamples {
        command: "env status",
        examples: &[ExampleDoc {
            id: "env-status-json",
            title: "The snapshot as one document",
            description: "The same value the HTTP route `/api/v1/environment` and the MCP resource `majordomus://environment` answer with, and the value the banner renders. Every field carries where it came from under `provenance`, and a value nothing could resolve is absent rather than zero.",
            argv: &["env", "status", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/schema", "/project/version", "/repository/name", "/provenance"]),
        }],
    },
    CommandExamples {
        command: "env banner",
        examples: &[ExampleDoc {
            id: "env-banner-compact",
            title: "The two-line form, at a width you choose",
            description: "What `direnv` renders on entering the repository. It resolves fast — it never builds the index — and it writes to standard error, because direnv reads the standard output of a `.envrc` as the environment it is applying. `--width` renders as if the terminal were that wide, which is what makes the layout testable.",
            argv: &["env", "banner", "--mode", "compact", "--width", "80"],
            setup: &[],
            expect: Expect::Success,
        }],
    },
    CommandExamples {
        command: "env export",
        examples: &[ExampleDoc {
            id: "env-export-direnv",
            title: "The assignments a shell in this repository wants",
            description: "Assignments and nothing else, safe to `eval`: no command runs, no file is touched, and every value is quoted so that a repository path holding a quote or a `$(...)` cannot become shell code. This is the whole of what `.envrc` needs from Majordomus.",
            argv: &["env", "export", "--shell", "direnv"],
            setup: &[],
            expect: Expect::StdoutContains(&["export MAJORDOMUS_ROOT="]),
        }],
    },
    CommandExamples {
        command: "env enter",
        examples: &[ExampleDoc {
            id: "env-enter",
            title: "Everything entering this repository is, as one call",
            description: "What the file a shell evaluates on entry runs, and the only call it makes (ADR 0043): the assignments on standard output for `eval`, the banner on standard error, the workflow bridge refreshed when a declaration behind it moved, and the repository's shared server ensured when nothing is serving this checkout. It never builds, never ensures from an executable older than its sources, never waits for a server it started to answer, and never exits non-zero — a non-zero exit here would make direnv report that the whole environment failed. `--no-runtime` is what this example passes, because an example is not the place to start a server.",
            argv: &["env", "enter", "--shell", "direnv", "--no-banner", "--no-bridge", "--no-runtime"],
            setup: &[],
            expect: Expect::StdoutContains(&["export MAJORDOMUS_ROOT="]),
        }],
    },
    CommandExamples {
        command: "env explain",
        examples: &[ExampleDoc {
            id: "env-explain-field",
            title: "Where one value came from",
            description: "An inferred system without provenance is magic. Every field of the snapshot can name the file, command or compile-time constant that decided it, the resolver that read it, and whether it was read now, taken from the cache, or not resolved at all.",
            argv: &["env", "explain", "project.version"],
            setup: &[],
            expect: Expect::StdoutContains(&["project.version", "source", "resolver"]),
        }],
    },
    CommandExamples {
        command: "run",
        examples: &[ExampleDoc {
            id: "run-demonstrate",
            title: "Watch an execution happen",
            description: "`run` starts a capability as an execution and follows it to its end: every step, every line it logs and every advance of its progress, as the handler reports them. `executions.demonstrate` exists to make that visible without waiting for real work — it reads nothing and writes nothing, and its only effect is the events it produces. The same execution, started from the Cockpit, streams the same events to a browser.",
            argv: &["run", "executions.demonstrate", "--input", "{\"steps\":2,\"delay_ms\":0}", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/state", "/id", "/output/steps", "/steps/0/name"]),
        }],
    },
    CommandExamples {
        command: "executions",
        examples: &[ExampleDoc {
            id: "executions-default-list",
            title: "What has run",
            description: "`executions` with nothing after it lists what the server serving this repository has run, newest first. In a checkout where no server is running it says so rather than pretending: an execution lives in the process that accepted it.",
            argv: &["executions"],
            setup: &[],
            expect: Expect::StdoutContains(&["execution"]),
        }],
    },
    CommandExamples {
        command: "evidence show",
        examples: &[ExampleDoc {
            id: "evidence-show-json",
            title: "The whole join, as one document",
            description: "The same answer `GET /api/v1/evidence` and the MCP tool `majordomus_evidence` return: every claim with its proof state, the sentence that explains how that state was derived, the execution behind it, the files that have changed since, and the command that produces the proof again. The tallies count the whole matrix even when the claims are filtered, so a narrowed answer never misreports how much of it was examined.",
            argv: &["evidence", "show", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/claims", "/totals", "/ledger/path", "/findings"]),
        }],
    },
    CommandExamples {
        command: "evidence claim",
        examples: &[ExampleDoc {
            id: "evidence-claim-absent",
            title: "A claim the matrix does not declare",
            description: "A claim id nothing declares is a not-found rather than an empty answer. A typo that read as `this claim has no evidence` is the one answer this command must never give, because it is indistinguishable from the finding the whole subsystem exists to report.",
            argv: &["evidence", "claim", "no-such-claim-exists"],
            setup: &[],
            expect: Expect::ExitCode(13),
        }],
    },
    CommandExamples {
        command: "evidence proves",
        examples: &[ExampleDoc {
            id: "evidence-proves-unknown",
            title: "Something that names no test",
            description: "A test is named by its identity (`suite:<case>`, `crate:<binary>`) or by the path a claim writes down, and the two resolve to the same thing. An argument that is neither is refused with the spellings it could have been, rather than answered with a test that proves nothing.",
            argv: &["evidence", "proves", "not-a-test"],
            setup: &[],
            expect: Expect::ExitCode(13),
        }],
    },
    CommandExamples {
        command: "rules report",
        examples: &[ExampleDoc {
            id: "rules-report-json",
            title: "Every rule against the proof there is for it",
            description: "The same answer `GET /api/v1/rules` and the MCP tool `majordomus_rules` return: per rule, its class, the mode it declares, the validator and the cases it names, whether each is in the tree, the execution behind each, the gates that run them, and the sentence explaining how the state was derived. The tallies count the whole corpus even when the rules are filtered, and `review_only` is counted apart from `passing` so that a rule a person enforces is never added to a total that reads as proof.",
            argv: &["rules", "report", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/rules", "/states", "/coverage/rules", "/findings"]),
        }],
    },
    CommandExamples {
        command: "rules show",
        examples: &[ExampleDoc {
            id: "rules-show-absent",
            title: "A rule the repository does not declare",
            description: "A rule id nothing declares is a not-found rather than an empty answer. A typo that read as `this rule has no proof` is the one answer this command must never give, because it is indistinguishable from the finding the whole subsystem exists to report.",
            argv: &["rules", "show", "project.no-such-rule"],
            setup: &[],
            expect: Expect::ExitCode(12),
        }],
    },
    CommandExamples {
        command: "rules proves",
        examples: &[ExampleDoc {
            id: "rules-proves-case",
            title: "What a case proves, and what would lose its only proof",
            description: "The reverse of `rules show`, reading the same derivation so the two directions cannot disagree. It names every rule that names this test and, separately, the rules that would be left with no proof at all if it were deleted — the question to ask before renaming a case, and the one that could not be asked while the relation ran one way only.",
            argv: &["rules", "proves", "test/cases/125_rule_proof.sh", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/proves", "/sole_proof_of", "/path"]),
        }],
    },
    CommandExamples {
        command: "evidence record",
        examples: &[ExampleDoc {
            id: "evidence-record-missing",
            title: "Recording a report that is not there",
            description: "The recorder reads what a run already wrote — the suite's TSV report, `cargo test`'s output — and stamps it with the provenance the run did not carry. A report it cannot read is refused: recording nothing would leave every claim reading `not run` after a run that ran, which is a lie in the safe direction and still a lie.",
            argv: &["evidence", "record", "--suite", "target/no-such-run.tsv"],
            setup: &[],
            expect: Expect::ExitCode(13),
        }],
    },
    CommandExamples {
        command: "executions list",
        examples: &[ExampleDoc {
            id: "executions-list-json",
            title: "Every execution, as one document",
            description: "The same answer `GET /api/v1/executions`, the MCP tool `majordomus_executions` and the Cockpit's Executions page render, with the counts beside it: how many are remembered, how many are active, how many are waiting for a worker and how many live channels are following them.",
            argv: &["executions", "list", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/count", "/active", "/queued", "/live_channels"]),
        }],
    },
    CommandExamples {
        command: "executions show",
        examples: &[ExampleDoc {
            id: "executions-show-absent",
            title: "An execution that is not there",
            description: "An execution lives in the process that accepted it and is remembered in bounded numbers, so asking for one nothing ran says so and exits with the missing-artifact code rather than inventing an empty answer. Against a running server, the same command prints that execution's state, its steps and what it produced.",
            argv: &["executions", "show", "x-20260101T120000Z-4c3b2a19"],
            setup: &[],
            expect: Expect::ExitCode(12),
        }],
    },
    CommandExamples {
        command: "executions events",
        examples: &[ExampleDoc {
            id: "executions-events-absent",
            title: "The events of an execution that is not there",
            description: "The retained events of one execution, oldest first, after a sequence number — what a reconnecting client reads before it opens the live channel. For an execution nothing ran, the same refusal as `show`.",
            argv: &["executions", "events", "x-20260101T120000Z-4c3b2a19"],
            setup: &[],
            expect: Expect::ExitCode(12),
        }],
    },
    CommandExamples {
        command: "executions cancel",
        examples: &[ExampleDoc {
            id: "executions-cancel-absent",
            title: "Asking an execution that is not there to stop",
            description: "Cancellation is cooperative: the flag is set and a task stops when it next looks at it. There is nothing to set for an execution nothing ran, and the command says so rather than reporting a success it did not have.",
            argv: &["executions", "cancel", "x-20260101T120000Z-4c3b2a19"],
            setup: &[],
            expect: Expect::ExitCode(12),
        }],
    },
    CommandExamples {
        command: "executions protocol",
        examples: &[ExampleDoc {
            id: "executions-protocol",
            title: "The live channel's contract, from the types that implement it",
            description: "Where the WebSocket is, how a subscription and a reconnect are expressed, every message type, and the JSON Schema of each — derived from the Rust types, so a client validating against this is validating against the implementation. OpenAPI cannot describe a socket, which is why this is a capability and not a paragraph.",
            argv: &["executions", "protocol", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/protocol_version", "/websocket", "/event_types/0", "/stream_types/0", "/limits/max_events"]),
        }],
    },
    CommandExamples {
        command: "worktree",
        examples: &[ExampleDoc {
            id: "worktree-default-status",
            title: "Where am I, and is that where I belong?",
            description: "`worktree` with nothing after it answers the question a worker asks before starting: which branch this is, whether this directory is that branch's canonical worktree (or the primary checkout on the trunk), what is uncommitted here, and how many errors the whole topology carries. The same answer from the primary checkout and from four directories deep inside a linked worktree.",
            argv: &["worktree"],
            setup: &[],
            expect: Expect::StdoutContains(&["branch", "worktree", "container"]),
        }],
    },
    CommandExamples {
        command: "worktree status",
        examples: &[ExampleDoc {
            id: "worktree-status-json",
            title: "The current worktree as one document",
            description: "The same answer as JSON: the repository, the container, the trunk and how it was decided, this worktree with its standing and diagnostics, and whether it is where it belongs. This is what the MCP tool `majordomus_worktree_status` and `GET /api/v1/worktrees/status` answer.",
            argv: &["worktree", "status", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/worktree/standing", "/container/path", "/trunk/source", "/canonical"]),
        }],
    },
    CommandExamples {
        command: "worktree list",
        examples: &[ExampleDoc {
            id: "worktree-list-text",
            title: "Every worktree, the misplaced ones obvious",
            description: "The primary checkout first, then every linked worktree with its standing, its branch, where it belongs when it is somewhere else, and its uncommitted work. The primary checkout is exempt from the path rule and held to the trunk rule instead.",
            argv: &["worktree", "list"],
            setup: &[&["worktree", "create", "feature/example"]],
            expect: Expect::StdoutContains(&["PRIMARY", "CANONICAL", "-wt/feature/example"]),
        }],
    },
    CommandExamples {
        command: "worktree topology",
        examples: &[ExampleDoc {
            id: "worktree-topology-json",
            title: "The whole topology as one document",
            description: "The repository, the container, the trunk, every worktree, every branch with or without a worktree, every diagnostic with its code and remedy, and the tallies. This is what the MCP resource `majordomus://worktrees`, the tool `majordomus_worktrees`, `GET /api/v1/worktrees` and the Cockpit all render.",
            argv: &["worktree", "topology", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/container/path", "/trunk/branch", "/worktrees/0/standing", "/branches/0/name", "/tallies/worktrees", "/valid"]),
        }],
    },
    CommandExamples {
        command: "worktree root",
        examples: &[ExampleDoc {
            id: "worktree-root-path",
            title: "The container, for the shell",
            description: "Prints the container and nothing else, so a shell can use it: `cd \"$(majordomus worktree root)\"`. It is the primary checkout's sibling named with `-wt`, derived from git's own identity and never from the current directory — which is why running this inside a linked worktree does not answer a container inside that worktree.",
            argv: &["worktree", "root"],
            setup: &[],
            expect: Expect::StdoutContains(&["-wt"]),
        }],
    },
    CommandExamples {
        command: "worktree path",
        examples: &[ExampleDoc {
            id: "worktree-path-branch",
            title: "The canonical path of a branch, for the shell",
            description: "A child process cannot change its parent shell's directory, so nothing here pretends to: this prints one path and the shell does the rest — `cd \"$(majordomus worktree path feature/x)\"`. The path is the branch name under the container, hierarchy kept; the branch need not exist yet.",
            argv: &["worktree", "path", "feature/providers/streaming"],
            setup: &[],
            expect: Expect::StdoutContains(&["-wt/feature/providers/streaming"]),
        }],
    },
    CommandExamples {
        command: "worktree inspect",
        examples: &[ExampleDoc {
            id: "worktree-inspect-branch",
            title: "One branch, before creating its worktree",
            description: "Where the branch's worktree belongs, whether the branch exists, whether anything occupies the path, and what would stand in the way. For a branch that does not exist yet, the answer is the path `worktree create` would use and the command to run.",
            argv: &["worktree", "inspect", "feature/new-dashboard"],
            setup: &[],
            expect: Expect::StdoutContains(&["-wt/feature/new-dashboard", "does not exist yet"]),
        }],
    },
    CommandExamples {
        command: "worktree create",
        examples: &[ExampleDoc {
            id: "worktree-create-branch",
            title: "Start work on a branch without deciding where it goes",
            description: "Creates the branch `feature/improve-cli` from the trunk and checks it out in a new worktree at `<repository>-wt/feature/improve-cli`. No path is given and none may be: the destination follows from the repository's identity and the branch name, so the same command in the same repository always produces the same path — from the primary checkout, and from inside another worktree.",
            argv: &["worktree", "create", "feature/improve-cli"],
            setup: &[],
            expect: Expect::StdoutContains(&["-wt/feature/improve-cli", "feature/improve-cli (new"]),
        }],
    },
    CommandExamples {
        command: "worktree ensure",
        examples: &[ExampleDoc {
            id: "worktree-ensure-existing",
            title: "The canonical worktree, whether or not it exists yet",
            description: "`ensure` is `create` for a caller that does not care whether the worktree is already there: it creates it when it is absent and answers the existing one when it is present. What a script or an agent runs before starting on a branch.",
            argv: &["worktree", "ensure", "feature/improve-cli"],
            setup: &[&["worktree", "create", "feature/improve-cli"]],
            expect: Expect::StdoutContains(&["exists", "-wt/feature/improve-cli"]),
        }],
    },
    CommandExamples {
        command: "worktree migrate",
        examples: &[ExampleDoc {
            id: "worktree-migrate-plan",
            title: "What it would take to bring every worktree home",
            description: "`--plan` shows each misplaced worktree with where it belongs, how it would move, the uncommitted work that moves with it, and what blocks it, and changes nothing. Without `--plan` the movable steps are carried out: each worktree is fingerprinted, moved with `git worktree move`, fingerprinted again at its new path, and reported as moved only when the two are equal.",
            argv: &["worktree", "migrate", "--plan"],
            setup: &[],
            expect: Expect::StdoutContains(&["nothing to migrate"]),
        }],
    },
    CommandExamples {
        command: "worktree validate",
        examples: &[ExampleDoc {
            id: "worktree-validate-clean",
            title: "Is the topology valid?",
            description: "Every error-level diagnostic and nothing else, then the verdict; exit 10 when there is an error. What a script gates on.",
            argv: &["worktree", "validate"],
            setup: &[],
            expect: Expect::StdoutContains(&["worktree topology: valid"]),
        }],
    },
    CommandExamples {
        command: "worktree doctor",
        examples: &[ExampleDoc {
            id: "worktree-doctor-clean",
            title: "Every diagnostic, with its code and its remedy",
            description: "Errors, warnings and facts — a misplaced worktree, a stale registration, a detached HEAD, the primary checkout off the trunk, an unknown trunk — each under a stable code the API and the Cockpit carry too, each with the command that addresses it.",
            argv: &["worktree", "doctor"],
            setup: &[],
            expect: Expect::StdoutContains(&["worktree topology: valid"]),
        }],
    },
    CommandExamples {
        command: "worktree guard",
        examples: &[ExampleDoc {
            id: "worktree-guard-ok",
            title: "May a commit proceed from here?",
            description: "The pre-commit hook's question. Exit 0 in a branch's canonical worktree, in the primary checkout on the trunk, or on a detached HEAD; exit 10 with the diagnostic and the remedy when a feature branch is being worked on somewhere it does not belong. The hook stays one line; this is the logic.",
            argv: &["worktree", "guard"],
            setup: &[],
            expect: Expect::StdoutContains(&["worktree guard: ok"]),
        }],
    },
    CommandExamples {
        command: "worktree repair",
        examples: &[ExampleDoc {
            id: "worktree-repair-dry-run",
            title: "What git would forget",
            description: "`repair` drops git's registrations of worktrees whose directories no longer exist and lets git repair the administrative links of the ones that do. It deletes no directory and touches no branch; `--dry-run` reports what it would drop and changes nothing.",
            argv: &["worktree", "repair", "--dry-run"],
            setup: &[],
            expect: Expect::StdoutContains(&["nothing to prune"]),
        }],
    },
    CommandExamples {
        command: "worktree remove",
        examples: &[ExampleDoc {
            id: "worktree-remove-clean",
            title: "Remove a worktree, and keep its branch",
            description: "Removes the worktree and nothing else. The branch it held still exists: worktree lifecycle and branch lifecycle are separate, and deleting a branch is a git command a person types deliberately. A worktree with uncommitted work is refused rather than removed.",
            argv: &["worktree", "remove", "feature/improve-cli"],
            setup: &[&["worktree", "create", "feature/improve-cli"]],
            expect: Expect::StdoutContains(&["removed", "feature/improve-cli still exists"]),
        }],
    },
    CommandExamples {
        command: "worktree cleanup",
        examples: &[ExampleDoc {
            id: "worktree-cleanup-nothing",
            title: "What could go, and what it would take",
            description: "Every branch merged into the trunk whose worktree is clean or absent, with the two commands that would remove the worktree and then the branch. Derived state only: nothing is deleted here, and a dirty or unmerged worktree is never listed.",
            argv: &["worktree", "cleanup"],
            setup: &[],
            expect: Expect::StdoutContains(&["cleanup-eligible"]),
        }],
    },
    CommandExamples {
        command: "worktree branches",
        examples: &[ExampleDoc {
            id: "worktree-branches-list",
            title: "The live branch set, for completion",
            description: "Every local branch, one per line, nothing else. A shell completion for `worktree path`, `create` or `remove` reads this rather than a list kept anywhere, so a branch created a second ago completes.",
            argv: &["worktree", "branches"],
            setup: &[],
            expect: Expect::Success,
        }],
    },
    CommandExamples {
        command: "distribution",
        examples: &[ExampleDoc {
            id: "distribution-show-default",
            title: "How this project is installed",
            description: "`distribution` with nothing after it shows the model: the one-line install command, where an installation goes, and how many platforms a release builds. Every value comes from share/distribution.yaml, which is the only place any of them is written.",
            argv: &["distribution"],
            setup: &[],
            expect: Expect::StdoutContains(&["binary", "install", "targets"]),
        }],
    },
    CommandExamples {
        command: "distribution show",
        examples: &[ExampleDoc {
            id: "distribution-show-json",
            title: "The distribution model as one JSON document",
            description: "The same answer as a document a script can read: the install command, the default locations, and every declared target with the artifact name the naming function derives for it. This is what the website's install block and the cockpit's install card render.",
            argv: &["distribution", "show", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/install_command", "/targets"]),
        }],
    },
    CommandExamples {
        command: "distribution targets",
        examples: &[ExampleDoc {
            id: "distribution-targets",
            title: "Every platform, and whether a release builds it",
            description: "One line per declared target: its id, its Rust target triple, whether it is supported, experimental or unavailable, and how it is written in prose. The supported-platform table in the documentation and the installer's own refusal message are rendered from these same rows.",
            argv: &["distribution", "targets"],
            setup: &[],
            expect: Expect::StdoutContains(&["RUST TARGET", "supported"]),
        }],
    },
    CommandExamples {
        command: "distribution validate",
        examples: &[ExampleDoc {
            id: "distribution-validate",
            title: "Every invariant of the model and of the release records",
            description: "Refuses a duplicate target id or triple, two targets deriving one artifact name, a Linux target with no C library, a published target with nothing to build it on, an unbuilt target with no recorded reason, a base URL that is not HTTPS, and a release record that misses a supported target, renames an artifact or serves one from another host. Exits 10 with each violation named.",
            argv: &["distribution", "validate"],
            setup: &[],
            expect: Expect::StdoutContains(&["distribution"]),
        }],
    },
    CommandExamples {
        command: "distribution matrix",
        examples: &[ExampleDoc {
            id: "distribution-matrix",
            title: "The release build matrix the workflow runs",
            description: "One entry per published target, with the runner it is built on, the packages that runner needs, and the artifact name with `{tag}` where a release's tag goes. The release workflow reads this and states no platform of its own; adding a target to the model adds a build here and nowhere else.",
            argv: &["distribution", "matrix"],
            setup: &[],
            expect: Expect::Json(&["/include", "/binary"]),
        }],
    },
    CommandExamples {
        command: "distribution artifact",
        examples: &[ExampleDoc {
            id: "distribution-artifact",
            title: "What one target and one tag are called",
            description: "The one naming function, asked directly: the archive's name, the directory it unpacks into, and where a release publishes it. `scripts/release-package` asks this rather than composing a name, so a change to the naming function reaches the packaging without an edit.",
            argv: &["distribution", "artifact", "--target", "aarch64-apple-darwin", "--tag", "v0.2.0", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/name", "/root", "/url"]),
        }],
    },
    CommandExamples {
        command: "distribution status",
        examples: &[ExampleDoc {
            id: "distribution-status",
            title: "Whether the published one-line installation works right now",
            description: "The operator's question — *can a machine that has never seen this project install it with the advertised command?* — answered from the distribution model and the release records, without touching the network. Each check names what was observed; a failing one names its cause and the command that changes it. Shown here in a repository that has published nothing, where the answer is no and the exit code is 10, which is what makes it usable as a check rather than as prose. `distribution validate` is the gate over the model itself; this is the gate over the state a user meets.",
            argv: &["distribution", "status"],
            setup: &[],
            expect: Expect::ExitCode(10),
        }],
    },
    CommandExamples {
        command: "distribution releases",
        examples: &[ExampleDoc {
            id: "distribution-releases",
            title: "What has been published, and what an unpinned install resolves to",
            description: "Every release record this repository holds, newest first, and which of them the stable pointer names: the highest version among the stable, unwithdrawn records. The pointer is derived on every read and is authored nowhere.",
            argv: &["distribution", "releases"],
            setup: &[],
            expect: Expect::Success,
        }],
    },
    CommandExamples {
        command: "distribution metadata",
        examples: &[ExampleDoc {
            id: "distribution-metadata",
            title: "What one release record publishes",
            description: "The public metadata a record turns into, rendered from the record and the model alone: the release pipeline prints this before it commits anything, and the installer's own tests serve it as a release that never existed. Shown here in a repository that has published nothing, where the record does not exist and the command says which file it wanted and exits 10 rather than inventing one.",
            argv: &["distribution", "metadata", "--record", ".ai/repo/releases/v0.2.0.yaml"],
            setup: &[],
            expect: Expect::ExitCode(10),
        }],
    },
    CommandExamples {
        command: "distribution build",
        examples: &[ExampleDoc {
            id: "distribution-build",
            title: "What this executable is",
            description: "The crate version, the Rust target triple, the profile and the commit, all compiled in at build time. An installed binary answers this without a repository, a toolchain or git, which is what makes a support question answerable.",
            argv: &["distribution", "build"],
            setup: &[],
            expect: Expect::StdoutContains(&["version", "target", "commit"]),
        }],
    },
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
        command: "web list",
        examples: &[ExampleDoc {
            id: "web-list",
            title: "Every web surface this repository exposes",
            description: "The resolved topology, in route-precedence order: the routes the executable answers itself, the application's site, and every generated report that declared itself under the generated web root. Nothing is registered anywhere; each line was discovered.",
            argv: &["web", "list"],
            setup: &[],
            expect: Expect::StdoutContains(&["MOUNT", "/api/v1", "/swagger"]),
        }],
    },
    CommandExamples {
        command: "web",
        examples: &[ExampleDoc {
            id: "web-topology",
            title: "The topology, from the command with no subcommand",
            description: "`web` with nothing after it lists, because listing is what a person wants when they ask what this repository exposes.",
            argv: &["web"],
            setup: &[],
            expect: Expect::StdoutContains(&["ID", "MOUNT"]),
        }],
    },
    CommandExamples {
        command: "web explain",
        examples: &[ExampleDoc {
            id: "web-explain",
            title: "Why a surface exists and where each of its values came from",
            description: "For each field a reader could be surprised by — the mount, the kind, the directory — the source that decided it: a producer's own declaration, the site configuration, the capability registry, or the model's documented default.",
            argv: &["web", "explain", "swagger"],
            setup: &[],
            expect: Expect::StdoutContains(&["swagger", "came from"]),
        }],
    },
    CommandExamples {
        command: "web validate",
        examples: &[ExampleDoc {
            id: "web-validate",
            title: "Check the topology before anything serves or publishes it",
            description: "Two surfaces claiming one path, a surface nested inside another's subtree, a directory outside the generated root or one that walks out of the repository: each is a named finding with the surface, the value, its source and the fix. Exit 10 on any error finding. `--artifacts` also requires every static surface's directory and index to exist, which is what serving and publishing need.",
            argv: &["web", "validate"],
            setup: &[],
            expect: Expect::StdoutContains(&["no conflict"]),
        }],
    },
    CommandExamples {
        command: "web manifest",
        examples: &[ExampleDoc {
            id: "web-manifest",
            title: "Write the resolved topology down for another tool to read",
            description: "The manifest under the generated web root is derived state: a publisher or a CI job may read it instead of resolving the topology again, and nothing may edit it, because the next run overwrites it from the same discovery.",
            argv: &["web", "manifest"],
            setup: &[],
            expect: Expect::StdoutContains(&["web manifest", "surface"]),
        }],
    },
    CommandExamples {
        command: "web report tests",
        examples: &[ExampleDoc {
            id: "web-report-tests",
            title: "Render the suite's own results into the /tests surface",
            description: "The runner writes its report with `MJ_TEST_REPORT=<file> bash test/run.sh`; this renders it, keeps the machine-readable results beside the page, and declares the directory so discovery finds it. Without that file there is nothing to render and the command says so rather than publishing an empty page.",
            argv: &["web", "report", "tests", "--suite", "target/web/run.tsv"],
            setup: &[],
            expect: Expect::ExitCode(13),
        }],
    },
    CommandExamples {
        command: "web report benchmarks",
        examples: &[ExampleDoc {
            id: "web-report-benchmarks",
            title: "Render a benchmark run into the /benchmarks surface",
            description: "Reads a results document — a run's own output, or an accepted baseline under the layer's benchmarks section, which have the same shape — and renders every measured target ordered by median. It measures nothing itself: a figure on the page is a figure a run produced.",
            argv: &[
                "web", "report", "benchmarks",
                "--from", ".ai/repo/benchmarks/rust/baseline.macos-aarch64-debug.json",
            ],
            setup: &[],
            expect: Expect::ExitCode(13),
        }],
    },
    CommandExamples {
        command: "web report ui",
        examples: &[ExampleDoc {
            id: "web-report-ui",
            title: "Render the UI conformance audit into /tests/ui",
            description: "`scripts/ui audit` drives a browser over every page of the built site at every width the compiled stylesheet's breakpoints imply, and writes one results document; this renders it. The rendering is a section of the test surface rather than a surface of its own, because a conformance run is a test run and the topology refuses a surface mounted inside another's subtree. Without that document there is nothing to render and the command says so.",
            argv: &["web", "report", "ui", "--from", "target/web/run-ui.json"],
            setup: &[],
            expect: Expect::ExitCode(13),
        }],
    },
    CommandExamples {
        command: "web compose",
        examples: &[ExampleDoc {
            id: "web-compose",
            title: "Compose every published surface into one publishable tree",
            description: "Each producer owns its own output directory; publication needs one tree, and the mapping is the resolved mount and nothing else. A surface that is discovered is published without a copy step being written anywhere, and a repository with nothing generated yet composes an empty tree rather than an error. Where a surface exists and its directory does not, composition refuses and names the producer to run.",
            argv: &["web", "compose", "--destination", "target/site"],
            setup: &[],
            expect: Expect::Success,
        }],
    },
    CommandExamples {
        command: "devtask issue",
        examples: &[
            ExampleDoc {
                id: "devtask-issue",
                title: "One issue as work to be done, not as metadata",
                description: "Identity, intent, status, milestone, dependencies, blockers, branches, commits, sessions and readiness in one answer, composed from the derivations that already own each half — the plan for the graph, `trace` for git, the canonical record for everything a person wrote. Nothing is manufactured: a key the record does not carry is `unknown` with the reason, never an empty string that reads as authored.",
                argv: &["devtask", "issue", "I0001"],
                setup: &[],
                expect: Expect::StdoutContains(&["readiness", "explicit"]),
            },
            ExampleDoc {
                id: "devtask-issue-json",
                title: "The same, as the shape the API and MCP answer with",
                description: "One domain model behind every projection: this document is what `GET /api/v1/devtask/issue` returns and what the `majordomus_devtask` tool answers, with the four groups kept apart by the type — what a person authored, what the plan derives, what happened locally, and where the external projection stands — and a provenance on every field.",
                argv: &["devtask", "issue", "I0001", "--no-git", "--format", "json"],
                setup: &[],
                expect: Expect::Json(&[
                    "/declaration/title/provenance",
                    "/position/status/provenance",
                    "/readiness/state",
                    "/attestation/explicit",
                ]),
            },
            ExampleDoc {
                id: "devtask-issue-undeclared",
                title: "An id the model does not declare is answered, not refused",
                description: "A typo that read as \"nothing has been authored\" is the one answer a work surface must never give, so an unknown id answers with `declared: false`, the readiness `undeclared`, and every canonical field `unknown` with the reason on it.",
                argv: &["devtask", "issue", "I9999", "--no-git"],
                setup: &[],
                expect: Expect::StdoutContains(&["undeclared"]),
            },
        ],
    },
    CommandExamples {
        command: "devtask milestone",
        examples: &[
            ExampleDoc {
                id: "devtask-milestone",
                title: "What to work on next in one outcome, and what to unblock first",
                description: "The issues partitioned by readiness, the critical blockers ordered by how much unfinished work each holds back, and the startable work partitioned into subsets that may genuinely run at the same time. A pure function of the canonical records — no git, no clock, no network — so two runs on two machines produce the same bytes and a reader derives nothing itself.",
                argv: &["devtask", "milestone", "foundation"],
                setup: &[],
                expect: Expect::StdoutContains(&["READINESS"]),
            },
            ExampleDoc {
                id: "devtask-milestone-json",
                title: "The graph, as the shape the API and MCP answer with",
                description: "Every partition the plan implies and no surface should recompute: `ready`, `blocked`, `waiting`, `active`, `review`, `completion_blocked`, `complete`, `cancelled`, plus `critical_blockers`, `parallelizable` with the scope path behind each serialisation, and `cycles` as strongly connected components.",
                argv: &["devtask", "milestone", "foundation", "--format", "json"],
                setup: &[],
                expect: Expect::Json(&["/counts/total", "/ready", "/critical_blockers", "/parallelizable"]),
            },
        ],
    },
    CommandExamples {
        command: "why",
        examples: &[ExampleDoc {
            id: "why-catalogue",
            title: "The operational moments this repository holds",
            description: "`why` with nothing after it lists, because listing is what a person wants when they ask what this section is. The count on the last line is computed from the catalogue; no number anywhere is written down.",
            argv: &["why"],
            setup: &[],
            expect: Expect::StdoutContains(&["SLUG", "moment(s)"]),
        }],
    },
    CommandExamples {
        command: "why list",
        examples: &[
            ExampleDoc {
                id: "why-list",
                title: "Every public moment, in presentation order",
                description: "Drafts are excluded unless `--all` is given. The facets a listing may be narrowed by are the ones the catalogue itself reports, so an audience or an area added as a file is a filter without anything being registered.",
                argv: &["why", "list"],
                setup: &[],
                expect: Expect::StdoutContains(&["SLUG"]),
            },
            ExampleDoc {
                id: "why-list-audience",
                title: "Only what one audience recognises",
                description: "Membership is declared by each moment and never listed in the audience's own file, so this answer is derived. An audience the catalogue does not have is an invalid input naming the ones it does, not an empty answer.",
                argv: &["why", "list", "--audience", "fixture-team"],
                setup: &[],
                expect: Expect::StdoutContains(&["SLUG"]),
            },
            ExampleDoc {
                id: "why-list-json",
                title: "The same, as the shape the API and MCP answer with",
                description: "One domain model behind every projection: this document is what `GET /api/v1/why` returns and what the `majordomus_why` tool answers, including the derived facets and the catalogue's fingerprint.",
                argv: &["why", "list", "--format", "json"],
                setup: &[],
                expect: Expect::Json(&["/counts/moments", "/facets/audiences", "/fingerprint"]),
            },
        ],
    },
    CommandExamples {
        command: "why show",
        examples: &[ExampleDoc {
            id: "why-show",
            title: "One moment, with every relation derived from its metadata",
            description: "The record as its file declares it, then what nobody authored: the responsibilities its claims belong to, the moments that name it, and the moments nearest it by shared area, audience and tag.",
            argv: &["why", "show", "fixture-moment"],
            setup: &[],
            expect: Expect::StdoutContains(&["fixture-moment", "derived"]),
        }],
    },
    CommandExamples {
        command: "why audiences",
        examples: &[ExampleDoc {
            id: "why-audiences",
            title: "Who recognises what, with the counts derived",
            description: "Each audience with how many public moments name it. The number is computed from the moments; an audience's own file never lists one.",
            argv: &["why", "audiences"],
            setup: &[],
            expect: Expect::StdoutContains(&["SLUG", "TITLE"]),
        }],
    },
    CommandExamples {
        command: "why areas",
        examples: &[ExampleDoc {
            id: "why-areas",
            title: "The operational areas, with the counts derived",
            description: "The same relation read the other way: each area with the public moments that fall under it.",
            argv: &["why", "areas"],
            setup: &[],
            expect: Expect::StdoutContains(&["SLUG", "TITLE"]),
        }],
    },
    CommandExamples {
        command: "why diagnose",
        examples: &[
            ExampleDoc {
                id: "why-diagnose-questions",
                title: "The questionnaire, assembled from the catalogue's own signals",
                description: "With no selection there is nothing to diagnose, so the questions are printed instead of an empty answer. Every line is a signal a moment declares; nothing here is a list of questions.",
                argv: &["why", "diagnose"],
                setup: &[],
                expect: Expect::StdoutContains(&["Which of these happened to you this week?"]),
            },
            ExampleDoc {
                id: "why-diagnose",
                title: "What the symptoms you recognise imply",
                description: "A name is a signal id or a moment id. The answer is counting, not inference: each recommendation carries the moments that produced it, and there is no percentage because there is no model behind one.",
                argv: &["why", "diagnose", "--signal", "fixture-signal"],
                setup: &[],
                expect: Expect::StdoutContains(&["moment(s) matched", "fixture-moment"]),
            },
        ],
    },
    CommandExamples {
        command: "why validate",
        examples: &[ExampleDoc {
            id: "why-validate",
            title: "Check the catalogue before anything projects it",
            description: "A reference that resolves to nothing, with the nearest candidate; a duplicate identity; a file name that disagrees with its id; a public record that does not meet the floor its status promises. Exit 10 on any error.",
            argv: &["why", "validate"],
            setup: &[],
            expect: Expect::StdoutContains(&["moment(s)", "valid"]),
        }],
    },
    CommandExamples {
        command: "serve status",
        examples: &[ExampleDoc {
            id: "serve-status",
            title: "Where this checkout's server stands, and every server of the repository",
            description: "The standing of this checkout's server measured against what this executable would serve, the lease it holds, and every checkout git registers for the repository with its own server. Asked of the running server when there is one, so that the answer includes the lease that process holds; answered locally otherwise.",
            argv: &["serve", "status"],
            setup: &[],
            expect: Expect::StdoutContains(&["standing"]),
        }],
    },
    CommandExamples {
        command: "serve ensure",
        examples: &[ExampleDoc {
            id: "serve-ensure-idle",
            title: "Make sure a server serves this checkout, and let it end when idle",
            description: "Starts a server as a process of its own when none answers, waits until it is ready, and prints one line: the standing, the address, the pid. Run again, it finds the server ready and starts nothing. `--idle` is how long the started server outlives its last peer; one second here, so that the example leaves nothing behind.",
            argv: &["serve", "ensure", "--idle", "1", "--wait", "30"],
            setup: &[],
            expect: Expect::StdoutContains(&["ready"]),
        }],
    },
    CommandExamples {
        command: "serve stop",
        examples: &[ExampleDoc {
            id: "serve-stop-nothing",
            title: "Stop this checkout's server, when there is one",
            description: "Signals the server this checkout's lease names, when it answers for this checkout, and waits for the lease to go. A checkout with no lease has nothing to stop, and says so.",
            argv: &["serve", "stop"],
            setup: &[],
            expect: Expect::StdoutContains(&["nothing to stop"]),
        }],
    },
    CommandExamples {
        command: "serve",
        examples: &[ExampleDoc {
            id: "serve-ephemeral-port",
            title: "Serve the same capabilities over HTTP on a free port",
            description: "Port 0 asks the operating system for a free port; the address is logged on stderr. `/` is the home page, generated from the surfaces this process resolved; the document at /openapi.json is the same one `majordomus generate` commits; /swagger is the Swagger UI over it; /docs/ is this repository's documentation when it has been built for that mount.",
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
        command: "capabilities projections",
        examples: &[
            ExampleDoc {
                id: "capabilities-projections-unmet",
                title: "Every exposure a capability claims that its surface does not answer",
                description: "`rows: 0` is the closure `project.interfaces-are-projections` asks for: every declared command line, route and tool is answered by the surface that carries it. The commands no capability claims are reported beside it, as the measure of how much of the command line is still hand-written.",
                argv: &["capabilities", "projections", "--unmet"],
                setup: &[],
                expect: Expect::Success,
            },
            ExampleDoc {
                id: "capabilities-projections-module",
                title: "Where one module's capabilities appear",
                description: "A row per capability with the command line, HTTP route and MCP tool it reaches, so a capability that exists but is reachable from nowhere is visible as one.",
                argv: &["capabilities", "projections", "--module", "worktree"],
                setup: &[],
                expect: Expect::StdoutContains(&["worktree.topology", "majordomus worktree topology"]),
            },
        ],
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
    CommandExamples {
        command: "commands",
        examples: &[ExampleDoc {
            id: "commands-list",
            title: "Every command this repository offers",
            description: "The command graph, composed from the three declarations that already exist: the clap tree of this executable, the shipped command registry of the shell tool, and the recipes the workflow runner describes. One line per command, with the program that runs it and what running it changes.",
            argv: &["commands"],
            setup: &[],
            expect: Expect::StdoutContains(&["commands", "executable", "read-only"]),
        }],
    },
    CommandExamples {
        command: "commands list",
        examples: &[ExampleDoc {
            id: "commands-list-filtered",
            title: "Only what reads",
            description: "The filters are the graph's own vocabulary rather than a search over text: `--effect read-only` is every command that changes nothing anywhere, which is the same predicate the exposure policy uses to decide what a machine surface may call.",
            argv: &["commands", "list", "--effect", "read-only"],
            setup: &[],
            expect: Expect::StdoutContains(&["read-only"]),
        }],
    },
    CommandExamples {
        command: "commands show",
        examples: &[ExampleDoc {
            id: "commands-show",
            title: "One command, and every surface that carries it",
            description: "The arguments with the source of each one's values, the effect, and the projections: the command line, the workflow recipe, the MCP tool, the HTTP route, the Cockpit and the page. A surface that withholds it says why.",
            argv: &["commands", "show", "executable.worktree.status"],
            setup: &[],
            expect: Expect::StdoutContains(&["executable.worktree.status", "projections"]),
        }],
    },
    CommandExamples {
        command: "commands explain",
        examples: &[ExampleDoc {
            id: "commands-explain",
            title: "Why a command appears where it does",
            description: "The same command with its provenance: the file that declares it, the reader that found it, the capability behind it when there is one, what it requires, and the file the exposure policy lives in. Nothing about a command's placement is a mystery a grep has to solve.",
            argv: &["commands", "explain", "executable.serve"],
            setup: &[],
            expect: Expect::StdoutContains(&["declared in", "policy"]),
        }],
    },
    CommandExamples {
        command: "commands graph",
        examples: &[ExampleDoc {
            id: "commands-graph-json",
            title: "The whole graph as one document",
            description: "Deterministic and fingerprinted: two builds over one tree produce the same bytes, which is what lets the workflow bridge, the completion index and the Cockpit all key on the fingerprint instead of regenerating.",
            argv: &["commands", "graph", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/schema", "/fingerprint", "/commands"]),
        }],
    },
    CommandExamples {
        command: "commands bridge",
        examples: &[ExampleDoc {
            id: "commands-bridge",
            title: "The workflow runner's recipes, derived",
            description: "Every command of both programs, written as a recipe that runs the canonical program with the caller's own arguments. It goes under .ai/local/cache/, which no commit carries, and it is rewritten only when the graph's fingerprint changes.",
            argv: &["commands", "bridge"],
            setup: &[],
            expect: Expect::StdoutContains(&["bridge", "recipe"]),
        }],
    },
    CommandExamples {
        command: "completion",
        examples: &[ExampleDoc {
            id: "completion-default",
            title: "The integration a person installs once",
            description: "With no subcommand, the shell integration for zsh. It contains no command, no flag and no identifier: every candidate comes from a query against the command graph of the repository the shell is in, so one integration serves every checkout and never goes stale.",
            argv: &["completion"],
            setup: &[],
            expect: Expect::StdoutContains(&["completion query", "compdef"]),
        }],
    },
    CommandExamples {
        command: "completion init",
        examples: &[ExampleDoc {
            id: "completion-init-bash",
            title: "The same, for bash",
            description: "A different shell's protocol, the same question. Both adapters read the words being completed, find the cursor, ask this executable and print what comes back.",
            argv: &["completion", "init", "--shell", "bash"],
            setup: &[],
            expect: Expect::StdoutContains(&["completion query", "complete -F"]),
        }],
    },
    CommandExamples {
        command: "completion query",
        examples: &[ExampleDoc {
            id: "completion-query",
            title: "What a shell asks on every TAB",
            description: "The words of the command line and the position of the cursor; back come the candidates with their descriptions. The same call answers the workflow runner's completion with `--surface workflow`, resolving the recipe name to the command it bridges and then completing that command's own arguments.",
            argv: &["completion", "query", "--surface", "cli", "--", "majordomus", "work"],
            setup: &[],
            expect: Expect::StdoutContains(&["worktree"]),
        }],
    },
    CommandExamples {
        command: "quality report",
        examples: &[
            ExampleDoc {
                id: "quality-summary",
                title: "Where the crate's public surface stands",
                description: "The counts alone: how much of the exported surface is documented and exampled, how many modules something exercises, and how the canonical operations stand against the command line, HTTP, OpenAPI and MCP. Exits 10 when any finding stands outside the recorded baseline.",
                argv: &["quality", "report", "--summary"],
                setup: &[],
                // Success and not a fixed line: a repository that carries no Rust crate is
                // answered with the reason and exits 0, because a rule that cannot apply is
                // not a violation — and the examples run against exactly such a repository.
                expect: Expect::Success,
            },
            ExampleDoc {
                id: "quality-code-json",
                title: "One kind of finding, with the rule and the remedy",
                description: "Filtered to one violation code. Every finding carries the rule that requires it, where it is, why it matters and what to do — which is what lets a person and an agent act on the same report.",
                argv: &[
                    "quality",
                    "report",
                    "--code",
                    "RUST_MODULE_MISSING_EXAMPLE",
                    "--format",
                    "json",
                ],
                setup: &[],
                expect: Expect::Json(&["/measured", "/passes", "/report/schema"]),
            },
        ],
    },
    CommandExamples {
        command: "devcontext compile",
        examples: &[
            ExampleDoc {
                id: "devcontext-compile-issue",
                title: "The context a session working on one issue should be given",
                description: "The issue is the seed. Its milestone follows along the `belongs_to` edge of the composed graph, the code and the cases under the scope it declares follow from the paths, and the policy and the scope are governance every session is held to. Every selected line names the selector that reached it and why; everything left out is listed with the reason.",
                argv: &["devcontext", "compile", "--issue", "I0001"],
                setup: &[],
                expect: Expect::StdoutContains(&["SELECTED", "majordomus://issue/I0001", "EXCLUDED"]),
            },
            ExampleDoc {
                id: "devcontext-compile-json",
                title: "The same, as the structure every other surface answers with",
                description: "The canonical form: `GET /api/v1/devcontext` and the `majordomus_devcontext` tool return this document. Entries keep their canonical identifier, the index's provenance, every discovery path with its confidence, and the cost in estimated tokens; nothing is flattened to prose.",
                argv: &["devcontext", "compile", "--issue", "I0001", "--format", "json"],
                setup: &[],
                expect: Expect::Json(&["/selected/0/uri", "/selected/0/discovered_by/0/reason", "/budget/limit_tokens", "/fingerprint"]),
            },
        ],
    },
    CommandExamples {
        command: "devcontext explain",
        examples: &[ExampleDoc {
            id: "devcontext-explain-seed",
            title: "Why one thing is in the context",
            description: "The identifier is judged under the same request `compile` takes: selected, excluded with the reason, folded into another identifier, held by the index and never reached, or unknown.",
            argv: &["devcontext", "explain", "majordomus://issue/I0001", "--issue", "I0001"],
            setup: &[],
            expect: Expect::StdoutContains(&["selected", "majordomus://issue/I0001"]),
        }],
    },
    CommandExamples {
        command: "devcontext policy",
        examples: &[ExampleDoc {
            id: "devcontext-policy",
            title: "The compiler's own rules",
            description: "The tiers in the order the budget spends in, every edge kind the composed graph declares with the weight it is followed by or the reason it is refused, and which selectors infer rather than read.",
            argv: &["devcontext", "policy"],
            setup: &[],
            expect: Expect::StdoutContains(&["TIER", "is_a", "REFUSED"]),
        }],
    },
    CommandExamples {
        command: "mesh status",
        examples: &[ExampleDoc {
            id: "mesh-status",
            title: "Whether this checkout's server runs a mesh",
            description: "The mesh lives inside the shared server, so the command asks the running server for `mesh.status` and renders it. No server, or no mesh declaration, is an answer with its reason — never an error: the default posture is that nothing leaves the machine until a declaration says otherwise.",
            argv: &["mesh", "status"],
            setup: &[],
            expect: Expect::StdoutContains(&["mesh"]),
        }],
    },
    CommandExamples {
        command: "mesh nodes",
        examples: &[ExampleDoc {
            id: "mesh-nodes",
            title: "The nodes the running server has observed",
            description: "One row per node, deduplicated by node identity across every discovery source, in node-id order: trust, presence, endpoints and where each observation came from. The registry lives in the server's memory; without a running server there are no nodes to list, and the command says so.",
            argv: &["mesh", "nodes"],
            setup: &[],
            expect: Expect::StdoutContains(&["mesh"]),
        }],
    },
    CommandExamples {
        command: "mesh identity",
        examples: &[ExampleDoc {
            id: "mesh-identity",
            title: "This machine's node identity, public half only",
            description: "The node id is a digest of the machine's Ed25519 public key, kept under the user's state directory — never inside a repository, and the signing key appears in no output. Absent is an answer: the identity is created when a mesh first activates.",
            argv: &["mesh", "identity"],
            setup: &[],
            expect: Expect::StdoutContains(&["present"]),
        }],
    },
    CommandExamples {
        command: "mesh doctor",
        examples: &[ExampleDoc {
            id: "mesh-doctor",
            title: "Every mesh prerequisite, proved on this machine alone",
            description: "Deterministic checks in a fixed order — the declaration parses, the identity loads, a UDP socket binds, the multicast group joins, broadcast enables, and the protocol signs, encodes, parses and verifies in memory. The report is the value and the command exits 0; a failed check is a row that says why, so `--format json` scripts against it.",
            argv: &["mesh", "doctor"],
            setup: &[],
            expect: Expect::StdoutContains(&["protocol"]),
        }],
    },
    CommandExamples {
        command: "models list",
        examples: &[ExampleDoc {
            id: "models-list",
            title: "The declared model catalogue",
            description: "Every vendor and model share/models.yaml declares, in declaration order — which is also routing's preference order. Vendors show whether their named credential variable is set: presence only, never a value. An empty catalogue is an answer, not an error.",
            argv: &["models", "list"],
            setup: &[],
            expect: Expect::StdoutContains(&["model(s)"]),
        }],
    },
    CommandExamples {
        command: "models route",
        examples: &[ExampleDoc {
            id: "models-route",
            title: "Which model a need selects, and why",
            description: "The first declared model satisfying every requirement wins; the qualifying rest are the fallback chain, and every excluded model carries the first check it failed. Pure over the declared data — the same question always gets the same answer, and the reasons are in it.",
            argv: &["models", "route", "--require", "text"],
            setup: &[],
            expect: Expect::StdoutContains(&["selected"]),
        }],
    },
];
