//! Release identity: one model of what this repository is, what it published, and what it
//! would have to publish next.
//!
//! ```text
//!   Cargo.toml version ─────┐
//!   .ai/repo/releases/*.yaml│
//!   .ai/repo/changes/*.md   ├──▶ ReleaseState ──▶ CLI · API · OpenAPI · MCP · Cockpit
//!   contract snapshot ──────┤                     · CHANGELOG.md · release notes
//!   baseline snapshot (git) │                     · installer metadata · site
//!   .ai/repo/deployments/   ┘
//! ```
//!
//! # The four versions
//!
//! They are not one thing and this module refuses to pretend they are. Collapsing them is
//! how a page says `0.9.0` while the machine serving it runs `0.8.4`:
//!
//! | | what it is | where it comes from |
//! |---|---|---|
//! | source | what this tree would release | the crate manifest, compiled in |
//! | published | what an unpinned installation resolves to | the highest stable release record |
//! | running | what this process is | the same crate manifest, of the build that is running |
//! | deployed | what a hosted service reports | the deployment records, and the service itself |
//!
//! A [`VersionReport`] carries all four and names the ones that disagree. The Cockpit's
//! version display is a rendering of it, so a divergence is visible where a person already
//! looks rather than in a command nobody runs.
//!
//! # What is derived and what is authored
//!
//! Everything except two things. The crate's version is authored, once, and is the
//! authority; a change record's prose is authored, once, because no machine knows why a
//! break was worth making. The required bump, the minimum version, the changelog, the
//! release notes, the manifest, the readiness verdict and every projection of them are
//! derived from those two and from the repository's own history.

pub mod change;
pub mod changelog;
pub mod contract;
pub mod diff;
pub mod manifest;
pub mod version;

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::distribution::release::Releases;
use crate::distribution::Model as DistributionModel;
use crate::Index;

use change::Changes;
use changelog::Changelog;
use contract::{ContractSnapshot, Surface};
use diff::{CompatibilityImpact, ContractDiff};
use version::{Bump, Version};

/// Where the committed contract snapshot lives.
///
/// A generated artifact like every other, committed at every commit, which is what makes
/// the snapshot at a release tag readable months later without rebuilding that release.
pub const SNAPSHOT_PATH: &str = "docs/generated/contract.json";

/// Where the generated changelog lives.
pub const CHANGELOG_PATH: &str = "CHANGELOG.md";

/// The directory a release manifest lives in.
pub const MANIFEST_DIR: &str = "docs/generated/releases";

/// Where a release manifest's JSON encoding lives, by tag.
pub fn manifest_path(tag: &str) -> String {
    format!("{MANIFEST_DIR}/{tag}.json")
}

/// Which release a contract is compared against, and why that one.
///
/// Baseline selection is stated here and nowhere else. A shell script that picked the
/// latest tag with `sort -V` would be a second policy, and the two would disagree on the
/// day a pre-release was tagged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case")]
#[schemars(rename = "ReleaseBaseline")]
pub enum Baseline {
    /// A stable, unwithdrawn release whose tag resolves in this clone and whose contract
    /// snapshot is readable at that tag.
    Resolved {
        /// The version.
        version: Version,
        /// The tag.
        tag: String,
        /// The commit the record says it was built from.
        commit: String,
        /// The fingerprint of the contract as it stood there.
        fingerprint: String,
    },
    /// A stable release is recorded, and it was published before this repository recorded
    /// a contract at all. Nothing is wrong: a release is complete over the set of things
    /// that existed on the day it went out, and no release before the contract snapshot
    /// existed can be faulted for not carrying one.
    ///
    /// The consequence is real and is reported: there is nothing to measure against, so
    /// the *first* release after the snapshot appears carries no computed impact. Every
    /// release after that one does.
    Predates {
        /// The version.
        version: Version,
        /// The tag.
        tag: String,
    },
    /// A stable release is recorded and its contract cannot be read at its tag, for a
    /// reason that is not "it predates the snapshot": the tag is not in this clone, or the
    /// document there is not readable.
    ///
    /// This one is wrong. The version is known, so a minimum version is still computable,
    /// but the impact is not, and a report says so rather than reporting `none` and letting
    /// a break through.
    Unreadable {
        /// The version.
        version: Version,
        /// The tag.
        tag: String,
        /// Why the contract could not be read there.
        reason: String,
    },
    /// No stable release has ever been recorded. The first release is not compared against
    /// anything, carries no required bump, and is whatever the tree says it is.
    Initial,
}

impl Baseline {
    /// The version, when there is one.
    pub fn version(&self) -> Option<&Version> {
        match self {
            Self::Resolved { version, .. }
            | Self::Predates { version, .. }
            | Self::Unreadable { version, .. } => Some(version),
            Self::Initial => None,
        }
    }

    /// The tag, when there is one.
    pub fn tag(&self) -> Option<&str> {
        match self {
            Self::Resolved { tag, .. }
            | Self::Predates { tag, .. }
            | Self::Unreadable { tag, .. } => Some(tag),
            Self::Initial => None,
        }
    }

    /// One line for a person: which release this is measured against.
    pub fn summary(&self) -> String {
        match self {
            Self::Resolved { tag, .. } => format!("{tag}, contract readable"),
            Self::Predates { tag, .. } => {
                format!("{tag}, published before this repository recorded a contract")
            }
            Self::Unreadable { tag, reason, .. } => format!("{tag}, contract unreadable: {reason}"),
            Self::Initial => "none: no stable release is recorded".into(),
        }
    }
}

/// How far along a release is.
///
/// A state machine and not a set of booleans: `is_released` beside `has_tag` beside
/// `is_deployed` admits combinations that cannot happen, and every reader of them has to
/// know which. Each state here is reached from exactly one predecessor, and a report says
/// which state it is in and what would move it on.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "ReleaseReadiness")]
pub enum Readiness {
    /// The tree's version equals the published one and the contract has not moved: there
    /// is nothing to release.
    NothingToRelease,
    /// The contract moved and the tree's version does not yet clear the minimum the change
    /// requires. `majordomus release prepare` is the next step.
    BumpRequired,
    /// The version clears the minimum and something the release needs is missing — a
    /// change record for a break, a migration document, a stale generated artifact.
    Blocked,
    /// Everything the repository can check holds, and nothing has been tagged.
    ReadyToTag,
    /// A tag exists for this version and no release record does: the pipeline has not
    /// finished, or did not run.
    Tagged,
    /// A release record exists for this version. What is published is what the record says.
    Published,
}

impl Readiness {
    /// The word a report prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NothingToRelease => "nothing-to-release",
            Self::BumpRequired => "bump-required",
            Self::Blocked => "blocked",
            Self::ReadyToTag => "ready-to-tag",
            Self::Tagged => "tagged",
            Self::Published => "published",
        }
    }

    /// Whether this state stops a release from going out.
    pub fn is_blocking(self) -> bool {
        matches!(self, Self::BumpRequired | Self::Blocked)
    }
}

/// One thing wrong, with the command that would show it and the one that would fix it.
///
/// The codes are stable: a script may match on them, and a rule's failure message may
/// quote one. Nothing else in the repository defines a release diagnostic code.
///
/// The severity is the layer's own three, and only `Error` stops a release. The
/// distinction earns its place immediately: a baseline published before this repository
/// recorded a contract is a fact about history that nothing can fix, and reporting it as a
/// failure would leave the gate red forever and teach everybody to ignore it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseDiagnostic")]
pub struct Diagnostic {
    /// Whether this stops a release.
    pub severity: crate::model::Severity,
    /// The stable code, `SCREAMING_SNAKE_CASE`.
    pub code: String,
    /// What is wrong, in one line.
    pub message: String,
    /// The command that shows it in full, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reproduce: Option<String>,
    /// The command or edit that would change it, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

impl Diagnostic {
    /// A diagnostic that stops a release.
    pub fn error(
        code: &str,
        message: impl Into<String>,
        reproduce: Option<&str>,
        next: Option<String>,
    ) -> Self {
        Self {
            severity: crate::model::Severity::Error,
            code: code.into(),
            message: message.into(),
            reproduce: reproduce.map(str::to_string),
            next,
        }
    }

    /// A diagnostic a reader should know about and nobody can act on.
    pub fn info(code: &str, message: impl Into<String>, reproduce: Option<&str>) -> Self {
        Self {
            severity: crate::model::Severity::Info,
            code: code.into(),
            message: message.into(),
            reproduce: reproduce.map(str::to_string),
            next: None,
        }
    }

    /// Whether this stops a release.
    pub fn is_blocking(&self) -> bool {
        self.severity == crate::model::Severity::Error
    }
}

/// Where the impact a version was computed from came from.
///
/// Kept beside the impact rather than folded into it, because a reader deciding whether to
/// trust a verdict needs to know which of the two it is: a measured contract diff is
/// evidence, and a record's claim is a statement somebody made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ImpactSource {
    /// Measured: the public contract at the baseline against the contract now.
    Contract,
    /// Claimed: the strongest impact the unreleased change records state, used because the
    /// contract could not be measured.
    Records,
}

impl ImpactSource {
    /// The word a report prints.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Contract => "contract",
            Self::Records => "records",
        }
    }
}

/// The four versions and whether they agree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "VersionReport")]
pub struct VersionReport {
    /// What this tree would release: the crate's version, compiled in.
    pub source: Version,
    /// What an unpinned installation resolves to, when a stable release is recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published: Option<Version>,
    /// What the process answering this is.
    pub running: Version,
    /// What each declared deployment reports, when anything says.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deployed: Vec<DeployedVersion>,
    /// True when the source version is ahead of the published one: the tree holds a
    /// release that has not gone out.
    pub unreleased: bool,
    /// Every disagreement worth a person's attention, in the order they matter.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub divergences: Vec<String>,
}

/// What one deployment reports it is running.
///
/// The version is `None` until something asks the service, which nothing in this process
/// does: a version display must render from local state without a network call, and a
/// deployment whose version is unknown says `unknown` rather than `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "DeployedVersion")]
pub struct DeployedVersion {
    /// The deployment's id, as its record names it.
    pub deployment: String,
    /// The environment.
    pub environment: String,
    /// What the record says its state is.
    pub status: String,
    /// The version the service reports, when a verification recorded one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<Version>,
    /// Whether the reported version is the published one, the source one, or neither.
    pub agreement: Agreement,
}

/// How a deployed version stands against the repository's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "VersionAgreement")]
pub enum Agreement {
    /// It runs the published release.
    Published,
    /// It runs what this tree would release, which is not published.
    Source,
    /// It runs something else: this is drift, and it is what the display must show.
    Drift,
    /// Nothing has reported a version. Not the same as drift, and not the same as agreement.
    Unknown,
}

/// The whole release state, as every surface reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "ReleaseState")]
pub struct ReleaseState {
    /// The four versions.
    pub versions: VersionReport,
    /// Which release the contract was compared against.
    pub baseline: Baseline,
    /// What the change costs, when it could be decided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact: Option<CompatibilityImpact>,
    /// Whether that impact was measured from the contract or claimed by the records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact_source: Option<ImpactSource>,
    /// The bump that impact requires under this project's policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_bump: Option<Bump>,
    /// The lowest version a release carrying this change may declare.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_version: Option<Version>,
    /// The version this tree would publish: the source version, once it clears the
    /// minimum, and the minimum otherwise.
    pub target_version: Version,
    /// How far along the release is.
    pub readiness: Readiness,
    /// How many changes each impact accounts for.
    pub contract_changes: usize,
    /// How many change records are unreleased.
    pub unreleased_changes: usize,
    /// Everything wrong, in the order it matters.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

/// Everything the release engine reads, gathered once.
///
/// Held apart from [`ReleaseState`] because the diff and the changelog are large and only
/// some callers want them: `release status` wants the verdict, `version explain` wants the
/// diff, `release changelog` wants the changelog, and all three want the same computation
/// to have happened once.
pub struct Engine {
    /// The version this tree would release.
    pub source: Version,
    /// The contract as it stands now.
    pub current: ContractSnapshot,
    /// The contract at the baseline, when it could be read.
    pub baseline_snapshot: Option<ContractSnapshot>,
    /// Which release the baseline is.
    pub baseline: Baseline,
    /// The published releases.
    pub releases: Releases,
    /// The authored changes.
    pub changes: Changes,
    /// The contract diff, when a baseline snapshot was readable.
    pub diff: Option<ContractDiff>,
    /// The distribution model, when the process was started with one.
    pub distribution: Option<DistributionModel>,
    /// Why the committed contract snapshot could not be read, when it could not.
    pub snapshot_error: Option<String>,
    /// How the committed snapshot's capability surface differs from the live registry,
    /// when it does. One line per difference.
    pub stale: Option<Vec<String>>,
    /// True when the index came back degraded: something under `.ai/` did not read, so the
    /// set of change records this engine saw may be short of the set that exists.
    pub layer_degraded: bool,
}

impl Engine {
    /// Read everything the release engine needs from a repository.
    ///
    /// # Where the current contract comes from
    ///
    /// From `docs/generated/contract.json`, the committed projection — not from a snapshot
    /// rebuilt here. Three reasons, in order of weight:
    ///
    /// 1. **It is the same document the baseline is.** The baseline is that file as it
    ///    stood at the release tag, and comparing a committed document against a freshly
    ///    built one would compare two things built by two code paths.
    /// 2. **A full snapshot needs the command graph**, which costs a `just --dump` and a
    ///    clap walk. A version display must not pay for that, and the Cockpit renders one
    ///    on every page.
    /// 3. **Staleness is already somebody's job.** `majordomus generate --check` refuses a
    ///    stale projection, from the pre-commit hook and from CI, for this artifact exactly
    ///    as for `openapi.json`.
    ///
    /// What this does check, because it is cheap and it is the case that bites in a working
    /// tree, is that the committed snapshot's *capability* surface still matches the live
    /// registry. A capability added and not yet generated is the ordinary mid-work state,
    /// and a verdict computed over the old contract would be quietly wrong; the diagnostic
    /// `CONTRACT_SNAPSHOT_STALE` says so and names the command that fixes it.
    pub fn load(
        root: &Path,
        index: &Index,
        registry: Option<&crate::capability::registry::CapabilityRegistry>,
    ) -> Result<Self, String> {
        Self::load_with(root, index, registry, None)
    }

    /// The same, over a contract this caller has already built.
    ///
    /// `majordomus generate` is the one caller. It builds the snapshot from the live
    /// inputs and then renders the manifests, which carry the contract's fingerprint —
    /// and if the engine read the *committed* snapshot at that moment, a manifest would
    /// carry the fingerprint of the contract as it stood before this generation. The next
    /// run would then see the manifest as stale, regenerate it, and see it stale again:
    /// generation would never converge, and `generate --check` would fail on a tree
    /// nothing was wrong with.
    pub fn load_with(
        root: &Path,
        index: &Index,
        registry: Option<&crate::capability::registry::CapabilityRegistry>,
        contract: Option<ContractSnapshot>,
    ) -> Result<Self, String> {
        let source: Version = crate::VERSION
            .parse()
            .map_err(|e| format!("the crate's version is not a version: {e}"))?;
        let releases = Releases::from_index(index)?;
        let changes = Changes::from_index(index);
        let (current, snapshot_error) = match contract {
            Some(built) => (built, None),
            None => match std::fs::read_to_string(root.join(SNAPSHOT_PATH))
                .ok()
                .map(|text| ContractSnapshot::parse(&text))
            {
                Some(Ok(snapshot)) => (snapshot, None),
                Some(Err(reason)) => (
                    ContractSnapshot::from_entries(Vec::new()),
                    Some(format!("{SNAPSHOT_PATH}: {reason}")),
                ),
                None => (
                    ContractSnapshot::from_entries(Vec::new()),
                    Some(format!(
                        "{SNAPSHOT_PATH} is not committed, so this tree states no contract"
                    )),
                ),
            },
        };
        let stale = registry.and_then(|r| stale_surface(&current, r));
        let (baseline, baseline_snapshot) = discover_baseline(root, &releases);
        let diff = baseline_snapshot.as_ref().map(|b| diff::diff(b, &current));
        Ok(Self {
            source,
            current,
            baseline_snapshot,
            baseline,
            releases,
            changes,
            diff,
            distribution: index.distribution.clone(),
            snapshot_error,
            stale,
            layer_degraded: index.state == crate::index::State::Degraded,
        })
    }

    /// The impact of the contract change, when it could be computed.
    ///
    /// The measurement, and only the measurement. [`Engine::effective_impact`] is what a
    /// version is computed from; this is what a report says was *observed*.
    pub fn impact(&self) -> Option<CompatibilityImpact> {
        self.diff.as_ref().map(|d| d.impact)
    }

    /// The strongest impact the unreleased change records claim.
    ///
    /// Authored, not measured. A record's `impact` may not understate the contract — the
    /// release check compares them — so where both exist the contract decides and this
    /// adds nothing. Where the contract could not be measured it is all there is, and it
    /// is better than nothing: a person who wrote "this removes an endpoint" has said
    /// something a diff would have said, in the same words.
    pub fn declared_impact(&self) -> Option<CompatibilityImpact> {
        self.changes
            .unreleased()
            .map(|c| c.impact)
            .reduce(CompatibilityImpact::max)
    }

    /// The impact a version is computed from, and where it came from.
    ///
    /// The contract diff when there is one, the change records when there is not. Never
    /// the commit messages: a message says what somebody meant to do.
    pub fn effective_impact(&self) -> Option<(CompatibilityImpact, ImpactSource)> {
        match self.impact() {
            Some(impact) => Some((impact, ImpactSource::Contract)),
            None => self
                .declared_impact()
                .map(|impact| (impact, ImpactSource::Records)),
        }
    }

    /// The bump the change requires, computed from the baseline's version.
    pub fn required_bump(&self) -> Option<Bump> {
        let (impact, _) = self.effective_impact()?;
        let baseline = self.baseline.version()?;
        Some(diff::required_bump(impact, baseline))
    }

    /// The lowest version a release may declare.
    pub fn minimum_version(&self) -> Option<Version> {
        let (impact, _) = self.effective_impact()?;
        let baseline = self.baseline.version()?;
        Some(diff::minimum_version(impact, baseline))
    }

    /// The version this tree would publish.
    ///
    /// The source version when it already clears the minimum — a tree whose version was
    /// bumped by an earlier commit must not be asked to bump again for the same change —
    /// and the minimum otherwise.
    pub fn target_version(&self) -> Version {
        match self.minimum_version() {
            Some(minimum) if minimum > self.source => minimum,
            _ => self.source.clone(),
        }
    }

    /// The changelog, built from the records this engine read.
    pub fn changelog(&self) -> Changelog {
        Changelog::build(&self.changes, &self.releases)
    }

    /// The four versions and their disagreements.
    pub fn versions(&self, index: &Index) -> VersionReport {
        let published: Option<Version> = self
            .releases
            .latest_stable()
            .and_then(|r| r.version.parse().ok());
        let running: Version = crate::VERSION
            .parse()
            .unwrap_or_else(|_| Version::new(0, 0, 0));
        let deployed = deployed_versions(index, published.as_ref(), &self.source);
        let unreleased = published.as_ref().is_none_or(|p| self.source > *p);
        let mut divergences = Vec::new();
        if let Some(published) = &published {
            if self.source > *published {
                divergences.push(format!(
                    "this tree would release {} and {} is published",
                    self.source, published
                ));
            } else if self.source < *published {
                divergences.push(format!(
                    "this tree states {} and {} is already published: the tree is behind its own release",
                    self.source, published
                ));
            }
        }
        for d in &deployed {
            if d.agreement == Agreement::Drift {
                divergences.push(format!(
                    "{} reports {} and the published release is {}",
                    d.deployment,
                    d.version
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "an unknown version".into()),
                    published
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "none".into()),
                ));
            }
        }
        VersionReport {
            source: self.source.clone(),
            published,
            running,
            deployed,
            unreleased,
            divergences,
        }
    }

    /// The whole state, including every diagnostic.
    pub fn state(&self, index: &Index) -> ReleaseState {
        let versions = self.versions(index);
        let impact = self.effective_impact().map(|(i, _)| i);
        let required_bump = self.required_bump();
        let minimum_version = self.minimum_version();
        let target_version = self.target_version();
        let diagnostics = self.diagnostics(&target_version);
        let readiness = self.readiness(&versions, &target_version, &diagnostics);
        ReleaseState {
            versions,
            baseline: self.baseline.clone(),
            impact,
            impact_source: self.effective_impact().map(|(_, s)| s),
            required_bump,
            minimum_version,
            target_version,
            readiness,
            contract_changes: self.diff.as_ref().map(|d| d.changes.len()).unwrap_or(0),
            unreleased_changes: self.changes.unreleased().count(),
            diagnostics,
        }
    }

    /// Every way this repository is not ready to release.
    ///
    /// The order is the order a person should act in: a version that does not clear the
    /// minimum first, because nothing else matters until it does; then the changelog
    /// coverage the release would be published without; then the records' own consistency.
    pub fn diagnostics(&self, target: &Version) -> Vec<Diagnostic> {
        let mut out = Vec::new();

        // A layer that did not read is a layer whose change records may not all be here —
        // and a missing record is a release whose changelog is silently short. The verdict
        // below is computed over what was read, so it is reported, and this says the
        // verdict cannot be trusted until the layer does.
        if self.layer_degraded {
            out.push(Diagnostic::error(
                "LAYER_DEGRADED",
                "the repository's layer did not read cleanly, so the change records this \
                 verdict was computed over may be incomplete",
                Some("majordomus capabilities validate"),
                None,
            ));
        }

        if let (Some(minimum), Some((impact, source))) =
            (self.minimum_version(), self.effective_impact())
        {
            if self.source < minimum {
                out.push(Diagnostic::error(
                    "SEMVER_BUMP_TOO_LOW",
                    format!(
                        "the change is {impact} — {} — and requires at least {minimum}; this \
                         tree states {}",
                        match source {
                            ImpactSource::Contract =>
                                "measured against the baseline's public contract",
                            ImpactSource::Records => "as the unreleased change records state it",
                        },
                        self.source
                    ),
                    Some("majordomus release explain"),
                    Some(format!("majordomus release prepare --version {minimum}")),
                ));
            }
        }

        // A repository with no committed contract has nothing to measure — the ordinary
        // state of one that has never generated — so this is reported and does not block.
        // Whether a repository that *should* have one has fallen behind is
        // `generate --check`'s question, and it is asked on the same paths by the same
        // gate; answering it twice, in two voices, would make a fresh repository look
        // broken for having done nothing wrong.
        if let Some(reason) = &self.snapshot_error {
            out.push(Diagnostic::info(
                "CONTRACT_SNAPSHOT_MISSING",
                reason.clone(),
                Some("majordomus generate --target release --check"),
            ));
        }

        if let Some(differences) = &self.stale {
            out.push(Diagnostic::error(
                "CONTRACT_SNAPSHOT_STALE",
                format!(
                    "the committed contract states a capability surface this build does not: {}",
                    differences.join("; ")
                ),
                Some("majordomus generate --target release --check"),
                Some("majordomus generate --target release".into()),
            ));
        }

        match &self.baseline {
            Baseline::Predates { tag, .. } => out.push(Diagnostic::info(
                "CONTRACT_BASELINE_PREDATES",
                format!(
                    "{tag} was published before this repository recorded a contract, so this \
                     release carries no computed compatibility; the next one will"
                ),
                Some("majordomus release explain"),
            )),
            Baseline::Unreadable { tag, reason, .. } => out.push(Diagnostic::error(
                "CONTRACT_BASELINE_UNREADABLE",
                format!(
                    "the contract at {tag} could not be read ({reason}), so no compatibility \
                     verdict was reached for this release"
                ),
                Some("majordomus release explain"),
                Some(format!("git fetch --tags && git cat-file -e {tag}")),
            )),
            Baseline::Resolved { .. } | Baseline::Initial => {}
        }

        // A breaking contract change nobody wrote a record for is a release whose changelog
        // does not explain the thing its readers most need explained.
        if let Some(diff) = &self.diff {
            let covered = self.changes.covered();
            for change in diff.with_impact(CompatibilityImpact::Breaking) {
                let reference = format!("{}:{}", change.surface.as_str(), change.entry);
                if !covered.contains(reference.as_str()) {
                    out.push(Diagnostic::error(
                        "CHANGELOG_MISSING",
                        format!(
                            "`{reference}` is a breaking contract change and no change record \
                             names it"
                        ),
                        Some("majordomus release diff --impact breaking"),
                        Some(format!(
                            "write .ai/repo/changes/<id>.md with `contract: [\"{reference}\"]`"
                        )),
                    ));
                }
            }
        }

        for change in self.changes.unreleased() {
            if change.impact == CompatibilityImpact::Breaking && change.migration.is_none() {
                out.push(Diagnostic::error(
                    "MIGRATION_GUIDE_REQUIRED",
                    format!(
                        "{} is breaking and names no migration document",
                        change.path
                    ),
                    Some("majordomus release check"),
                    Some(format!(
                        "add `migration: docs/migrations/{}.md` and write it",
                        target.to_string().replace('.', "-")
                    )),
                ));
            }
        }

        for finding in self.changes.findings() {
            // the two above are reported with their own codes; everything else the change
            // records disagree about is one code, because a reader acts on it the same way.
            if finding.contains("no way through it") {
                continue;
            }
            out.push(Diagnostic::error(
                "RELEASE_CHANGE_INVALID",
                finding,
                Some("majordomus release check"),
                None,
            ));
        }

        if let Some(model) = &self.distribution {
            for finding in self.releases.findings(model) {
                out.push(Diagnostic::error(
                    "RELEASE_MANIFEST_INVALID",
                    finding,
                    Some("majordomus distribution validate"),
                    None,
                ));
            }
        }

        out
    }

    /// How far along the release is.
    fn readiness(
        &self,
        versions: &VersionReport,
        target: &Version,
        diagnostics: &[Diagnostic],
    ) -> Readiness {
        // Nothing to release means exactly that: the published version is this one, and
        // neither the contract nor a change record says anything has happened since.
        if versions.published.as_ref() == Some(&self.source)
            && self
                .effective_impact()
                .is_none_or(|(i, _)| i == CompatibilityImpact::None)
            && self.changes.unreleased().next().is_none()
        {
            return Readiness::NothingToRelease;
        }
        if self
            .releases
            .by_tag(&target.tag())
            .is_some_and(|r| !r.yanked)
        {
            return Readiness::Published;
        }
        if diagnostics.iter().any(|d| d.code == "SEMVER_BUMP_TOO_LOW") {
            return Readiness::BumpRequired;
        }
        if diagnostics.iter().any(Diagnostic::is_blocking) {
            return Readiness::Blocked;
        }
        Readiness::ReadyToTag
    }
}

/// How the committed snapshot's capability surface differs from the live registry.
///
/// `None` when they agree, which is the state a committed tree is in. Only the capability
/// surface: it is the one the process can see without building anything, and it is the one
/// that moves while somebody is working.
fn stale_surface(
    committed: &ContractSnapshot,
    registry: &crate::capability::registry::CapabilityRegistry,
) -> Option<Vec<String>> {
    use std::collections::BTreeSet;
    let recorded: BTreeSet<&str> = committed
        .entries
        .iter()
        .filter(|e| e.surface == Surface::Capability)
        .map(|e| e.id.as_str())
        .collect();
    // A snapshot with no capability surface at all is not stale, it is absent; the missing
    // diagnostic already says so, and reporting every capability as new on top of it would
    // bury it.
    if recorded.is_empty() {
        return None;
    }
    // The same set the snapshot carries: the executable's own capabilities. A declarative
    // one comes and goes with a repository's files and is not this executable's contract.
    let live: BTreeSet<&str> = registry
        .iter()
        .filter(|c| {
            matches!(
                c.provenance,
                crate::capability::model::Provenance::Builtin { .. }
            )
        })
        .map(|c| c.id.as_str())
        .collect();
    let mut out = Vec::new();
    for id in live.difference(&recorded) {
        out.push(format!("`{id}` exists and is not in the snapshot"));
    }
    for id in recorded.difference(&live) {
        out.push(format!("`{id}` is in the snapshot and does not exist"));
    }
    (!out.is_empty()).then_some(out)
}

/// Pick the baseline and read its contract.
///
/// The baseline is the release an unpinned installation currently resolves to: the highest
/// stable, unwithdrawn record. A pre-release is never a baseline — it is published and
/// addressable and it is not what the contract is measured against, because measuring
/// against one would let a break in between two pre-releases pass unnoticed on the way to
/// the release they precede.
fn discover_baseline(root: &Path, releases: &Releases) -> (Baseline, Option<ContractSnapshot>) {
    let Some(record) = releases.latest_stable() else {
        return (Baseline::Initial, None);
    };
    let Ok(version) = record.version.parse::<Version>() else {
        return (
            Baseline::Unreadable {
                version: Version::new(0, 0, 0),
                tag: record.tag.clone(),
                reason: format!(
                    "the record states `{}`, which is not a version",
                    record.version
                ),
            },
            None,
        );
    };
    let tag = record.tag.clone();
    if !crate::git::resolves(root, &tag) {
        return (
            Baseline::Unreadable {
                version,
                tag: tag.clone(),
                reason: format!("`{tag}` does not resolve in this clone"),
            },
            None,
        );
    }
    let Some(text) = crate::git::show(root, &tag, SNAPSHOT_PATH) else {
        return (Baseline::Predates { version, tag }, None);
    };
    match ContractSnapshot::parse(&text) {
        Ok(snapshot) => (
            Baseline::Resolved {
                version,
                tag,
                commit: record.commit.clone(),
                fingerprint: snapshot.fingerprint.clone(),
            },
            Some(snapshot),
        ),
        Err(reason) => (
            Baseline::Unreadable {
                version,
                tag,
                reason,
            },
            None,
        ),
    }
}

/// What each declared deployment reports.
fn deployed_versions(
    index: &Index,
    published: Option<&Version>,
    source: &Version,
) -> Vec<DeployedVersion> {
    let mut out = Vec::new();
    for object in index.objects.iter().filter(|o| o.kind == "deployment") {
        let field = |name: &str| -> Option<&str> {
            object
                .metadata
                .get(name)
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
        };
        let id = field("id").unwrap_or(&object.identity).to_string();
        let status = field("status").unwrap_or("declared").to_string();
        // A record carries the version it was last verified running, when a verification
        // wrote one. Nothing here contacts the service: a version display renders from
        // local state, always, and asks the network never.
        let version = field("running_version").and_then(|v| v.parse::<Version>().ok());
        let agreement = match (&version, published) {
            (None, _) => Agreement::Unknown,
            (Some(v), Some(p)) if v == p => Agreement::Published,
            (Some(v), _) if v == source => Agreement::Source,
            (Some(_), _) => Agreement::Drift,
        };
        out.push(DeployedVersion {
            deployment: id,
            environment: field("environment").unwrap_or("unknown").to_string(),
            status,
            version,
            agreement,
        });
    }
    out.sort_by(|a, b| a.deployment.cmp(&b.deployment));
    out
}

/// The surfaces a snapshot must cover for a verdict to be complete.
///
/// Used by the check that a snapshot written on a machine without a built registry does not
/// become a baseline: a partial snapshot compared against a full one reports every entry of
/// the missing surface as removed, and the diff refuses to do that — but a *committed*
/// partial snapshot would silently narrow every future verdict, so it is refused here.
pub const REQUIRED_SURFACES: &[Surface] = &[Surface::Capability, Surface::Command];

#[cfg(test)]
mod tests;
