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
    /// How this project is packaged, published and installed: the platforms, the artifact names, the installer, the releases
    Distribution(DistributionArgs),
    /// The branch-to-worktree topology: where every linked worktree belongs (<repo>-wt/<branch>), where each one is, and the lifecycle — create, migrate, repair, guard
    #[command(alias = "wt")]
    Worktree(WorktreeArgs),
    /// The repository knowledge system: what the repository knows about itself, held against a committed baseline — scan, status, list, show, search, explain, graph, impact, gaps, coverage, stale, conflicts, reconcile, validate, baseline, check, canonicality, derive, context
    Knowledge(KnowledgeArgs),
    /// The canonicality audit: every capability's one canonical source and the surfaces derived from it, every hand-kept mirror, orphan projection and undeclared generated file; the CI gate of the canonicality doctrine
    Canonicality(CanonicalityArgs),
    /// Why the knowledge model says what it says about one thing: a node, a capability, an object URI or a path — its provenance, evidence, claims, freshness, relations, conflicts, gaps and remedies
    Explain(ExplainArgs),
    /// A change set inspected before it is merged: what it touches in the knowledge, every capability it adds with the surfaces derived for it, and the debt it introduces
    Change(ChangeArgs),
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

#[derive(Debug, Args)]
/// `majordomus knowledge`. The output shape is global, so it reads the way a person writes
/// it — `knowledge list --format json` — and is declared once.
pub struct KnowledgeArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// The subcommand; none is `status`.
    pub command: Option<KnowledgeCommand>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus knowledge`.
pub enum KnowledgeCommand {
    /// Adopt a brownfield repository: scan it, record every present fact and every present debt as the baseline, and print where it stands; refuses to overwrite a recorded baseline without --force
    Bootstrap {
        /// Record over a baseline that exists
        #[arg(long)]
        force: bool,
    },
    /// Scan the repository and print the whole model as one JSON document (`majordomus/knowledge/v1`); --public keeps only what may leave the repository
    Scan {
        /// Only the public projection
        #[arg(long)]
        public: bool,
    },
    /// Where the knowledge stands: nodes by kind, freshness and provenance, conflicts, gaps, coverage, the check and the canonicality verdict
    Status,
    /// List nodes, filtered; one page at a time
    List {
        /// Only this node kind (`component`, `document`, `rule`, `capability`, ...)
        #[arg(long, value_name = "KIND")]
        kind: Option<String>,
        /// Only this provenance: observed, declared, derived, curated
        #[arg(long, value_name = "WORD")]
        provenance: Option<String>,
        /// Only this freshness: current, possibly_stale, stale, conflicted, unverified
        #[arg(long, value_name = "WORD")]
        freshness: Option<String>,
        /// Only this ownership: external, majordomus, hybrid
        #[arg(long, value_name = "WORD")]
        ownership: Option<String>,
        /// Only nodes this extractor produced
        #[arg(long, value_name = "ID")]
        extractor: Option<String>,
        /// A substring of the id or the title
        #[arg(long, value_name = "TEXT")]
        query: Option<String>,
        /// Only nodes whose freshness is debt
        #[arg(long)]
        debt: bool,
        /// Skip this many
        #[arg(long, default_value_t = 0)]
        offset: usize,
        /// At most this many; 0 for the default
        #[arg(long, default_value_t = 0)]
        limit: usize,
    },
    /// One node with its claims, evidence, relations, conflicts and gaps
    Show {
        /// A node id, a capability id, an object URI or a path
        id: String,
    },
    /// Search ids, titles, summaries and claim values
    Search {
        /// What to look for
        query: String,
        /// At most this many hits; 0 for the default
        #[arg(long, default_value_t = 0)]
        limit: usize,
    },
    /// Why the model says what it says about one node: provenance, evidence, claims with freshness, relations, conflicts, gaps, remedies
    Explain {
        /// A node id, a capability id, an object URI or a path
        id: String,
    },
    /// A slice of the knowledge graph: around a root, or every node of a kind
    Graph {
        /// Cut the slice around this node
        #[arg(long, value_name = "ID")]
        root: Option<String>,
        /// Hops from the root; 2 when unset
        #[arg(long, default_value_t = 0)]
        depth: usize,
        /// Without a root: only this kind
        #[arg(long, value_name = "KIND")]
        kind: Option<String>,
        /// At most this many nodes
        #[arg(long, default_value_t = 0)]
        limit: usize,
    },
    /// What a change set touches: the working tree against HEAD (or --base), two revisions (--base --to), or named paths
    Impact {
        /// The base revision; HEAD when unset
        #[arg(long, value_name = "REV")]
        base: Option<String>,
        /// Compare the base with this revision instead of the working tree
        #[arg(long, value_name = "REV")]
        to: Option<String>,
        /// Changed paths, named outright
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
    },
    /// Everything the repository could know and does not, with the remedy for each
    Gaps {
        /// Only this category: undocumented_component, unresolved_reference, unverified_knowledge, unexercised_capability, canonicality
        #[arg(long, value_name = "WORD")]
        category: Option<String>,
    },
    /// Coverage over deterministic denominators: numbers, and what is missing
    Coverage,
    /// Every node whose freshness is debt, with the reason: what a person should look at
    Stale,
    /// Every conflict: both sides, severity, basis, resolution, remedy
    Conflicts {
        /// Only open conflicts
        #[arg(long)]
        open_only: bool,
    },
    /// Accept one open conflict by id, with a reason: it stays reported, and stops counting as new debt
    Accept {
        /// The conflict id, as `knowledge conflicts` prints it (`<subject>#<predicate>`)
        conflict: String,
        /// Why both values stand
        #[arg(long, value_name = "TEXT")]
        reason: String,
    },
    /// Propose what to do about every conflict, stale claim, unresolved reference and gap; --accept records that curated claims were verified against their present evidence
    Reconcile {
        /// Record the verifications in the baseline (a deliberate act; the diff is in the commit)
        #[arg(long)]
        accept: bool,
    },
    /// Validate the model, the baseline and the exceptions against their contracts; exit 10 with each finding named
    Validate,
    /// The committed baseline: show it, record it, or migrate it to the current schema
    Baseline {
        #[command(subcommand)]
        /// What to do with it; none shows it
        command: Option<KnowledgeBaselineCommand>,
    },
    /// Hold the scan against the baseline: exit 0 when the mode passes, 10 with every new debt item named
    Check {
        /// Check in this mode instead of the policy's: observe, warn, protect, strict
        #[arg(long, value_name = "WORD")]
        mode: Option<String>,
    },
    /// The canonicality audit: every capability's canonical source and derived surfaces, every violation, the manual maintenance surface; exit 10 when a violation counts
    Canonicality {
        /// Only this capability's row
        #[arg(long, value_name = "ID")]
        capability: Option<String>,
    },
    /// Run the semantic provider the policy names over the model and cache what it derived; off unless the policy enables it, and nothing leaves the machine unless the policy allows it
    Derive {
        /// Only nodes of these kinds
        #[arg(long = "kind", value_name = "KIND")]
        kinds: Vec<String>,
        /// Show what would be given to the provider and what withheld; run nothing
        #[arg(long)]
        dry_run: bool,
    },
    /// How the model is made: every extractor with its vocabulary, the providers, the schema versions and migrations
    Extractors,
    /// What an agent should read before touching some paths, cut to a budget
    Context {
        /// The paths about to be touched; the whole repository when none
        #[arg(value_name = "PATH")]
        paths: Vec<String>,
        /// The budget in bytes; 0 for the default
        #[arg(long, default_value_t = 0)]
        budget: usize,
        /// Only public knowledge
        #[arg(long)]
        public: bool,
    },
    /// What a change set means for the knowledge: the paths that changed, what they touch, every capability the change adds with the surfaces derived for it, and the canonicality and freshness debt it introduces — the pull-request gate
    Inspect {
        /// The base revision; HEAD when unset (the working tree), or a branch to compare with
        #[arg(long, value_name = "REV")]
        base: Option<String>,
    },
    /// Every node id, one per line, for a shell's completion
    Ids {
        /// Only this kind
        #[arg(long, value_name = "KIND")]
        kind: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus knowledge baseline`.
pub enum KnowledgeBaselineCommand {
    /// The baseline as recorded: when, how much of what, and what the scan would change
    Show,
    /// Record the present scan as the baseline: every fact verified, every present debt tolerated; refuses to overwrite without --force
    Record {
        /// Record over a baseline that exists
        #[arg(long)]
        force: bool,
    },
    /// Rewrite the baseline in the current schema, naming each migration step; a current one is left alone
    Migrate,
}

#[derive(Debug, Args)]
/// `majordomus canonicality`.
pub struct CanonicalityArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// The subcommand; none is `check`.
    pub command: Option<CanonicalityCommand>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus canonicality`.
pub enum CanonicalityCommand {
    /// The audit over every capability and the tree; exit 10 when a violation counts
    Check,
    /// One capability: its canonical source, every derived surface, every hand-written mention, its manual maintenance surface and its verdict
    Explain {
        /// The capability id
        capability: String,
    },
}

#[derive(Debug, Args)]
/// `majordomus change`.
pub struct ChangeArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    #[command(subcommand)]
    /// The subcommand; none is `inspect`.
    pub command: Option<ChangeCommand>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text, global = true)]
    /// Output shape
    pub format: OutputFormat,
}

#[derive(Debug, Subcommand)]
/// The subcommands of `majordomus change`.
pub enum ChangeCommand {
    /// Inspect the working tree against HEAD, or against --base: the same answer as `knowledge inspect`
    Inspect {
        /// The base revision
        #[arg(long, value_name = "REV")]
        base: Option<String>,
    },
}

#[derive(Debug, Args)]
/// `majordomus explain`.
pub struct ExplainArgs {
    #[command(flatten)]
    /// Where and how the repository is read.
    pub repo: RepoArgs,

    /// A node id, a capability id, an object URI or a path; `capability <id>` is accepted too
    #[arg(value_name = "SUBJECT", num_args = 1..=2)]
    pub subject: Vec<String>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    /// Output shape
    pub format: OutputFormat,
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

    /// Interface to bind; loopback unless you say otherwise
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to bind; 0 picks a free one and the address is logged on stderr
    #[arg(long, default_value_t = DEFAULT_PORT)]
    pub port: u16,

    /// Bind the address this deployment object declares (.ai/repo/deployments/<ID>.yaml)
    /// instead of the local default. What a hosted process is started with; the address is
    /// the object's, not this command line's
    #[arg(long, value_name = "ID", conflicts_with_all = ["host", "port"])]
    pub deployment: Option<String>,
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
        command: "knowledge",
        examples: &[ExampleDoc {
            id: "knowledge-default-status",
            title: "Where the repository's knowledge stands",
            description: "`knowledge` with nothing after it is `knowledge status`: one scan of the checkout, summarised — nodes by kind, freshness and provenance, open conflicts, gaps, coverage, the check against the baseline in the policy's mode, and the canonicality verdict.",
            argv: &["knowledge"],
            setup: &[],
            expect: Expect::StdoutContains(&["nodes", "freshness", "check"]),
        }],
    },
    CommandExamples {
        command: "knowledge status",
        examples: &[ExampleDoc {
            id: "knowledge-status-json",
            title: "The status as one document",
            description: "The same answer as JSON: what `majordomus_knowledge`, `GET /api/v1/knowledge` and the Cockpit's Knowledge page read.",
            argv: &["knowledge", "status", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/repository/name", "/freshness", "/check/verdict", "/canonicality/verdict", "/coverage/rows"]),
        }],
    },
    CommandExamples {
        command: "knowledge bootstrap",
        examples: &[ExampleDoc {
            id: "knowledge-bootstrap",
            title: "Adopt a repository that already has debt",
            description: "The first run in a brownfield repository: scan it, verify every curated claim against its present evidence, tolerate every present debt by name, and write the baseline under the knowledge section. From then on `knowledge check` refuses new debt and the recorded debt may only shrink.",
            argv: &["knowledge", "bootstrap"],
            setup: &[],
            expect: Expect::StdoutContains(&["baseline", "recorded"]),
        }],
    },
    CommandExamples {
        command: "knowledge scan",
        examples: &[ExampleDoc {
            id: "knowledge-scan-public",
            title: "The whole model, public projection",
            description: "Every extractor, evidence, node with claims, relation, conflict, gap and coverage row as one `majordomus/knowledge/v1` document, restricted to what may leave the repository. The site's knowledge dataset is this document.",
            argv: &["knowledge", "scan", "--public"],
            setup: &[],
            expect: Expect::Json(&["/schema", "/nodes", "/evidence", "/relations", "/fingerprint"]),
        }],
    },
    CommandExamples {
        command: "knowledge list",
        examples: &[ExampleDoc {
            id: "knowledge-list-documents",
            title: "Every document the repository carries",
            description: "One line per node of one kind: id, freshness, provenance, and the reason when it is not current.",
            argv: &["knowledge", "list", "--kind", "document"],
            setup: &[],
            expect: Expect::StdoutContains(&["document:"]),
        }],
    },
    CommandExamples {
        command: "knowledge show",
        examples: &[ExampleDoc {
            id: "knowledge-show-readme",
            title: "One node, everything that bears on it",
            description: "The README as the model holds it: its claims with provenance and freshness, the evidence with fingerprints, the relations in and out. A path, an object URI or a capability id resolve to their node too.",
            argv: &["knowledge", "show", "README.md"],
            setup: &[],
            expect: Expect::StdoutContains(&["README.md", "claims"]),
        }],
    },
    CommandExamples {
        command: "knowledge search",
        examples: &[ExampleDoc {
            id: "knowledge-search",
            title: "Find a node by a word",
            description: "Ids and titles first, then summaries, then claim values; ranked and stable.",
            argv: &["knowledge", "search", "readme"],
            setup: &[],
            expect: Expect::StdoutContains(&["README"]),
        }],
    },
    CommandExamples {
        command: "knowledge explain",
        examples: &[ExampleDoc {
            id: "knowledge-explain-readme",
            title: "Why the model says what it says",
            description: "How the node is known, what it rests on, every claim with its freshness and the reason, and what to do when something is wrong. The same answer `majordomus explain <subject>` prints.",
            argv: &["knowledge", "explain", "document:README.md"],
            setup: &[],
            expect: Expect::StdoutContains(&["document:README.md", "evidence"]),
        }],
    },
    CommandExamples {
        command: "knowledge graph",
        examples: &[ExampleDoc {
            id: "knowledge-graph-around-readme",
            title: "The neighbourhood of one node",
            description: "The nodes within one hop of the README and the typed relations among them, as JSON a drawing reads.",
            argv: &["knowledge", "graph", "--root", "document:README.md", "--depth", "1", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/nodes", "/edges"]),
        }],
    },
    CommandExamples {
        command: "knowledge impact",
        examples: &[ExampleDoc {
            id: "knowledge-impact-readme",
            title: "What a change to one file touches",
            description: "The nodes whose evidence is the named path, the claims resting on it, and everything reached along propagating relations, nearest first.",
            argv: &["knowledge", "impact", "README.md"],
            setup: &[],
            expect: Expect::StdoutContains(&["README.md"]),
        }],
    },
    CommandExamples {
        command: "knowledge gaps",
        examples: &[ExampleDoc {
            id: "knowledge-gaps",
            title: "What the repository could know and does not",
            description: "Every gap with its category, the reason and the remedy: a worklist, not a score.",
            argv: &["knowledge", "gaps", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/gaps", "/tallies"]),
        }],
    },
    CommandExamples {
        command: "knowledge coverage",
        examples: &[ExampleDoc {
            id: "knowledge-coverage",
            title: "Coverage over deterministic denominators",
            description: "One row per denominator — components documented, capabilities exercised, references resolved, curated records verified, artifacts derived, layer objects reached — with the numbers and what is missing.",
            argv: &["knowledge", "coverage"],
            setup: &[],
            expect: Expect::StdoutContains(&["components-documented", "references-resolved"]),
        }],
    },
    CommandExamples {
        command: "knowledge stale",
        examples: &[ExampleDoc {
            id: "knowledge-stale",
            title: "What a person should look at",
            description: "Every node whose freshness is debt — stale, possibly stale, unverified, conflicted — with the reason. Empty when everything is current.",
            argv: &["knowledge", "stale"],
            setup: &[],
            expect: Expect::Success,
        }],
    },
    CommandExamples {
        command: "knowledge conflicts",
        examples: &[ExampleDoc {
            id: "knowledge-conflicts",
            title: "Where two sources disagree",
            description: "Every conflict with both sides, their provenance and evidence, the severity and the remedy; none is resolved silently.",
            argv: &["knowledge", "conflicts", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/conflicts", "/open"]),
        }],
    },
    CommandExamples {
        command: "knowledge accept",
        examples: &[ExampleDoc {
            id: "knowledge-accept-unknown-conflict",
            title: "Accepting a conflict names one that exists",
            description: "A conflict is accepted by the id `knowledge conflicts` prints, with a reason that goes into the baseline; an id that is not an open conflict is refused with exit 12 and nothing is written.",
            argv: &["knowledge", "accept", "component:none#version", "--reason", "both are right"],
            setup: &[],
            expect: Expect::ExitCode(12),
        }],
    },
    CommandExamples {
        command: "knowledge reconcile",
        examples: &[ExampleDoc {
            id: "knowledge-reconcile-propose",
            title: "What to do about every finding",
            description: "Proposals with an owner: which a person edits (an external source is never rewritten), and which `--accept` applies by recording in the baseline that the curated claims were verified against their present evidence.",
            argv: &["knowledge", "reconcile"],
            setup: &[],
            expect: Expect::StdoutContains(&["proposal"]),
        }],
    },
    CommandExamples {
        command: "knowledge validate",
        examples: &[ExampleDoc {
            id: "knowledge-validate",
            title: "The model and its files against their contracts",
            description: "Every diagnostic of the scan, the baseline's and the exceptions' schema and shape, and the migrations a file would need; exit 10 when a finding is an error.",
            argv: &["knowledge", "validate"],
            setup: &[],
            expect: Expect::StdoutContains(&["knowledge"]),
        }],
    },
    CommandExamples {
        command: "knowledge baseline",
        examples: &[ExampleDoc {
            id: "knowledge-baseline-default-show",
            title: "The baseline as recorded",
            description: "`baseline` with nothing after it shows it: when it was recorded, how much of what it holds, and whether the present scan would change it.",
            argv: &["knowledge", "baseline"],
            setup: &[&["knowledge", "bootstrap"]],
            expect: Expect::StdoutContains(&["recorded"]),
        }],
    },
    CommandExamples {
        command: "knowledge baseline show",
        examples: &[ExampleDoc {
            id: "knowledge-baseline-show-json",
            title: "The baseline as one document",
            description: "The typed baseline: evidence fingerprints, verified claims, tolerated debt, accepted conflicts, tolerated canonicality violations.",
            argv: &["knowledge", "baseline", "show", "--format", "json"],
            setup: &[&["knowledge", "bootstrap"]],
            expect: Expect::Json(&["/schema", "/nodes", "/verified", "/debt"]),
        }],
    },
    CommandExamples {
        command: "knowledge baseline record",
        examples: &[ExampleDoc {
            id: "knowledge-baseline-record-force",
            title: "Record the baseline again, deliberately",
            description: "After debt was paid down or a conflict accepted: record the present state over the old one. The diff is in the commit, which is the review.",
            argv: &["knowledge", "baseline", "record", "--force"],
            setup: &[&["knowledge", "bootstrap"]],
            expect: Expect::StdoutContains(&["recorded"]),
        }],
    },
    CommandExamples {
        command: "knowledge baseline migrate",
        examples: &[ExampleDoc {
            id: "knowledge-baseline-migrate",
            title: "Bring the baseline to the current schema",
            description: "A baseline written by an older Majordomus is rewritten step by step, each step named; a current one is left alone and says so. A newer one is refused with the version that would read it.",
            argv: &["knowledge", "baseline", "migrate"],
            setup: &[&["knowledge", "bootstrap"]],
            expect: Expect::StdoutContains(&["baseline"]),
        }],
    },
    CommandExamples {
        command: "knowledge check",
        examples: &[ExampleDoc {
            id: "knowledge-check-after-bootstrap",
            title: "The gate, right after adoption",
            description: "With the present debt tolerated by the baseline, the check passes in protect mode: nothing new. A later change that adds a stale claim, an open conflict or a canonicality violation fails it with the item named; a change that pays debt down passes and says the baseline should be recorded again.",
            argv: &["knowledge", "check"],
            setup: &[&["knowledge", "bootstrap"]],
            expect: Expect::StdoutContains(&["pass"]),
        }],
    },
    CommandExamples {
        command: "knowledge canonicality",
        examples: &[ExampleDoc {
            id: "knowledge-canonicality-json",
            title: "The canonicality audit as one document",
            description: "Every capability with its canonical source, derived surfaces, hand-written mentions and manual maintenance surface; every violation with whether the baseline tolerates it or an exception covers it; the verdict.",
            argv: &["knowledge", "canonicality", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/capabilities", "/violations", "/verdict", "/mms_centi"]),
        }],
    },
    CommandExamples {
        command: "knowledge derive",
        examples: &[ExampleDoc {
            id: "knowledge-derive-refused-when-off",
            title: "The semantic layer is off until the policy turns it on",
            description: "Without `knowledge.semantic.enabled: true` in the policy the command refuses with exit 10 and says which switch to set. Nothing is read by a provider and nothing leaves the machine.",
            argv: &["knowledge", "derive", "--dry-run"],
            setup: &[],
            expect: Expect::ExitCode(10),
        }],
    },
    CommandExamples {
        command: "knowledge extractors",
        examples: &[ExampleDoc {
            id: "knowledge-extractors",
            title: "How the model is made",
            description: "Every extractor with the kinds, relations and predicates it declares, the semantic providers this executable ships, and the schema versions it reads and writes.",
            argv: &["knowledge", "extractors"],
            setup: &[],
            expect: Expect::StdoutContains(&["git", "layer", "docs", "registry"]),
        }],
    },
    CommandExamples {
        command: "knowledge context",
        examples: &[ExampleDoc {
            id: "knowledge-context-docs",
            title: "What to read before touching a directory",
            description: "The nodes whose sources are under the path and what they govern, describe and depend on, most governing first; the claims among them that are not current as caveats; cut to a budget. What an agent asks over MCP as `majordomus_knowledge_context`.",
            argv: &["knowledge", "context", "docs", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/nodes", "/caveats", "/budget"]),
        }],
    },
    CommandExamples {
        command: "knowledge inspect",
        examples: &[ExampleDoc {
            id: "knowledge-inspect-working-tree",
            title: "What this change means, before it is merged",
            description: "The paths that changed against HEAD, the nodes and claims they touch, every capability the change adds with the checklist of surfaces derived for it, and the freshness and canonicality debt the change introduces. What a pull request is inspected with.",
            argv: &["knowledge", "inspect"],
            setup: &[],
            expect: Expect::StdoutContains(&["change set"]),
        }],
    },
    CommandExamples {
        command: "change",
        examples: &[ExampleDoc {
            id: "change-default-inspect",
            title: "The pull-request gate",
            description: "`change` with nothing after it is `change inspect`: the working tree against HEAD, or `--base origin/master` for a branch, with what the change touches, what it adds and what debt it introduces.",
            argv: &["change"],
            setup: &[],
            expect: Expect::StdoutContains(&["change set"]),
        }],
    },
    CommandExamples {
        command: "change inspect",
        examples: &[ExampleDoc {
            id: "change-inspect-json",
            title: "The inspection as one document",
            description: "The same answer as JSON: the change set, the impact, the added capabilities with their surfaces, and the debt, for a gate that reads the verdict.",
            argv: &["change", "inspect", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/impact", "/added_capabilities", "/verdict"]),
        }],
    },
    CommandExamples {
        command: "knowledge ids",
        examples: &[ExampleDoc {
            id: "knowledge-ids",
            title: "Every node id, for completion",
            description: "One id per line, nothing else: what a shell completes `knowledge show` and `knowledge explain` with.",
            argv: &["knowledge", "ids", "--kind", "document"],
            setup: &[],
            expect: Expect::StdoutContains(&["document:README.md"]),
        }],
    },
    CommandExamples {
        command: "canonicality",
        examples: &[ExampleDoc {
            id: "canonicality-default-check",
            title: "The canonicality gate",
            description: "`canonicality` with nothing after it is `canonicality check`: the audit over every capability and the tree, the manual maintenance surface, and the verdict — exit 10 when a violation counts that neither the baseline tolerates nor an exception covers.",
            argv: &["canonicality"],
            setup: &[],
            expect: Expect::StdoutContains(&["MMS", "verdict"]),
        }],
    },
    CommandExamples {
        command: "canonicality check",
        examples: &[ExampleDoc {
            id: "canonicality-check-json",
            title: "The audit as one document",
            description: "The same audit as JSON, for a gate that reads the verdict and a page that lists the violations.",
            argv: &["canonicality", "check", "--format", "json"],
            setup: &[],
            expect: Expect::Json(&["/verdict", "/capabilities", "/violations", "/exceptions"]),
        }],
    },
    CommandExamples {
        command: "canonicality explain",
        examples: &[ExampleDoc {
            id: "canonicality-explain-capability",
            title: "One capability's canonical source and derived surfaces",
            description: "The declaration file that is its one source of truth, every surface derived from it with a tick, every hand-written file that names it, the manual maintenance surface, and the verdict.",
            argv: &["canonicality", "explain", "rks.status"],
            setup: &[],
            expect: Expect::StdoutContains(&["canonical source", "rks.status"]),
        }],
    },
    CommandExamples {
        command: "explain",
        examples: &[ExampleDoc {
            id: "explain-capability",
            title: "Why, for one capability",
            description: "`explain capability <id>` and `explain <subject>` are the knowledge model's explanation of one thing: how it is known, what it rests on, every claim with its freshness, what it relates to, and what to do. For a capability the canonical source and the derived surfaces are the first lines.",
            argv: &["explain", "capability", "rks.status"],
            setup: &[],
            expect: Expect::StdoutContains(&["capability:rks.status", "canonical"]),
        }],
    },
];
