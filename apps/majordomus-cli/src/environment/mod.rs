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
    /// The word as serialised.
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
    /// Did the tier produce values?
    pub fn is_known(self) -> bool {
        !matches!(self, TierState::Unavailable)
    }

    /// The word as serialised.
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
    /// A summary nothing could resolve.
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
    pub fn kind(&self, id: &str) -> Option<usize> {
        self.kinds.iter().find(|k| k.kind == id).map(|k| k.count)
    }
}

/// How many valid objects of one kind the layer holds.
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
    /// A catalogue nothing could resolve.
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

/// One parameter of a workflow.
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
    /// A field resolved exactly from a named source.
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
    /// The schema identity a document of this shape carries.
    pub fn schema_id() -> String {
        format!("{SCHEMA}/v{SCHEMA_VERSION}")
    }

    /// The provenance of one field, by its dotted name.
    ///
    /// ```
    /// use majordomus_cli::environment::{FieldSource, RepositoryEnvironment};
    /// # fn demo(env: &RepositoryEnvironment) {
    /// let _ = env.explain("project.version");
    /// # }
    /// ```
    pub fn explain(&self, field: &str) -> Option<&FieldSource> {
        self.provenance.iter().find(|p| p.field == field)
    }

    /// A digest of everything a reader would notice, excluding the moment it was taken.
    /// Two snapshots with the same digest say the same thing, which is how the banner
    /// decides whether a person has already seen this.
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

    /// The digest that decides whether a banner is news: [`Self::digest`] without the
    /// services.
    ///
    /// The entry file watches the server's lease, so a server coming up re-evaluates entry
    /// twice more — once when it claims the lease, once when it publishes its address — and
    /// a digest that counted the services made each of those a first look, drawing the whole
    /// box again at a person who had just seen it. A server arriving or leaving is not a
    /// different repository; the short form names the Cockpit when it answers.
    ///
    /// ```
    /// use majordomus_cli::environment::RepositoryEnvironment;
    /// # fn demo(env: &RepositoryEnvironment) {
    /// // a server arriving or leaving is not a different repository, so clearing the
    /// // services changes nothing this digest can see
    /// let mut without = env.clone();
    /// without.services.clear();
    /// assert_eq!(env.news_digest(), without.news_digest());
    /// // and with no services left, the two digests are the same value
    /// assert_eq!(without.news_digest(), without.digest());
    /// # }
    /// ```
    pub fn news_digest(&self) -> String {
        let mut stable = self.clone();
        stable.services.clear();
        stable.digest()
    }

    /// The service with this id.
    pub fn service(&self, id: &str) -> Option<&ServiceState> {
        self.services.iter().find(|s| s.id == id)
    }

    /// The worst thing in the snapshot worth telling a person about, when there is one.
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
