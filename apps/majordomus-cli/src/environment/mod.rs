//! The repository environment: one typed snapshot of what this checkout is right now —
//! the project, the repository, version control, the toolchains it declares, what the
//! layer holds, the workflows a person can run, the provider projections and the local
//! services — with a provenance entry for every fact in it.
//!
//! It is the canonical source for every surface that says any of those things. The direnv
//! banner, `majordomus env`, `GET /api/v1/environment`, the MCP resource
//! `majordomus://environment`, the Cockpit's overview and the documentation are
//! renderings of this one value; none of them discovers anything of its own. That is the
//! whole point: before it existed, "what branch is this and what is running" was a shell
//! pipeline in whoever's dotfiles, and a second one in the next surface that wanted it.
//!
//! # Two resolutions, one model
//!
//! Building the index costs seconds, and the banner runs on every `cd`. So a snapshot is
//! resolved in one of two modes ([`Resolution`]), and both produce the same type:
//!
//! - [`Resolution::Full`] reads everything, including the index, and writes the cache.
//! - [`Resolution::Fast`] reads only what is cheap — one `git` call, one `just` call, a
//!   few file reads — and takes the rest from the cache written by the last full
//!   resolution. What no cache can supply is reported as unknown, never guessed and never
//!   defaulted: a count that is not a count is worse than no count.
//!
//! The modes differ in what they may read, never in what they mean. `fast_and_full_agree`
//! holds them to that.
//!
//! # What is canonical, and what is derived
//!
//! | Fact | Canonical source |
//! |---|---|
//! | project name, version, licence, summary | the crate manifest and [`crate::about`], at compile time |
//! | repository root, layer sections | `.ai/manifest.yaml`, through [`crate::repository::Repository`] |
//! | version control | one `git status --porcelain=v2 --branch` |
//! | toolchains | the manifest that declares each one |
//! | what the layer holds | the index, through the capability registry |
//! | workflows | `just --dump --dump-format json` |
//! | provider projections | the policy's `projections[]` |
//! | services | the capability registry's routes and the shared server's lease |
//!
//! Nothing in this module writes to the repository outside `.ai/local/`, and nothing in it
//! opens a socket to anything but the loopback address the lease names.
//!
//! # The lifecycle
//!
//! Build the inputs a caller already has, say what may be spent, take one snapshot, and
//! render it. The two resolutions are the same call with a different budget, and the
//! example is the assertion that they cannot drift apart: the contract, the project and
//! the provenance are the same, and only what the cheap one could not reach differs.
//!
//! ```
//! # let dir = tempfile::tempdir().expect("a temporary directory");
//! # std::fs::create_dir_all(dir.path().join(".ai/repo")).expect("the tracked half");
//! # std::fs::write(dir.path().join(".ai/manifest.yaml"), "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").expect("a manifest");
//! use majordomus_cli::environment::{resolve, EnvironmentQuery, Inputs, Resolution, TierState};
//! let repository = majordomus_cli::Repository::discover(dir.path()).expect("a repository");
//! let inputs = Inputs { repository: &repository, share: None, index: None, registry: None };
//!
//! let prompt = resolve(&inputs, &EnvironmentQuery::fast().sealed());
//! let command = resolve(&inputs, &EnvironmentQuery::full().sealed());
//! assert_eq!(prompt.schema, command.schema, "two resolutions, one contract");
//! assert_eq!(prompt.project, command.project, "and the cheap facts are the same facts");
//! assert_eq!(prompt.resolution, Resolution::Fast);
//! assert_eq!(command.resolution, Resolution::Full);
//!
//! assert_eq!(prompt.layer.state, TierState::Unavailable, "no index, and no cache to stand in");
//! assert!(prompt.explain("layer.objects").is_some(), "which is a fact with a source of its own");
//! ```

pub mod cache;
pub mod probe;
pub mod render;
pub mod resolve;
pub mod services;
pub mod shell;
pub mod text;
pub mod toolchain;
pub mod vcs;
pub mod workflows;

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::Diagnostic;

pub use cache::{Cache, CachedTier};
pub use resolve::{resolve, EnvironmentQuery, Inputs};
pub use vcs::{GitWorkingTree, VcsState};

/// The schema identity of [`RepositoryEnvironment`]. It travels in the snapshot itself, so
/// a document read from a cache, an API or a file says which contract it was written
/// under; [`SCHEMA_VERSION`] rises when a reader of the old shape would misread the new
/// one.
pub const SCHEMA: &str = "majordomus/repository-environment";

/// The version of [`SCHEMA`]. A cache entry written under a different version is not read.
pub const SCHEMA_VERSION: u32 = 1;

/// How completely a snapshot was resolved. Carried in the snapshot because a consumer must
/// be able to tell "there is no server running" from "nobody looked".
///
/// It says what was affordable and never what was found: a fast snapshot of a healthy
/// repository and a full snapshot of a broken one are both complete answers, and the
/// difference between them is in the tiers rather than here.
///
/// ```
/// use majordomus_cli::environment::Resolution;
/// assert_eq!(serde_json::to_string(&Resolution::Fast).expect("a mode serialises"), "\"fast\"");
/// assert_eq!(
///     serde_json::from_str::<Resolution>("\"full\"").expect("and deserialises"),
///     Resolution::Full
/// );
/// assert!(Resolution::Fast < Resolution::Full, "one is strictly less work than the other");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// Only what is cheap enough for a shell prompt; the rest from the cache.
    Fast,
    /// Everything, including the index. Writes the cache.
    Full,
}

impl Resolution {
    /// The word this mode is called by everywhere it is spoken about: in the serialised
    /// snapshot, in a diagnostic, and in the documentation.
    ///
    /// Written once here rather than formatted at each surface, because a banner that says
    /// `Fast` and an API that says `fast` are two names for one thing and somebody will
    /// eventually match on the wrong one.
    ///
    /// ```
    /// use majordomus_cli::environment::Resolution;
    /// assert_eq!(Resolution::Fast.as_str(), "fast");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Resolution::Fast => "fast",
            Resolution::Full => "full",
        }
    }
}

/// Where a tier of the snapshot came from. A tier the active resolution cannot reach is
/// [`TierState::Unavailable`] with the reason, and its values are absent rather than zero.
///
/// Three states and not a boolean, because "read just now" and "read by the last full
/// resolution" are both answers a reader may act on, while the third is the absence of an
/// answer. A surface that renders a tier is expected to show which of the three it has —
/// that is why the state travels with the values rather than beside them.
///
/// ```
/// use majordomus_cli::environment::{LayerSummary, TierState};
/// assert_eq!(serde_json::to_string(&TierState::Cached).expect("a state serialises"), "\"cached\"");
/// assert_eq!(LayerSummary::unavailable().state, TierState::Unavailable);
/// assert!(TierState::Resolved.is_known() && TierState::Cached.is_known());
/// assert!(!TierState::Unavailable.is_known(), "and only one of the three has no values");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TierState {
    /// Read from its canonical source during this resolution.
    Resolved,
    /// Taken from the cache written by an earlier full resolution.
    Cached,
    /// Neither available; the values are absent and the reason is a diagnostic.
    Unavailable,
}

impl TierState {
    /// Whether the tier has values at all, whichever of the two ways it got them.
    ///
    /// The question every renderer actually has: a cached count and a freshly read one are
    /// both numbers to draw, and the third state has nothing to draw. Asking this rather
    /// than matching on the variant is what keeps a surface from treating `Cached` as a
    /// failure — the mistake this method exists to make impossible.
    ///
    /// ```
    /// use majordomus_cli::environment::TierState;
    /// assert!(TierState::Resolved.is_known());
    /// assert!(TierState::Cached.is_known(), "a cached answer is still an answer");
    /// assert!(!TierState::Unavailable.is_known());
    /// ```
    pub fn is_known(self) -> bool {
        !matches!(self, TierState::Unavailable)
    }

    /// The word this state is called by in the serialised snapshot and in prose about it.
    ///
    /// One spelling for every surface, so that a Cockpit badge, a JSON field and a
    /// banner's legend cannot disagree about what to call the same state.
    ///
    /// ```
    /// use majordomus_cli::environment::TierState;
    /// assert_eq!(TierState::Resolved.as_str(), "resolved");
    /// assert_eq!(TierState::Unavailable.as_str(), "unavailable");
    /// let json = serde_json::to_string(&TierState::Cached).expect("a state serialises");
    /// assert_eq!(json, format!("\"{}\"", TierState::Cached.as_str()), "one spelling, not two");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            TierState::Resolved => "resolved",
            TierState::Cached => "cached",
            TierState::Unavailable => "unavailable",
        }
    }
}

/// What the project is, from the crate manifest and the prose written once in
/// [`crate::about`]. Every field here is a compile-time constant of this executable: there
/// is no file to read, nothing to parse, and nothing that can disagree with the binary
/// that answers.
///
/// It describes the *executable*, never the checkout it is pointed at: the commit is the
/// one this binary was built from, which is exactly what makes a stale build visible when
/// it disagrees with what version control reports for the work tree.
///
/// ```
/// use majordomus_cli::environment::ProjectIdentity;
/// let identity = ProjectIdentity::of_this_build();
/// assert_eq!(identity.version, majordomus_cli::VERSION);
/// assert_eq!(identity.target, majordomus_cli::TARGET, "the triple it was built for");
/// assert!(!identity.commit.is_empty(), "`unknown` outside a work tree, but never empty");
/// assert!(!identity.license.is_empty(), "and the SPDX identifier the crate declares");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectIdentity {
    /// The product name.
    pub name: String,
    /// The version of this executable, from the crate manifest.
    pub version: String,
    /// One sentence: what this is.
    pub summary: String,
    /// The SPDX licence identifier.
    pub license: String,
    /// The source repository.
    pub repository: String,
    /// The Rust target triple this executable was built for.
    pub target: String,
    /// The cargo profile it was built with.
    pub profile: String,
    /// The commit it was built from, or `unknown` outside a work tree.
    pub commit: String,
}

impl ProjectIdentity {
    /// The identity of this executable. Reads nothing.
    ///
    /// Every field is a constant baked in at compile time, so this cannot fail, cannot be
    /// slow, and cannot be affected by where it is called from. It is also the only
    /// constructor: there is no way to describe a *different* build, because a snapshot
    /// that named one would be describing something it cannot see.
    ///
    /// ```
    /// use majordomus_cli::environment::ProjectIdentity;
    /// assert_eq!(ProjectIdentity::of_this_build(), ProjectIdentity::of_this_build());
    /// assert_eq!(ProjectIdentity::of_this_build().name, "Majordomus");
    /// assert!(!ProjectIdentity::of_this_build().summary.is_empty());
    /// ```
    pub fn of_this_build() -> Self {
        ProjectIdentity {
            name: crate::about::NAME.into(),
            version: crate::VERSION.into(),
            summary: crate::about::SUMMARY.into(),
            license: crate::about::LICENSE.into(),
            repository: crate::about::REPOSITORY.into(),
            target: crate::TARGET.into(),
            profile: crate::PROFILE.into(),
            commit: crate::COMMIT.into(),
        }
    }
}

/// The checkout this snapshot is of.
///
/// Distinct from [`ProjectIdentity`] because one executable serves many checkouts: the
/// linked-work-tree flag and the local half are here rather than there for that reason,
/// and they are what a caller needs to know before it writes anything — a linked work
/// tree has its own local half and must not be given another's.
///
/// ```
/// # let dir = tempfile::tempdir().expect("a temporary directory");
/// # std::fs::create_dir_all(dir.path().join(".ai/repo")).expect("the tracked half");
/// # std::fs::write(dir.path().join(".ai/manifest.yaml"), "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").expect("a manifest");
/// # use majordomus_cli::environment::{resolve, EnvironmentQuery, Inputs};
/// # let repository = majordomus_cli::Repository::discover(dir.path()).expect("a repository");
/// # let inputs = Inputs { repository: &repository, share: None, index: None, registry: None };
/// use majordomus_cli::environment::RepositoryIdentity;
/// let snapshot = resolve(&inputs, &EnvironmentQuery::fast().sealed());
/// let checkout: &RepositoryIdentity = &snapshot.repository;
/// assert_eq!(checkout.layer_schema, "ai-repository/v1", "the manifest's own schema");
/// assert!(checkout.root.ends_with(&checkout.name), "the name is the root's last segment");
/// assert_eq!(
///     checkout.sections.get("policy").map(String::as_str),
///     Some(".ai/repo/policy.yaml"),
///     "the manifest declares it under .ai/, and this is repository-relative"
/// );
/// assert!(!checkout.linked_worktree, "a directory with a manifest and no git is the main one");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryIdentity {
    /// The name a person calls it: the base name of the root directory.
    pub name: String,
    /// The root, absolute. The same value `repository.info` reports.
    pub root: String,
    /// The manifest's `schema`, `ai-repository/v1`.
    pub layer_schema: String,
    /// Section name to repository-relative path, as the manifest declares them.
    pub sections: BTreeMap<String, String>,
    /// The checkout-local half of the layer, repository-relative. Never tracked, and
    /// where anything this checkout alone knows — the server's lease, this snapshot's
    /// cache — is kept.
    pub local_path: String,
    /// Whether this checkout is a linked work tree rather than the main one.
    pub linked_worktree: bool,
}

/// A toolchain the repository declares, and what is installed for it. The three are kept
/// apart on purpose: a repository can declare a version nobody has, and a machine can have
/// a version no repository asked for, and reporting either as the other is how a version
/// mismatch stays invisible for a week.
///
/// `declared_by` is not decoration: it is the file a person has to edit to change the
/// answer, and a snapshot that reported a version without it would leave them grepping.
///
/// ```
/// use majordomus_cli::environment::{ToolchainAvailability, ToolchainState};
/// let asked_for = ToolchainState {
///     id: "rust".into(),
///     title: "Rust".into(),
///     declared: Some("1.85".into()),
///     declared_by: "apps/majordomus-cli/Cargo.toml".into(),
///     installed: None,
///     availability: ToolchainAvailability::Unknown,
/// };
/// assert!(asked_for.installed.is_none(), "declared here says nothing about what is installed");
/// let json = serde_json::to_string(&asked_for).expect("a toolchain serialises");
/// assert!(!json.contains("installed"), "and a version nobody asked for is absent, not null");
/// assert!(json.contains("declared_by"), "while the file that declared it is always carried");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ToolchainState {
    /// A stable id, `[a-z][a-z0-9-]*`: `rust`, `node`.
    pub id: String,
    /// The short name a person reads.
    pub title: String,
    /// The version the repository declares, when it declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub declared: Option<String>,
    /// The repository-relative file that declares it.
    pub declared_by: String,
    /// The version installed here, when it could be asked and the answer is not stale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<String>,
    /// Where the installed version stands.
    pub availability: ToolchainAvailability,
}

/// Whether a declared toolchain is usable here.
///
/// `Unknown` is the state a fast resolution leaves behind and is not a soft `Missing`:
/// asking costs a process spawn, a shell prompt does not pay it, and a banner that showed
/// "not installed" because nobody asked would send a person looking for a problem that is
/// not there.
///
/// ```
/// use majordomus_cli::environment::ToolchainAvailability;
/// assert_eq!(
///     serde_json::to_string(&ToolchainAvailability::Missing).expect("it serialises"),
///     "\"missing\""
/// );
/// assert_ne!(
///     ToolchainAvailability::Unknown,
///     ToolchainAvailability::Missing,
///     "nobody asked is not the same answer as asked and not there"
/// );
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ToolchainAvailability {
    /// Installed, and its version answered.
    Installed,
    /// The executable is not on the path.
    Missing,
    /// Nobody asked: this resolution may not run a subprocess for it, and no cache held it.
    Unknown,
}

/// What the layer holds, counted per kind, plus the registry the executable composes.
/// Every number here comes from the index — the same objects every other surface serves —
/// and never from counting files that match a pattern: a file that does not parse is not
/// a rule, and a count that says otherwise is a lie a person acts on.
///
/// Every count is an `Option` for one reason: a fast resolution has no index and must be
/// able to say so. `objects: None` and `objects: Some(0)` are different claims — nobody
/// counted, against a layer with nothing in it — and only one of them is a reason to go
/// looking for a broken manifest.
///
/// ```
/// use majordomus_cli::environment::{KindCount, LayerSummary, TierState};
/// let counted = LayerSummary {
///     state: TierState::Resolved,
///     kinds: vec![KindCount { kind: "rule".into(), count: 87 }],
///     objects: Some(902),
///     capabilities: Some(934),
///     invalid: Some(0),
///     degraded: Some(false),
/// };
/// assert_eq!(counted.kind("rule"), Some(87));
/// assert_eq!(counted.kind("adr"), None, "a kind with no objects is absent from the counts");
///
/// let nobody_counted = LayerSummary::unavailable();
/// assert_eq!(nobody_counted.objects, None, "and this is not a repository with no objects");
/// assert!(!nobody_counted.state.is_known());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LayerSummary {
    /// Where these numbers came from.
    pub state: TierState,
    /// One entry per kind present, sorted by kind.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub kinds: Vec<KindCount>,
    /// How many objects the index holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub objects: Option<usize>,
    /// How many capabilities the registry holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<usize>,
    /// How many files the layer declared that did not become objects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid: Option<usize>,
    /// Whether the layer read cleanly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degraded: Option<bool>,
}

impl LayerSummary {
    /// The summary of a tier that could not be reached: the state that says so, and not a
    /// single value.
    ///
    /// This exists so that "no answer" has one spelling. `Default` would give the same
    /// shape with a state of `Resolved` and zeros where the counts belong, which is the
    /// exact lie this type is built to refuse — hence a named constructor and no `Default`
    /// at all.
    ///
    /// ```
    /// use majordomus_cli::environment::{LayerSummary, TierState};
    /// let nothing = LayerSummary::unavailable();
    /// assert_eq!(nothing.state, TierState::Unavailable);
    /// assert_eq!(nothing.objects, None);
    /// assert_eq!(nothing.capabilities, None);
    /// assert!(nothing.kinds.is_empty());
    /// assert_eq!(nothing.kind("rule"), None, "and no kind can be looked up in it");
    /// ```
    pub fn unavailable() -> Self {
        LayerSummary {
            state: TierState::Unavailable,
            kinds: Vec::new(),
            objects: None,
            capabilities: None,
            invalid: None,
            degraded: None,
        }
    }

    /// The count of one kind, when the kind is present.
    ///
    /// `None` covers both ways of having no number — the layer holds none of that kind,
    /// and nothing counted at all — because a caller that wants to tell them apart has
    /// [`LayerSummary::state`] for exactly that, and a caller that does not must not be
    /// handed a zero it will print.
    ///
    /// ```
    /// use majordomus_cli::environment::{KindCount, LayerSummary, TierState};
    /// let summary = LayerSummary {
    ///     state: TierState::Cached,
    ///     kinds: vec![
    ///         KindCount { kind: "rule".into(), count: 87 },
    ///         KindCount { kind: "adr".into(), count: 19 },
    ///     ],
    ///     objects: Some(106),
    ///     capabilities: None,
    ///     invalid: None,
    ///     degraded: None,
    /// };
    /// assert_eq!(summary.kind("adr"), Some(19));
    /// assert_eq!(summary.kind("skill"), None, "a kind that is not there has no count");
    /// ```
    pub fn kind(&self, id: &str) -> Option<usize> {
        self.kinds.iter().find(|k| k.kind == id).map(|k| k.count)
    }
}

/// How many valid objects of one kind the layer holds.
///
/// A pair rather than a map, because the order matters to a reader: the banner's kinds
/// line is ranked by count so that the largest is first, and a map would hand the
/// renderer whatever order its keys happened to have.
///
/// ```
/// use majordomus_cli::environment::KindCount;
/// let rules = KindCount { kind: "rule".into(), count: 87 };
/// let json = serde_json::to_string(&rules).expect("a count serialises");
/// assert_eq!(json, r#"{"kind":"rule","count":87}"#);
///
/// let mut counts = vec![KindCount { kind: "adr".into(), count: 19 }, rules.clone()];
/// majordomus_cli::order::canonical(&mut counts);
/// assert_eq!(counts[0], rules, "the largest count leads, whatever the kind is called");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct KindCount {
    /// The kind, as `share/kinds.yaml` declares it.
    pub kind: String,
    /// How many objects of it the index holds.
    pub count: usize,
}

// The kinds line ranks by count, largest first, and the workflow list reads by name. Both
// end on an identity, so neither depends on the order its source produced.
impl crate::order::Ordered for KindCount {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        // Negated: the canonical rank ascends, and this line wants the largest first.
        crate::order::OrderKey::plain(&self.kind, &self.kind).ranked(-(self.count as i64))
    }
}

/// The workflows a person can run here, as the workflow runner itself describes them.
///
/// The `entrypoints` are the only derived thing in it, and they are derived rather than
/// listed on purpose: the entry point of a group is the public recipe named after the
/// group, so a repository publishes a recommended command by naming a recipe and never by
/// registering it here. A catalogue that carried a hand-written list would outlive the
/// recipes it names.
///
/// ```
/// use majordomus_cli::environment::workflows;
/// use majordomus_cli::environment::{TierState, WorkflowCatalogue};
/// let dir = tempfile::tempdir().expect("a temporary directory");
/// let catalogue: WorkflowCatalogue = workflows::resolve(dir.path());
/// assert_eq!(catalogue.state, TierState::Unavailable, "no runner answered here");
/// assert!(catalogue.entrypoints.is_empty(), "so no command can be recommended either");
/// let json = serde_json::to_string(&catalogue).expect("a catalogue serialises");
/// assert!(!json.contains("workflows"), "and an empty catalogue carries no empty lists");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowCatalogue {
    /// Where these came from.
    pub state: TierState,
    /// The command that produced them, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Every public workflow, sorted by name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workflows: Vec<WorkflowDescriptor>,
    /// The entry point of each group, in the order the groups are declared: what a person
    /// new to the repository runs first. Derived, never listed anywhere.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entrypoints: Vec<WorkflowEntrypoint>,
}

impl WorkflowCatalogue {
    /// The catalogue of a repository whose runner could not be asked: the state that says
    /// so, no workflows, and no source quoted for them.
    ///
    /// It is the answer for a repository with no justfile, a machine with no `just`, and a
    /// justfile the runner refused — three different situations that are one fact here,
    /// because in all three there is nothing a person can be offered. `source` stays
    /// `None`: naming a command that did not answer would be quoting evidence that does
    /// not exist.
    ///
    /// ```
    /// use majordomus_cli::environment::{TierState, WorkflowCatalogue};
    /// let nothing = WorkflowCatalogue::unavailable();
    /// assert_eq!(nothing.state, TierState::Unavailable);
    /// assert!(nothing.workflows.is_empty() && nothing.entrypoints.is_empty());
    /// assert_eq!(nothing.source, None, "nothing produced it, so nothing is cited");
    /// ```
    pub fn unavailable() -> Self {
        WorkflowCatalogue {
            state: TierState::Unavailable,
            source: None,
            workflows: Vec::new(),
            entrypoints: Vec::new(),
        }
    }
}

/// One workflow: a recipe of the repository's `justfile`, as `just` describes it.
///
/// Nothing here is this executable's opinion. The description is the recipe's own doc
/// comment, the group is its `[group(...)]`, and `confirm` is its `[confirm]` attribute —
/// so a workflow's documentation lives beside the workflow and cannot go stale against a
/// second copy. The name carries the module for a recipe that is in one, because
/// `just build` and `just site build` are two different commands.
///
/// ```
/// use majordomus_cli::environment::workflows::workflows_of;
/// use majordomus_cli::environment::WorkflowDescriptor;
/// let document = serde_json::json!({
///     "recipes": {
///         "deploy": { "name": "deploy", "doc": "Publish the site.", "private": false,
///                     "attributes": [{"group": "release"}, "confirm"],
///                     "parameters": [], "dependencies": [{"recipe": "build"}] }
///     },
///     "modules": {}
/// });
/// let found: Vec<WorkflowDescriptor> = workflows_of(&document);
/// assert_eq!(found[0].description.as_deref(), Some("Publish the site."));
/// assert_eq!(found[0].group.as_deref(), Some("release"));
/// assert_eq!(found[0].dependencies, vec!["build".to_string()], "what it runs first");
/// assert!(found[0].confirm, "and that it asks before it does anything");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowDescriptor {
    /// The name a person types after `just`.
    pub name: String,
    /// The module path, for a recipe in an imported module; empty at the root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    /// The recipe's doc comment, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The `[group(...)]` it belongs to, when it declares one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Its parameters, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<WorkflowParameter>,
    /// The recipes it runs first, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    /// Whether it asks before it runs (`[confirm]`).
    pub confirm: bool,
}

// By the name a person types. The namespace is part of the identity rather than a group:
// `just build` and `just site::build` are two recipes, and the list is read as one.
impl crate::order::Ordered for WorkflowDescriptor {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        crate::order::OrderKey::plain(&self.name, self.namespace.as_deref().unwrap_or(&self.name))
    }
}

/// One parameter of a workflow, with the two facts that decide how it may be invoked:
/// whether it may be repeated, and whether it must be given at all.
///
/// The default value is deliberately not carried. A default is the runner's to apply, and
/// a surface that echoed one would be promising a value this executable never resolved;
/// what it does carry is the *consequence* of there being one, which is that the
/// parameter is optional.
///
/// ```
/// use majordomus_cli::environment::workflows::workflows_of;
/// use majordomus_cli::environment::WorkflowParameter;
/// let document = serde_json::json!({
///     "recipes": {
///         "test": { "name": "test", "doc": null, "private": false, "attributes": [],
///                   "dependencies": [],
///                   "parameters": [
///                       {"name": "suite", "kind": "singular", "default": null},
///                       {"name": "args", "kind": "star", "default": null}
///                   ] }
///     },
///     "modules": {}
/// });
/// let parameters: &[WorkflowParameter] = &workflows_of(&document)[0].parameters;
/// assert_eq!(parameters[0].name, "suite");
/// assert!(parameters[0].required, "a parameter with no default has to be given");
/// assert!(parameters[1].variadic, "and `*args` takes any number of words");
/// assert!(!parameters[1].required, "which includes none of them");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowParameter {
    /// The name.
    pub name: String,
    /// Whether it may be repeated (`*args`, `+args`).
    pub variadic: bool,
    /// Whether it must be given.
    pub required: bool,
}

/// The entry point of one group of workflows: what the group is, and the one workflow that
/// stands for it.
///
/// Entirely derived, and therefore never wrong for long: it exists because a group had a
/// public recipe named after it, and it stops existing the moment that recipe is renamed.
/// The `command` is rendered here so that a banner, a page and a Cockpit panel all print
/// the same words a person would type.
///
/// ```
/// use majordomus_cli::environment::WorkflowEntrypoint;
/// let entry = WorkflowEntrypoint {
///     group: "check".into(),
///     workflow: "check".into(),
///     command: "just check".into(),
///     description: Some("Every gate.".into()),
/// };
/// assert!(entry.command.ends_with(&entry.workflow), "the command runs the workflow it names");
/// assert_eq!(entry.group, entry.workflow, "which is the rule that made it an entry point");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorkflowEntrypoint {
    /// The group, as the justfile declares it.
    pub group: String,
    /// The workflow that stands for the group.
    pub workflow: String,
    /// The command a person types.
    pub command: String,
    /// The workflow's description, when it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// One provider projection the policy declares, and whether the file on disk still matches
/// what the policy renders.
///
/// The comparison is against a rendering done now, not against a recorded hash: a file
/// that was hand-edited and a policy that moved under it are the same fact to a reader —
/// the file no longer says what the policy says — and neither can hide behind a stale
/// record of what it used to be.
///
/// ```
/// use majordomus_cli::environment::{ProjectionState, ProviderState};
/// let agents = ProviderState {
///     id: "agents".into(),
///     target: "AGENTS.md".into(),
///     state: ProjectionState::Current,
///     always_loaded: true,
/// };
/// assert!(matches!(agents.state, ProjectionState::Current), "the file is what the policy says");
/// assert!(agents.always_loaded, "and this provider reads it into every context it opens");
/// assert!(!agents.target.starts_with('/'), "targets are repository-relative");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProviderState {
    /// The provider id, as the policy names it: `agents`, `claude-code`.
    pub id: String,
    /// The repository-relative file it renders to.
    pub target: String,
    /// Whether the file matches what the policy renders now.
    pub state: ProjectionState,
    /// Whether the provider loads this file into every context.
    pub always_loaded: bool,
}

/// Where one provider projection stands against its policy.
///
/// Four states, and the last two are not the same: `Absent` is a file to write, and
/// `Unknown` is a rendering that could not be attempted — usually because the
/// distribution directory was not located — where nothing at all can be said about the
/// file. Reporting the second as the first would tell a person to regenerate something
/// that may already be correct.
///
/// ```
/// use majordomus_cli::environment::ProjectionState;
/// assert_eq!(serde_json::to_string(&ProjectionState::Stale).expect("it serialises"), "\"stale\"");
/// assert_ne!(
///     ProjectionState::Absent,
///     ProjectionState::Unknown,
///     "a file that is not there is not a file nobody could render"
/// );
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ProjectionState {
    /// The file matches the rendering byte for byte.
    Current,
    /// The file differs: it was hand-edited, or the policy moved under it.
    Stale,
    /// The file is not there.
    Absent,
    /// It could not be rendered, so nothing can be said.
    Unknown,
}

/// One local service of this repository: what it is, where it is, and whether anything
/// answers there. The path is the one the router serves; the URL exists only while a
/// server does.
///
/// The path and the URL are two different facts and are kept apart for that reason: the
/// path is a compile-time constant of this executable and is true of a repository nobody
/// is serving, while the URL is a running process's published address and is absent the
/// moment there is none. A surface may always name the first and may only link the second.
///
/// ```
/// use majordomus_cli::environment::{ServiceAvailability, ServiceState};
/// let cockpit = ServiceState {
///     id: "cockpit".into(),
///     title: "Cockpit".into(),
///     path: "/cockpit".into(),
///     url: None,
///     availability: ServiceAvailability::NotRunning,
/// };
/// assert!(cockpit.path.starts_with('/'), "the route exists whether or not a server does");
/// let json = serde_json::to_string(&cockpit).expect("a service serialises");
/// assert!(!json.contains("url"), "an address nothing published is absent, never null");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ServiceState {
    /// A stable id, `[a-z][a-z0-9-]*`.
    pub id: String,
    /// The short name a person reads.
    pub title: String,
    /// The absolute path the router serves it under.
    pub path: String,
    /// The full URL, when a server is running and published its address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Whether anything answers there.
    pub availability: ServiceAvailability,
}

/// Whether a service answers. `Unknown` is a real answer and never a disguised `no`: a
/// probe that timed out and a port that refused the connection lead a reader to different
/// actions.
///
/// `NotRunning` is a measurement — nobody holds the lease, or the port refused the
/// connection — and `Unknown` is the absence of one, which is what a resolution that may
/// not open a socket reports. Telling a person that their server is down because nobody
/// was allowed to look would be the same defect the whole snapshot is built to avoid.
///
/// ```
/// use majordomus_cli::environment::ServiceAvailability;
/// assert_eq!(
///     serde_json::to_string(&ServiceAvailability::NotRunning).expect("it serialises"),
///     "\"not_running\""
/// );
/// assert_ne!(
///     ServiceAvailability::Unknown,
///     ServiceAvailability::NotRunning,
///     "a probe that never happened has not found the port free"
/// );
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ServiceAvailability {
    /// The address accepted a connection.
    Available,
    /// No server holds the repository's lease, or the address refused the connection.
    NotRunning,
    /// The probe did not finish in its budget, or this resolution did not probe.
    Unknown,
}

/// Where one field of the snapshot came from. This is what makes an inferred system
/// debuggable: every fact can name the thing that decided it, so "why does it say that"
/// is answered by the tool rather than by reading its source.
///
/// A field that resolved to nothing still gets an entry, and that is the interesting half:
/// `source` then says what was *tried*, so an absent count is traceable to the reason it
/// is absent rather than looking like a field somebody forgot to fill in.
///
/// ```
/// use majordomus_cli::environment::{Confidence, FieldSource};
/// let read = FieldSource::exact(
///     "vcs.branch",
///     Some("master".into()),
///     "git status --porcelain=v2 --branch",
///     "environment::vcs",
/// );
/// assert_eq!(read.confidence, Confidence::Exact);
/// assert!(read.source.starts_with("git status"), "the command that decided it, quoted");
///
/// let missing = FieldSource::unknown(
///     "layer.objects",
///     "a fast resolution does not build the index, and no cache entry matched",
///     "environment::cache",
/// );
/// assert_eq!(missing.value, None, "no value, and still a source for the absence");
/// assert_eq!(missing.confidence, Confidence::Unknown);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FieldSource {
    /// The field, in dotted form: `services.cockpit.url`, `vcs.branch`.
    pub field: String,
    /// The value as it appears in the snapshot, rendered for a person; absent for a field
    /// that resolved to nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// What decided it: a file, a command, a compile-time constant, the cache.
    pub source: String,
    /// The part of this executable that read it.
    pub resolver: String,
    /// How certain the value is.
    pub confidence: Confidence,
}

impl FieldSource {
    /// A field resolved exactly from a named source: read now, from the thing that
    /// decides it.
    ///
    /// `source` is what the reader is told — a file, a command, a compile-time
    /// constant — and `resolver` is which part of this executable read it. Two fields,
    /// because "why does it say that" and "who is responsible for it" are different
    /// questions, and answering only the second is how provenance becomes decoration.
    ///
    /// ```
    /// use majordomus_cli::environment::{Confidence, FieldSource};
    /// let entry = FieldSource::exact(
    ///     "project.version",
    ///     Some(majordomus_cli::VERSION.to_string()),
    ///     "the crate manifest, at compile time (CARGO_PKG_VERSION)",
    ///     "environment::resolve",
    /// );
    /// assert_eq!(entry.field, "project.version", "dotted, as a person would ask for it");
    /// assert_eq!(entry.confidence, Confidence::Exact);
    /// assert_eq!(entry.resolver, "environment::resolve");
    /// ```
    pub fn exact(
        field: impl Into<String>,
        value: Option<String>,
        source: impl Into<String>,
        resolver: &'static str,
    ) -> Self {
        FieldSource {
            field: field.into(),
            value,
            source: source.into(),
            resolver: resolver.into(),
            confidence: Confidence::Exact,
        }
    }

    /// A field taken from an earlier full resolution rather than read now.
    ///
    /// The value may be perfectly correct; what this records is that nobody checked it
    /// during this resolution. A reader deciding whether to act on a number — or whether
    /// to run `majordomus env status` first — needs that distinction, and it is the only
    /// place in the snapshot where it is kept.
    ///
    /// ```
    /// use majordomus_cli::environment::{Confidence, FieldSource};
    /// let entry = FieldSource::cached(
    ///     "layer.objects",
    ///     Some("902".into()),
    ///     "state/environment/snapshot.json, written by the last full resolution",
    ///     "environment::cache",
    /// );
    /// assert_eq!(entry.confidence, Confidence::Cached, "believed, not verified");
    /// assert_ne!(entry.confidence, Confidence::Exact);
    /// assert!(entry.source.contains("snapshot.json"), "and it names the file it came from");
    /// ```
    pub fn cached(
        field: impl Into<String>,
        value: Option<String>,
        source: impl Into<String>,
        resolver: &'static str,
    ) -> Self {
        FieldSource {
            field: field.into(),
            value,
            source: source.into(),
            resolver: resolver.into(),
            confidence: Confidence::Cached,
        }
    }

    /// A field nothing could resolve; `source` says what was tried.
    ///
    /// There is no `value` parameter, because there is no value: the constructor makes it
    /// impossible to record an unknown field with something in it, which is the one way
    /// this half of the snapshot could start lying.
    ///
    /// ```
    /// use majordomus_cli::environment::{Confidence, FieldSource};
    /// let entry = FieldSource::unknown(
    ///     "vcs.ahead_behind",
    ///     "the branch tracks no upstream",
    ///     "environment::vcs",
    /// );
    /// assert_eq!(entry.value, None);
    /// assert_eq!(entry.confidence, Confidence::Unknown);
    /// assert_eq!(entry.source, "the branch tracks no upstream", "why, not what");
    /// ```
    pub fn unknown(
        field: impl Into<String>,
        source: impl Into<String>,
        resolver: &'static str,
    ) -> Self {
        FieldSource {
            field: field.into(),
            value: None,
            source: source.into(),
            resolver: resolver.into(),
            confidence: Confidence::Unknown,
        }
    }
}

/// How far a resolved value can be trusted.
///
/// The same three levels as [`TierState`], and deliberately a separate type: a tier is a
/// section of the snapshot and this is one field of it, so a cached tier can still carry a
/// field that was read exactly. The variants are declared best-first, so the derived
/// ordering runs from the most to the least trustworthy.
///
/// ```
/// use majordomus_cli::environment::Confidence;
/// assert_eq!(
///     serde_json::to_string(&Confidence::Cached).expect("it serialises"),
///     "\"cached\""
/// );
/// assert!(Confidence::Exact < Confidence::Cached, "ordered by how far it can be trusted");
/// assert!(Confidence::Cached < Confidence::Unknown);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Read from its canonical source during this resolution.
    Exact,
    /// Read from a cache whose fingerprint still matches its inputs.
    Cached,
    /// Not resolved.
    Unknown,
}

/// The snapshot: what this checkout is, right now.
///
/// Every surface that reports any of this renders this value. Collections are in a
/// documented, stable order — kinds and workflows by name, services and providers in the
/// order their canonical source declares them, diagnostics in the order they were found —
/// so that two snapshots of the same repository serialise identically apart from
/// [`RepositoryEnvironment::generated_at`].
///
/// It is a report and not a handle: nothing on it reads anything, so a snapshot can be
/// serialised, cached, sent over HTTP and rendered somewhere else, and every surface that
/// does so is looking at the same facts. The schema travels inside it for that reason —
/// a document that has left this process must still be able to say what it is.
///
/// ```
/// # let dir = tempfile::tempdir().expect("a temporary directory");
/// # std::fs::create_dir_all(dir.path().join(".ai/repo")).expect("the tracked half");
/// # std::fs::write(dir.path().join(".ai/manifest.yaml"), "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").expect("a manifest");
/// use majordomus_cli::environment::{resolve, EnvironmentQuery, Inputs, RepositoryEnvironment};
/// # let repository = majordomus_cli::Repository::discover(dir.path()).expect("a repository");
/// # let inputs = Inputs { repository: &repository, share: None, index: None, registry: None };
/// let snapshot = resolve(&inputs, &EnvironmentQuery::fast().sealed());
///
/// assert_eq!(snapshot.schema, RepositoryEnvironment::schema_id(), "it names its own contract");
/// assert!(snapshot.generated_at.ends_with('Z'), "and the moment it was taken, in UTC");
///
/// let text = serde_json::to_string(&snapshot).expect("a snapshot serialises");
/// let read: RepositoryEnvironment = serde_json::from_str(&text).expect("and comes back");
/// assert_eq!(read, snapshot, "a document that has left this process says the same thing");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryEnvironment {
    /// The contract this document follows, `majordomus/repository-environment/v1`.
    pub schema: String,
    /// When it was taken, RFC 3339 in UTC.
    pub generated_at: String,
    /// How completely it was resolved.
    pub resolution: Resolution,
    /// What the project is.
    pub project: ProjectIdentity,
    /// What the checkout is.
    pub repository: RepositoryIdentity,
    /// What version control says.
    pub vcs: VcsState,
    /// The toolchains the repository declares, sorted by id.
    pub toolchains: Vec<ToolchainState>,
    /// What the layer holds.
    pub layer: LayerSummary,
    /// The workflows a person can run.
    pub workflows: WorkflowCatalogue,
    /// The provider projections the policy declares, in the policy's order.
    pub providers: Vec<ProviderState>,
    /// The local services, in the order the service table declares them.
    pub services: Vec<ServiceState>,
    /// Everything that went wrong or is worth knowing, in the order it was found.
    pub diagnostics: Vec<Diagnostic>,
    /// Where every field came from.
    pub provenance: Vec<FieldSource>,
}

impl RepositoryEnvironment {
    /// The schema identity a document of this shape carries: [`SCHEMA`] with
    /// [`SCHEMA_VERSION`] appended.
    ///
    /// Composed here rather than written out anywhere, so that raising the version is one
    /// edit and cannot leave a surface announcing a contract it is not serving. It is also
    /// what a cache entry is checked against before it is read back into these types.
    ///
    /// ```
    /// use majordomus_cli::environment::{RepositoryEnvironment, SCHEMA, SCHEMA_VERSION};
    /// assert_eq!(RepositoryEnvironment::schema_id(), "majordomus/repository-environment/v1");
    /// assert!(RepositoryEnvironment::schema_id().starts_with(SCHEMA));
    /// assert!(RepositoryEnvironment::schema_id().ends_with(&format!("v{SCHEMA_VERSION}")));
    /// ```
    pub fn schema_id() -> String {
        format!("{SCHEMA}/v{SCHEMA_VERSION}")
    }

    /// The provenance of one field, by its dotted name.
    ///
    /// The dotted name is the one a person types — `vcs.branch`, `layer.objects` — and not
    /// a Rust path, so that `majordomus env explain` and this method ask the same question
    /// the same way. A field with no entry is `None` rather than an invented one: the
    /// provenance covers the facts the resolver decided, and a name that is not among them
    /// has nothing to be explained.
    ///
    /// ```
    /// # let dir = tempfile::tempdir().expect("a temporary directory");
    /// # std::fs::create_dir_all(dir.path().join(".ai/repo")).expect("the tracked half");
    /// # std::fs::write(dir.path().join(".ai/manifest.yaml"), "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").expect("a manifest");
    /// # use majordomus_cli::environment::{resolve, EnvironmentQuery, Inputs};
    /// # let repository = majordomus_cli::Repository::discover(dir.path()).expect("a repository");
    /// # let inputs = Inputs { repository: &repository, share: None, index: None, registry: None };
    /// # let snapshot = resolve(&inputs, &EnvironmentQuery::fast().sealed());
    /// let version = snapshot.explain("project.version").expect("every fact has a source");
    /// assert_eq!(version.value.as_deref(), Some(majordomus_cli::VERSION));
    /// assert_eq!(version.resolver, "environment::resolve");
    /// assert!(snapshot.explain("project.favourite_colour").is_none(), "and nothing else does");
    /// ```
    pub fn explain(&self, field: &str) -> Option<&FieldSource> {
        self.provenance.iter().find(|p| p.field == field)
    }

    /// A digest of everything a reader would notice, excluding the moment it was taken.
    /// Two snapshots with the same digest say the same thing, which is how the banner
    /// decides whether a person has already seen this.
    ///
    /// Three things are deliberately outside it: the timestamp, the resolution, and the
    /// provenance. The timestamp changes on every call; the resolution is how the answer
    /// was obtained rather than what it says, so a warm fast snapshot must digest the same
    /// as the full one it came from, or every `cd` would look like news; and the provenance
    /// is about the facts rather than being one.
    ///
    /// ```
    /// # let dir = tempfile::tempdir().expect("a temporary directory");
    /// # std::fs::create_dir_all(dir.path().join(".ai/repo")).expect("the tracked half");
    /// # std::fs::write(dir.path().join(".ai/manifest.yaml"), "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").expect("a manifest");
    /// # use majordomus_cli::environment::{resolve, EnvironmentQuery, Inputs};
    /// # let repository = majordomus_cli::Repository::discover(dir.path()).expect("a repository");
    /// # let inputs = Inputs { repository: &repository, share: None, index: None, registry: None };
    /// # let snapshot = resolve(&inputs, &EnvironmentQuery::fast().sealed());
    /// let mut later = snapshot.clone();
    /// later.generated_at = "2099-01-01T00:00:00Z".into();
    /// assert_eq!(snapshot.digest(), later.digest(), "the moment it was taken is not news");
    ///
    /// let mut changed = snapshot.clone();
    /// changed.diagnostics.push(majordomus_cli::Diagnostic::warning(
    ///     "toolchain_missing",
    ///     None,
    ///     "Rust is declared here and is not installed",
    /// ));
    /// assert_ne!(snapshot.digest(), changed.digest(), "and anything a person would read is");
    /// ```
    pub fn digest(&self) -> String {
        let mut stable = self.clone();
        stable.generated_at = String::new();
        // The resolution is not part of what a reader sees: the same repository resolved
        // fast and full must digest the same, or every warm banner would look like news.
        stable.resolution = Resolution::Full;
        stable.provenance.clear();
        let text = serde_json::to_string(&stable).unwrap_or_default();
        crate::policy::sha256_hex(&text)
    }

    /// The one service with this id, or `None` when this snapshot describes no such
    /// service.
    ///
    /// The ids are this executable's own and stable — `cockpit`, `docs`, `swagger`, `api`,
    /// `events`, `openapi`, `mcp`, `index` — and looking one up by id is what keeps a
    /// caller from depending on the order the service table happens to declare them in.
    /// Every service is described whether or not anything is serving, so a `None` here
    /// means the id is wrong rather than that the server is down.
    ///
    /// ```
    /// # let dir = tempfile::tempdir().expect("a temporary directory");
    /// # std::fs::create_dir_all(dir.path().join(".ai/repo")).expect("the tracked half");
    /// # std::fs::write(dir.path().join(".ai/manifest.yaml"), "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").expect("a manifest");
    /// # use majordomus_cli::environment::{resolve, EnvironmentQuery, Inputs};
    /// # let repository = majordomus_cli::Repository::discover(dir.path()).expect("a repository");
    /// # let inputs = Inputs { repository: &repository, share: None, index: None, registry: None };
    /// # let snapshot = resolve(&inputs, &EnvironmentQuery::fast().sealed());
    /// let home = snapshot.service("index").expect("the home page is always described");
    /// assert_eq!(home.path, "/");
    /// assert_eq!(home.url, None, "nothing is serving this fixture, so it has no address");
    /// assert!(snapshot.service("gopher").is_none(), "and an id nothing declares has no service");
    /// ```
    pub fn service(&self, id: &str) -> Option<&ServiceState> {
        self.services.iter().find(|s| s.id == id)
    }

    /// The worst thing in the snapshot worth telling a person about, when there is one.
    ///
    /// The banner has room for one line, so this picks it: the highest severity present,
    /// and nothing at all when the worst is merely informational. That last filter is the
    /// point — a snapshot always carries something to say, and a prompt that reported "the
    /// layer was not counted" on every `cd` would train a person to stop reading it.
    ///
    /// ```
    /// # let dir = tempfile::tempdir().expect("a temporary directory");
    /// # std::fs::create_dir_all(dir.path().join(".ai/repo")).expect("the tracked half");
    /// # std::fs::write(dir.path().join(".ai/manifest.yaml"), "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").expect("a manifest");
    /// # use majordomus_cli::environment::{resolve, EnvironmentQuery, Inputs};
    /// # let repository = majordomus_cli::Repository::discover(dir.path()).expect("a repository");
    /// # let inputs = Inputs { repository: &repository, share: None, index: None, registry: None };
    /// use majordomus_cli::Diagnostic;
    /// let mut snapshot = resolve(&inputs, &EnvironmentQuery::fast().sealed());
    ///
    /// snapshot.diagnostics = vec![Diagnostic::info("nothing_to_do", None, "a note")];
    /// assert!(snapshot.headline_diagnostic().is_none(), "information is not news");
    ///
    /// snapshot.diagnostics.push(Diagnostic::warning("projection_stale", None, "AGENTS.md is stale"));
    /// snapshot.diagnostics.push(Diagnostic::error("layer_degraded", None, "a file did not parse"));
    /// let worst = snapshot.headline_diagnostic().expect("something is wrong here");
    /// assert_eq!(worst.code, "layer_degraded", "the worst of them, not the first or the last");
    /// ```
    pub fn headline_diagnostic(&self) -> Option<&Diagnostic> {
        self.diagnostics
            .iter()
            .max_by_key(|d| d.severity)
            .filter(|d| d.severity != crate::model::Severity::Info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_schema_identity_carries_its_version() {
        assert_eq!(
            RepositoryEnvironment::schema_id(),
            "majordomus/repository-environment/v1"
        );
    }

    #[test]
    fn a_tier_nothing_resolved_reports_no_values_rather_than_zero() {
        let s = LayerSummary::unavailable();
        assert_eq!(s.state, TierState::Unavailable);
        assert!(!s.state.is_known());
        // The distinction this type exists for: "no rules" and "nobody counted" must not
        // serialise the same, because a person reads the first as a broken layer.
        assert_eq!(s.objects, None);
        assert_eq!(s.kind("rule"), None);
    }

    #[test]
    fn the_identity_of_this_build_reads_nothing() {
        let p = ProjectIdentity::of_this_build();
        assert_eq!(p.version, crate::VERSION);
        assert_eq!(p.name, crate::about::NAME);
        assert!(!p.target.is_empty());
    }
}
