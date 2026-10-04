//! The version obligation: what integrating this tree into the trunk requires the version
//! to become.
//!
//! # Two requirements, one answer
//!
//! [`crate::release::compat::analyze`] answers a compatibility question: what the public
//! contract has done since the last *release*, and so the smallest version a release of this
//! tree may carry. That answer is authoritative for compatibility and nothing here replaces
//! it. It is also silent about most work: a fix, a document, a test and a refactor behind the
//! boundary move no contract, so a trunk could take a hundred of them and stay on one
//! version, and nothing would say that the version no longer names what is being served.
//!
//! The second requirement is the repository's **completion cadence** (`release.cadence` in
//! the policy, ADR 0106): accepted work advances the trunk's version by at least that much.
//! It is measured against the version the *trunk* declares — not the last release — because
//! the trunk is what the work is integrated into, and two branches that both started from
//! it must not both land claiming the same next version.
//!
//! ```text
//!   contract floor  = last release raised by what the contract requires   (ADR 0051)
//!   cadence floor   = trunk version raised by the cadence, when the change carries work
//!   minimum         = max(trunk version, contract floor, cadence floor)
//!   effective       = the bump from the trunk version to the minimum
//! ```
//!
//! A breaking change on `1.x` therefore still costs a major whatever the cadence says, and an
//! internal fix still costs the cadence's minor whatever the contract says.
//!
//! # What carries work
//!
//! The obligation is owed by a change set, not by a conversation. Every path the tree
//! changes against its merge base with the trunk is classified by what makes it machine
//! output rather than by a message somebody wrote: a path the trunk's `.gitattributes` marks
//! `merge=derived` is a projection, a release record the change set *adds* is publication
//! evidence — and what the record generator writes from it (its public metadata, the stable
//! pointer, and the `merge=derived` lines declaring them, which the trunk cannot carry for a
//! file it does not have yet) are that record's projections — and the manifest and lock are
//! a version advance when they differ only by what [`super::version::write`] would have
//! written. A published record edited by hand is work, and so is anything else. Trunk
//! attributes that cannot be read leave the change set unclassified, and the verdict
//! `unverified`, never a guess. A change set that carries no work —
//! a projection refresh, a release record landing after publication, an advance on its own —
//! owes no cadence, which is what stops the release pipeline's own follow-up commits from
//! raising the version they were written to record.
//!
//! # Why a retry cannot advance twice
//!
//! The obligation is a predicate over the tree and the trunk, not a counter of events. It is
//! satisfied when the declared version reaches the minimum computed against the trunk *as it
//! is now*, so asking again — a second `finish`, a rerun workflow, a provider end hook that
//! fires twice — finds it satisfied and writes nothing. It is owed again only when the trunk
//! itself moves past it: another integration landed first, and this one must advance from
//! the version that won.
//!
//! ```
//! use majordomus_cli::release::compat::Impact;
//! use majordomus_cli::release::obligation::{decide, Carries};
//! use majordomus_cli::release::version::Version;
//! let v = |s| Version::parse(s).unwrap();
//! // two branches from 1.10.0 both advanced to 1.11.0; once the first has landed, the second owes 1.12.0
//! let second = decide(v("1.11.0"), v("1.11.0"), None, Impact::Minor, Carries::Work);
//! assert_eq!(second.minimum.to_string(), "1.12.0");
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::compat::{declared_impact, Impact};
use super::version::{self, Version};
use crate::capability::registry::CapabilityRegistry;
use crate::model::Object;

/// The schema an obligation carries, so a reader knows the rules it was decided under.
pub const OBLIGATION_SCHEMA: &str = "majordomus/version-obligation/v1";

/// Where the release records live: a path under it is publication evidence, not work.
pub const RELEASE_RECORDS: &str = ".ai/repo/releases/";

/// The trunk when the policy names none.
pub const DEFAULT_TRUNK: &str = "origin/master";

/// How many work paths an obligation names as its evidence; the count is always whole.
const WORK_EXAMPLES: usize = 8;

/// `release:` in the policy — how this repository's version follows its work.
///
/// ```
/// use majordomus_cli::release::compat::Impact;
/// use majordomus_cli::release::obligation::ReleasePolicy;
/// // A policy that says nothing imposes no cadence: the contract alone decides, as before.
/// let quiet = ReleasePolicy::default();
/// assert_eq!(quiet.cadence, Impact::None);
/// assert_eq!(quiet.trunk(), "origin/master");
/// let minor: ReleasePolicy = serde_json::from_str(r#"{"cadence":"minor"}"#).unwrap();
/// assert_eq!(minor.cadence, Impact::Minor);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ReleaseCadencePolicy")]
pub struct ReleasePolicy {
    /// How much integrated work advances the trunk's version at least: `none` (the default,
    /// and the behaviour before ADR 0106), `patch`, `minor` or `major`.
    #[serde(default = "no_cadence")]
    pub cadence: Impact,
    /// The ref the work is integrated into. `origin/master` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunk: Option<String>,
}

fn no_cadence() -> Impact {
    Impact::None
}

impl Default for ReleasePolicy {
    fn default() -> Self {
        ReleasePolicy {
            cadence: no_cadence(),
            trunk: None,
        }
    }
}

impl ReleasePolicy {
    /// The trunk ref, defaulted.
    /// The ref the work is integrated into: the policy's `trunk` when it names one, and
    /// `origin/master` — this repository's canonical trunk — when it does not.
    ///
    /// ```
    /// use majordomus_cli::release::obligation::ReleasePolicy;
    /// let named = ReleasePolicy { trunk: Some("origin/main".into()), ..ReleasePolicy::default() };
    /// assert_eq!(named.trunk(), "origin/main");
    /// assert_eq!(ReleasePolicy::default().trunk(), "origin/master");
    /// ```
    pub fn trunk(&self) -> &str {
        self.trunk.as_deref().unwrap_or(DEFAULT_TRUNK)
    }
}

/// What one changed path is, decided by what makes it machine output.
///
/// ```
/// use majordomus_cli::release::obligation::PathClass;
/// assert_eq!(PathClass::Work.as_str(), "work");
/// assert_eq!(PathClass::ReleaseEvidence.as_str(), "release-evidence");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "VersionPathClass")]
pub enum PathClass {
    /// A path the trunk's `.gitattributes` marks `merge=derived`, or one the record generator
    /// writes from a release record the same change set adds: a generator rewrites it.
    Derived,
    /// A release record the change set adds: evidence of a publication that already happened.
    ReleaseEvidence,
    /// The manifest or the lock, differing only by what the version writer writes.
    VersionAdvance,
    /// Everything else.
    Work,
}

impl PathClass {
    /// The word every surface prints for this class: the command line, the JSON a machine
    /// surface answers with and the gate's refusal all use it and no wording of their own.
    ///
    /// ```
    /// use majordomus_cli::release::obligation::PathClass;
    /// assert_eq!(PathClass::VersionAdvance.as_str(), "version-advance");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            PathClass::Derived => "derived",
            PathClass::ReleaseEvidence => "release-evidence",
            PathClass::VersionAdvance => "version-advance",
            PathClass::Work => "work",
        }
    }
}

/// What a change set carries, from the classes of its paths.
///
/// Work dominates: one authored path among a thousand derived ones is work. Otherwise the
/// set is named by the strongest machine class it holds, so a release record that lands with
/// the projections it publishes is release evidence, not a projection refresh.
///
/// ```
/// use majordomus_cli::release::obligation::{Carries, PathClass};
/// assert_eq!(Carries::of([PathClass::Derived, PathClass::Work]), Carries::Work);
/// assert_eq!(Carries::of([PathClass::Derived, PathClass::ReleaseEvidence]), Carries::ReleaseEvidence);
/// assert_eq!(Carries::of([PathClass::Derived]), Carries::GeneratedSync);
/// assert_eq!(Carries::of([PathClass::VersionAdvance, PathClass::Derived]), Carries::VersionAdvance);
/// assert_eq!(Carries::of([]), Carries::Nothing);
/// assert!(Carries::Work.owes_cadence());
/// assert!(!Carries::ReleaseEvidence.owes_cadence());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "VersionCarries")]
pub enum Carries {
    /// At least one authored path: accepted work, which owes the cadence.
    Work,
    /// A release record, with or without the projections it publishes.
    ReleaseEvidence,
    /// A version advance and its projections, and nothing else.
    VersionAdvance,
    /// Only projections a generator rewrites.
    GeneratedSync,
    /// No change at all against the trunk.
    Nothing,
}

impl Carries {
    /// The carriage of a set of path classes.
    ///
    /// ```
    /// use majordomus_cli::release::obligation::{Carries, PathClass};
    /// assert_eq!(Carries::of([PathClass::Work, PathClass::ReleaseEvidence]), Carries::Work);
    /// ```
    pub fn of(classes: impl IntoIterator<Item = PathClass>) -> Carries {
        let set: BTreeSet<PathClass> = classes.into_iter().collect();
        if set.contains(&PathClass::Work) {
            Carries::Work
        } else if set.contains(&PathClass::ReleaseEvidence) {
            Carries::ReleaseEvidence
        } else if set.contains(&PathClass::VersionAdvance) {
            Carries::VersionAdvance
        } else if set.contains(&PathClass::Derived) {
            Carries::GeneratedSync
        } else {
            Carries::Nothing
        }
    }

    /// Whether this carriage owes the completion cadence. Only work does: machine output
    /// that owed it would raise the version it was written to record, without end.
    ///
    /// ```
    /// use majordomus_cli::release::obligation::Carries;
    /// assert!(Carries::Work.owes_cadence());
    /// assert!(!Carries::GeneratedSync.owes_cadence());
    /// ```
    pub fn owes_cadence(self) -> bool {
        self == Carries::Work
    }

    /// The word every surface prints for this carriage, the same on the command line, in
    /// the JSON of every machine surface and in the gate's refusal.
    ///
    /// ```
    /// use majordomus_cli::release::obligation::Carries;
    /// assert_eq!(Carries::ReleaseEvidence.as_str(), "release-evidence");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Carries::Work => "work",
            Carries::ReleaseEvidence => "release-evidence",
            Carries::VersionAdvance => "version-advance",
            Carries::GeneratedSync => "generated-sync",
            Carries::Nothing => "nothing",
        }
    }
}

/// Where an obligation stands: the verdict every surface renders and every exit derives
/// from, decided from the tree and the trunk rather than counted from events.
///
/// ```
/// use majordomus_cli::release::obligation::ObligationState;
/// assert_eq!(ObligationState::Owed.exit_code(), 10);
/// assert_eq!(ObligationState::Satisfied.exit_code(), 0);
/// assert_eq!(ObligationState::Unverified.exit_code(), 12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "VersionObligationState")]
pub enum ObligationState {
    /// The declared version reaches the minimum, and the change set owed an advance.
    Satisfied,
    /// Nothing was owed: the change set carries no work and the contract is covered.
    NotOwed,
    /// The declared version is below the minimum and not below the trunk: an advance
    /// through the one writer satisfies it.
    Owed,
    /// The declared version is below the trunk's: this tree is behind the version that won.
    /// The trunk is merged first; then the advance is computed from what it declares.
    Behind,
    /// The trunk or a version could not be read, so nothing was decided — never a pass.
    Unverified,
}

impl ObligationState {
    /// The exit every command that renders this verdict gives: 0 holds, 10 refused, 12
    /// unreadable — the codes `release analyze` already uses for the same three answers.
    ///
    /// ```
    /// use majordomus_cli::release::obligation::ObligationState;
    /// assert_eq!(ObligationState::Behind.exit_code(), 10);
    /// assert_eq!(ObligationState::NotOwed.exit_code(), 0);
    /// ```
    pub fn exit_code(self) -> u8 {
        match self {
            ObligationState::Satisfied | ObligationState::NotOwed => 0,
            ObligationState::Owed | ObligationState::Behind => 10,
            ObligationState::Unverified => 12,
        }
    }

    /// Whether the obligation holds.
    /// Whether the obligation holds: satisfied, or nothing was owed. Owed, behind and
    /// unverified do not — an obligation nobody could read is never a pass.
    ///
    /// ```
    /// use majordomus_cli::release::obligation::ObligationState;
    /// assert!(ObligationState::NotOwed.holds());
    /// assert!(!ObligationState::Unverified.holds());
    /// ```
    pub fn holds(self) -> bool {
        self.exit_code() == 0
    }

    /// The word every surface prints for this verdict, and the value the JSON carries, so a
    /// reader of either never translates between two vocabularies.
    ///
    /// ```
    /// use majordomus_cli::release::obligation::ObligationState;
    /// assert_eq!(ObligationState::NotOwed.as_str(), "not-owed");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            ObligationState::Satisfied => "satisfied",
            ObligationState::NotOwed => "not-owed",
            ObligationState::Owed => "owed",
            ObligationState::Behind => "behind",
            ObligationState::Unverified => "unverified",
        }
    }
}

/// The trunk as it was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "VersionObligationTrunk")]
///
/// ```
/// use majordomus_cli::release::obligation::Trunk;
/// let t = Trunk { reference: "origin/master".into(), commit: "e45ce7c3e3".into(),
///                 version: "0.12.0".into(), contained: true };
/// assert!(t.contained, "a tree that contains its trunk is measured from it");
/// ```
pub struct Trunk {
    /// The ref asked for: `origin/master`, `HEAD^1`.
    pub reference: String,
    /// The commit it resolved to.
    pub commit: String,
    /// The version the trunk's manifest declares.
    pub version: String,
    /// Whether this tree already contains the trunk's commit. A tree that does not is
    /// measured from its merge base; its advance must be recomputed once the trunk is merged.
    pub contained: bool,
}

/// What the public contract requires, as [`crate::release::compat::analyze`] measured it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "VersionObligationContract")]
///
/// ```
/// use majordomus_cli::release::obligation::ContractRequirement;
/// use majordomus_cli::release::compat::Impact;
/// let c = ContractRequirement { baseline: Some("v1.8.0".into()), required: Impact::Major,
///                               floor: Some("2.0.0".into()), unmeasured: None };
/// assert_eq!(c.floor.as_deref(), Some("2.0.0"));
/// ```
pub struct ContractRequirement {
    /// The release the contract is measured from, when one could be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline: Option<String>,
    /// What the contract requires over that release.
    pub required: Impact,
    /// The smallest version the contract allows: the baseline raised by `required`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor: Option<String>,
    /// Why the contract could not be measured, when it could not. The cadence still binds;
    /// the version gate (`version-surface`) is where an unmeasurable contract is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unmeasured: Option<String>,
}

/// What the completion cadence requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "VersionObligationCadence")]
/// What the completion cadence requires of this change set: the policy's cadence, what of it
/// this change set owes (nothing unless it carries work), and the version that is.
///
/// ```
/// use majordomus_cli::release::obligation::CadenceRequirement;
/// use majordomus_cli::release::compat::Impact;
/// let c = CadenceRequirement { policy: Impact::Minor, required: Impact::None, floor: "1.4.0".into() };
/// assert_eq!(c.required, Impact::None, "machine output owes none of the cadence");
/// ```
pub struct CadenceRequirement {
    /// The policy's cadence.
    pub policy: Impact,
    /// What this change set owes of it: the policy's cadence when it carries work, `none`
    /// otherwise.
    pub required: Impact,
    /// The trunk's version raised by `required`.
    pub floor: String,
}

/// How many changed paths fell in each class.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "VersionObligationPaths")]
///
/// ```
/// use majordomus_cli::release::obligation::PathCounts;
/// let counts = PathCounts { work: 12, work_examples: vec!["lib/finish.sh".into()], ..PathCounts::default() };
/// assert!(counts.work > counts.work_examples.len(), "the examples are a sample, the count is whole");
/// ```
pub struct PathCounts {
    /// Authored paths.
    pub work: usize,
    /// Paths a generator rewrites.
    pub derived: usize,
    /// Release records.
    pub release_evidence: usize,
    /// The manifest and lock, differing only by the version.
    pub version_advance: usize,
    /// The first few work paths, sorted: the evidence that work is carried.
    pub work_examples: Vec<String>,
}

/// The version obligation of a tree against its trunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "VersionObligation")]
///
/// ```
/// use majordomus_cli::release::obligation::{assemble, Observation, ContractRequirement, ReleasePolicy, PathClass, VersionObligation};
/// use majordomus_cli::release::compat::Impact;
/// use majordomus_cli::release::version::Version;
/// let o: VersionObligation = assemble(
///     Observation { subject: "feature/x".into(), trunk: Err("no trunk".into()),
///                   declared: Version::parse("1.0.0"), paths: vec![("lib/a.sh".into(), PathClass::Work)],
///                   unclassified: None },
///     ContractRequirement { baseline: None, required: Impact::None, floor: None, unmeasured: None },
///     &ReleasePolicy::default(),
/// );
/// assert_eq!(o.schema, "majordomus/version-obligation/v1");
/// assert_eq!(o.state.as_str(), "unverified");
/// ```
pub struct VersionObligation {
    /// [`OBLIGATION_SCHEMA`].
    pub schema: String,
    /// The obligation's identity: the subject that owes it and the trunk version it advances
    /// from. Stable across retries — the same branch over the same trunk is the same
    /// obligation — and new when the trunk moves, because then the advance is a new one.
    pub id: String,
    /// The branch, or `HEAD@<commit>` when the tree is detached.
    pub subject: String,
    /// The trunk, when it could be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunk: Option<Trunk>,
    /// The version this tree declares.
    pub declared: String,
    /// What the change set carries.
    pub carries: Carries,
    /// The changed paths, by class.
    pub paths: PathCounts,
    /// The public contract's requirement.
    pub contract: ContractRequirement,
    /// The completion cadence's requirement.
    pub cadence: CadenceRequirement,
    /// The smallest version this tree may declare to be integrated.
    pub minimum: String,
    /// The bump from the trunk's version to the minimum.
    pub effective: Impact,
    /// Where it stands.
    pub state: ObligationState,
    /// Why, one sentence per input.
    pub reasons: Vec<String>,
    /// What to run when it does not hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
}

/// The decision, from versions alone: the function every scenario reduces to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
///
/// ```
/// use majordomus_cli::release::obligation::{decide, Carries, Decision, ObligationState};
/// use majordomus_cli::release::compat::Impact;
/// use majordomus_cli::release::version::Version;
/// let v = |s| Version::parse(s).unwrap();
/// let d: Decision = decide(v("0.12.0"), v("0.13.0"), None, Impact::Minor, Carries::Work);
/// assert_eq!(d.state, ObligationState::Satisfied);
/// ```
pub struct Decision {
    /// The smallest version the tree may declare.
    pub minimum: Version,
    /// The bump from the trunk to `minimum`.
    pub effective: Impact,
    /// What the cadence alone required of this change set.
    pub cadence: Impact,
    /// Where the declared version stands.
    pub state: ObligationState,
}

/// Decide an obligation from versions.
///
/// `contract_floor` is the last release raised by what the contract requires, when it could
/// be measured. The cadence binds only a change set that carries work.
///
/// ```
/// use majordomus_cli::release::compat::Impact;
/// use majordomus_cli::release::obligation::{decide, Carries, ObligationState};
/// use majordomus_cli::release::version::Version;
/// let v = |s| Version::parse(s).unwrap();
/// // internal work on 1.5.0: the contract owes nothing, the cadence a minor
/// let d = decide(v("1.5.0"), v("1.5.0"), Some(v("1.5.0")), Impact::Minor, Carries::Work);
/// assert_eq!((d.minimum.to_string(), d.effective, d.state), ("1.6.0".into(), Impact::Minor, ObligationState::Owed));
/// // a breaking change on 1.x: the contract's major wins over the cadence's minor
/// let d = decide(v("1.5.0"), v("1.5.0"), Some(v("2.0.0")), Impact::Minor, Carries::Work);
/// assert_eq!((d.minimum.to_string(), d.effective), ("2.0.0".into(), Impact::Major));
/// // a release record landing after publication owes nothing
/// let d = decide(v("1.6.0"), v("1.6.0"), Some(v("1.6.0")), Impact::Minor, Carries::ReleaseEvidence);
/// assert_eq!(d.state, ObligationState::NotOwed);
/// ```
pub fn decide(
    trunk: Version,
    declared: Version,
    contract_floor: Option<Version>,
    cadence: Impact,
    carries: Carries,
) -> Decision {
    let cadence = if carries.owes_cadence() {
        cadence
    } else {
        Impact::None
    };
    let cadence_floor = trunk.raised_to(cadence);
    let minimum = [Some(trunk), contract_floor, Some(cadence_floor)]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(trunk);
    let effective = declared_impact(trunk, minimum);
    let state = if declared < trunk {
        ObligationState::Behind
    } else if declared < minimum {
        ObligationState::Owed
    } else if minimum > trunk {
        ObligationState::Satisfied
    } else {
        ObligationState::NotOwed
    };
    Decision {
        minimum,
        effective,
        cadence,
        state,
    }
}

/// The class of one changed path.
///
/// `derived` is the subset of the change set the trunk's `.gitattributes` marks
/// `merge=derived`, together with the [`record_projections`] of the records the change set
/// adds; `base` and `head` read a path's text at the merge base and in this tree.
///
/// A release record is evidence only when the change set *adds* it: the publish job writes a
/// record once, after the publication it describes. A published record edited, or deleted, is
/// a change to what the repository says it published — authored, and so work, or a hand edit
/// could ride the record's exemption past the cadence.
///
/// ```
/// use majordomus_cli::release::obligation::{classify, PathClass};
/// let derived = std::collections::BTreeSet::new();
/// let none = |_: &str| None;
/// let record = |_: &str| Some("schema: release/v1\n".to_string());
/// // added: absent at the base, present here
/// assert_eq!(classify(".ai/repo/releases/v1.0.0.yaml", &derived, &none, &record), PathClass::ReleaseEvidence);
/// // edited: present on both sides
/// assert_eq!(classify(".ai/repo/releases/v1.0.0.yaml", &derived, &record, &record), PathClass::Work);
/// assert_eq!(classify("lib/finish.sh", &derived, &none, &none), PathClass::Work);
/// ```
pub fn classify(
    path: &str,
    derived: &BTreeSet<String>,
    base: &dyn Fn(&str) -> Option<String>,
    head: &dyn Fn(&str) -> Option<String>,
) -> PathClass {
    if derived.contains(path) {
        return PathClass::Derived;
    }
    if record_tag(path).is_some() {
        return if base(path).is_none() && head(path).is_some() {
            PathClass::ReleaseEvidence
        } else {
            PathClass::Work
        };
    }
    let rewrite: Option<fn(&str, &str) -> String> = if path == version::MANIFEST {
        Some(version::rewrite_manifest)
    } else if path == version::LOCK {
        Some(version::rewrite_lock)
    } else {
        None
    };
    if let Some(rewrite) = rewrite {
        // A version advance is exactly what the one writer produces from the base: the
        // base's text rewritten to this tree's version. A dependency added beside it is a
        // change to the build, and so work.
        let advance = (|| {
            let (before, after) = (base(path)?, head(path)?);
            let to = version::declared_in(&head(version::MANIFEST)?)?;
            Some(rewrite(&before, &to) == after)
        })();
        if advance == Some(true) {
            return PathClass::VersionAdvance;
        }
    }
    PathClass::Work
}

/// The tag a release record's path names — `v1.2.0` for `.ai/repo/releases/v1.2.0.yaml` —
/// when the path is a record: a `.yaml` directly under [`RELEASE_RECORDS`], which is exactly
/// what the record reader loads (`distribution::release::Releases::load`).
fn record_tag(path: &str) -> Option<&str> {
    let tag = path.strip_prefix(RELEASE_RECORDS)?.strip_suffix(".yaml")?;
    (!tag.is_empty() && !tag.contains('/')).then_some(tag)
}

/// The paths the record generator writes from the release record of `tag`: its public
/// metadata and the stable pointer, which `majordomus generate` renders from the records
/// (`generate.rs`, from `distribution::release::{PUBLIC_DIR, LATEST}`). Read from the
/// generator's own constants, so the classification follows what the generator declares
/// and not a list kept here.
///
/// ```
/// use majordomus_cli::release::obligation::record_projections;
/// assert_eq!(
///     record_projections("v0.13.0"),
///     ["site/static/releases/v0.13.0.json", "site/static/releases/latest.json"]
/// );
/// ```
pub fn record_projections(tag: &str) -> [String; 2] {
    use crate::distribution::release::{LATEST, PUBLIC_DIR};
    [
        format!("{PUBLIC_DIR}/{tag}.json"),
        format!("{PUBLIC_DIR}/{LATEST}.json"),
    ]
}

/// The paths of a change set that are projections of the release records it adds, though
/// the trunk's `.gitattributes` cannot say so: a record's public metadata is a new file, and
/// the `merge=derived` line that declares it arrives in the same change set (#748 added
/// `v0.13.0.yaml`, `v0.13.0.json` and its attribute line together, and the trunk's attributes
/// call the JSON `unspecified`). So, for every record the change set adds — absent at the base,
/// present here — the files [`record_projections`] names are projections, and so is
/// `.gitattributes` when the only attributes it adds are `<one of them> merge=derived` and it
/// removes none (comments are not attributes: the generated block's path count changes with
/// every path it declares). A change set that adds no record gets nothing from here, and an
/// edited or deleted record adds nothing either, so a hand edit cannot borrow the exemption.
///
/// ```
/// use majordomus_cli::release::obligation::release_unit;
/// let changed: Vec<String> = [".ai/repo/releases/v2.0.0.yaml", "site/static/releases/v2.0.0.json",
///                             ".gitattributes", "lib/a.sh"].map(String::from).to_vec();
/// let base = |p: &str| (p == ".gitattributes").then(|| "x merge=derived\n".to_string());
/// let head = |p: &str| Some(if p == ".gitattributes" {
///     "x merge=derived\nsite/static/releases/v2.0.0.json merge=derived\n".to_string()
/// } else { "{}".to_string() });
/// let unit = release_unit(&changed, &base, &head);
/// assert!(unit.contains("site/static/releases/v2.0.0.json") && unit.contains(".gitattributes"));
/// assert!(!unit.contains("lib/a.sh"));
/// ```
pub fn release_unit(
    changed: &[String],
    base: &dyn Fn(&str) -> Option<String>,
    head: &dyn Fn(&str) -> Option<String>,
) -> BTreeSet<String> {
    let projections: BTreeSet<String> = changed
        .iter()
        .filter(|p| record_tag(p).is_some() && base(p).is_none() && head(p).is_some())
        .flat_map(|p| record_projections(record_tag(p).unwrap_or_default()))
        .collect();
    let mut unit: BTreeSet<String> = changed
        .iter()
        .filter(|p| projections.contains(*p))
        .cloned()
        .collect();
    let attributes = ".gitattributes";
    if !projections.is_empty() && changed.iter().any(|p| p == attributes) {
        if let Some(after) = head(attributes) {
            if only_marks_derived(
                base(attributes).as_deref().unwrap_or(""),
                &after,
                &projections,
            ) {
                unit.insert(attributes.to_string());
            }
        }
    }
    unit
}

/// Whether `after` differs from `before` only by attribute lines `<path> merge=derived` for
/// paths in `allowed`, at least one of them, with no attribute line removed or changed.
fn only_marks_derived(before: &str, after: &str, allowed: &BTreeSet<String>) -> bool {
    fn lines(text: &str) -> BTreeMap<String, usize> {
        let mut out = BTreeMap::new();
        for line in text.lines() {
            let words: Vec<&str> = line.split_whitespace().collect();
            if words.is_empty() || words[0].starts_with('#') {
                continue;
            }
            *out.entry(words.join(" ")).or_insert(0) += 1;
        }
        out
    }
    let (before, after) = (lines(before), lines(after));
    if before
        .iter()
        .any(|(line, n)| after.get(line).copied().unwrap_or(0) < *n)
    {
        return false;
    }
    let mut added = 0;
    for (line, n) in &after {
        if *n <= before.get(line).copied().unwrap_or(0) {
            continue;
        }
        match line.split(' ').collect::<Vec<_>>().as_slice() {
            [path, "merge=derived"] if allowed.contains(*path) => added += 1,
            _ => return false,
        }
    }
    added > 0
}

/// The inputs an obligation is decided from that git and the policy provide, read once.
#[derive(Debug, Clone)]
///
/// ```
/// use majordomus_cli::release::obligation::Observation;
/// let o = Observation { subject: "HEAD@0123456789ab".into(), trunk: Err("unread".into()), declared: None,
///                       paths: vec![], unclassified: None };
/// assert!(o.trunk.is_err());
/// ```
pub struct Observation {
    /// The branch or the detached commit.
    pub subject: String,
    /// The trunk, or why it could not be read.
    pub trunk: Result<Trunk, String>,
    /// The version this tree declares, when it is three numbers.
    pub declared: Option<Version>,
    /// Every changed path against the merge base, with its class.
    pub paths: Vec<(String, PathClass)>,
    /// Why the changed paths could not be classified, when they could not — the trunk's
    /// attributes were unreadable. The obligation is then `unverified`: an empty set of
    /// projections would make every derived path work, and a guess is never a verdict.
    pub unclassified: Option<String>,
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn nul_list(text: Option<String>) -> Vec<String> {
    text.unwrap_or_default()
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// Read what an obligation is decided from: the trunk, the declared version, and every path
/// this tree changes against its merge base with the trunk — committed, staged, unstaged and
/// untracked, because `finish` judges the work as it stands and CI judges a clean merge.
///
/// ```
/// use majordomus_cli::release::obligation::observe;
/// let dir = tempfile::tempdir().unwrap();
/// // not a repository: nothing resolves, and the trunk is reported unreadable, never guessed
/// let o = observe(dir.path(), "origin/master");
/// assert!(o.trunk.unwrap_err().contains("does not resolve"));
/// ```
pub fn observe(root: &Path, trunk_ref: &str) -> Observation {
    let subject = match git(root, &["symbolic-ref", "--quiet", "--short", "HEAD"]) {
        Some(b) => b.trim().to_string(),
        // detached: named by its commit. A tree with no commit at all is on a branch (unborn),
        // so symbolic-ref answered above and this arm always has a commit to name.
        None => format!(
            "HEAD@{}",
            git(root, &["rev-parse", "--short=12", "HEAD"])
                .unwrap_or_default()
                .trim()
        ),
    };
    let declared = version::declared(root).and_then(|v| Version::parse(&v));
    let trunk_commit = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{trunk_ref}^{{commit}}"),
        ],
    )
    .map(|s| s.trim().to_string());
    let Some(commit) = trunk_commit else {
        return Observation {
            subject,
            trunk: Err(format!(
                "the trunk '{trunk_ref}' does not resolve to a commit in this clone"
            )),
            declared,
            paths: Vec::new(),
            unclassified: None,
        };
    };
    let trunk_version = git(root, &["show", &format!("{commit}:{}", version::MANIFEST)])
        .and_then(|text| version::declared_in(&text));
    let Some(trunk_version) = trunk_version.filter(|v| Version::parse(v).is_some()) else {
        return Observation {
            subject,
            trunk: Err(format!(
                "the trunk '{trunk_ref}' ({}) declares no version of three numbers in {}",
                &commit[..commit.len().min(12)],
                version::MANIFEST
            )),
            declared,
            paths: Vec::new(),
            unclassified: None,
        };
    };
    // A merge of the trunk in progress — `git merge --no-commit <trunk>`, which is how a
    // branch is refreshed before its advance is recomputed — already contains the trunk: the
    // work tree is the merge's result, and its change set is measured against the trunk
    // itself rather than against the branch's older merge base.
    let merging_trunk = git(root, &["rev-parse", "--verify", "--quiet", "MERGE_HEAD"])
        .is_some_and(|m| m.trim() == commit);
    // A tree with no history in common with the trunk has no merge base; it is measured from
    // the trunk itself, and it does not contain it.
    let merge_base = git(root, &["merge-base", &commit, "HEAD"]).map(|s| s.trim().to_string());
    let contained = merging_trunk || merge_base.as_deref() == Some(commit.as_str());
    let base = match merge_base {
        Some(b) if !merging_trunk => b,
        _ => commit.clone(),
    };

    let mut changed: BTreeSet<String> = nul_list(git(
        root,
        &["diff", "--name-only", "-z", "--no-renames", &base],
    ))
    .into_iter()
    .collect();
    changed.extend(nul_list(git(
        root,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )));
    let changed: Vec<String> = changed.into_iter().collect();
    let at_base = |p: &str| git(root, &["show", &format!("{base}:{p}")]);
    let in_tree = |p: &str| std::fs::read_to_string(root.join(p)).ok();
    // The trunk's attributes say which paths a generator rewrites. When they cannot be read
    // nothing is classified: an empty set would make every projection work, a guess that
    // owes a minor nobody owes — so the obligation is unverified instead.
    let (paths, unclassified) =
        match crate::integration::relation::derived_paths(root, &commit, &changed) {
            Ok(mut derived) => {
                derived.extend(release_unit(&changed, &at_base, &in_tree));
                let paths = changed
                    .into_iter()
                    .map(|p| {
                        let class = classify(&p, &derived, &at_base, &in_tree);
                        (p, class)
                    })
                    .collect();
                (paths, None)
            }
            Err(why) => (
                Vec::new(),
                Some(format!(
                    "the trunk's .gitattributes ({}) could not be read, so no changed path \
                     could be classified: {why}",
                    &commit[..commit.len().min(12)]
                )),
            ),
        };

    Observation {
        subject,
        trunk: Ok(Trunk {
            reference: trunk_ref.to_string(),
            commit,
            version: trunk_version,
            contained,
        }),
        declared,
        paths,
        unclassified,
    }
}

/// The contract's requirement, from the one analysis.
///
/// ```
/// use majordomus_cli::release::obligation::contract_requirement;
/// use majordomus_cli::capability::registry::CapabilityRegistry;
/// let dir = tempfile::tempdir().unwrap();
/// let registry = CapabilityRegistry::builder().build().unwrap();
/// // nothing published: the contract is unmeasured, and says why
/// let c = contract_requirement(dir.path(), &registry, &[]);
/// assert!(c.unmeasured.is_some() && c.floor.is_none());
/// ```
pub fn contract_requirement(
    root: &Path,
    registry: &CapabilityRegistry,
    objects: &[Object],
) -> ContractRequirement {
    contract_of(super::compat::analyze(root, registry, objects, None).map_err(|e| e.to_string()))
}

/// The contract's requirement from the analysis' answer: the floor when the plan is sound, the
/// errors that made it unsound when it is not, and why no plan could be made when none was.
///
/// ```
/// use majordomus_cli::release::obligation::contract_of;
/// // no published baseline: unmeasured, and the reason is carried
/// let c = contract_of(Err("this repository has published nothing".into()));
/// assert_eq!((c.baseline, c.floor), (None, None));
/// assert!(c.unmeasured.unwrap().contains("published nothing"));
/// ```
pub fn contract_of(analysis: Result<super::compat::VersionPlan, String>) -> ContractRequirement {
    match analysis {
        Ok(plan) if !plan.has_errors() => {
            let floor = Version::parse(&plan.baseline.version).map(|b| b.raised_to(plan.required));
            ContractRequirement {
                baseline: Some(plan.baseline.reference.clone()),
                required: plan.required,
                floor: floor.map(|f| f.to_string()),
                unmeasured: None,
            }
        }
        Ok(plan) => ContractRequirement {
            baseline: Some(plan.baseline.reference.clone()),
            required: plan.required,
            floor: None,
            unmeasured: plan.measured_version().err(),
        },
        Err(why) => ContractRequirement {
            baseline: None,
            required: Impact::None,
            floor: None,
            unmeasured: Some(why),
        },
    }
}

/// Assemble the obligation from what was observed, the contract's requirement and the
/// policy. Pure: everything it reads is in its arguments.
///
/// ```
/// use majordomus_cli::release::obligation::{assemble, Observation, ContractRequirement, ReleasePolicy, PathClass, Trunk};
/// use majordomus_cli::release::compat::Impact;
/// use majordomus_cli::release::version::Version;
/// let trunk = Trunk { reference: "origin/master".into(), commit: "e45ce7c3e3".into(),
///                     version: "0.12.0".into(), contained: true };
/// let o = assemble(
///     Observation { subject: "feature/x".into(), trunk: Ok(trunk), declared: Version::parse("0.12.0"),
///                   paths: vec![("lib/a.sh".into(), PathClass::Work)],
///                   unclassified: None },
///     ContractRequirement { baseline: Some("v0.12.0".into()), required: Impact::None,
///                           floor: Some("0.12.0".into()), unmeasured: None },
///     &ReleasePolicy { cadence: Impact::Minor, trunk: None },
/// );
/// assert_eq!((o.minimum.as_str(), o.state.as_str()), ("0.13.0", "owed"));
/// assert_eq!(o.id, "feature/x@0.12.0");
/// ```
pub fn assemble(
    observation: Observation,
    contract: ContractRequirement,
    policy: &ReleasePolicy,
) -> VersionObligation {
    let mut counts = PathCounts::default();
    for (p, class) in &observation.paths {
        match class {
            PathClass::Work => {
                counts.work += 1;
                if counts.work_examples.len() < WORK_EXAMPLES {
                    counts.work_examples.push(p.clone());
                }
            }
            PathClass::Derived => counts.derived += 1,
            PathClass::ReleaseEvidence => counts.release_evidence += 1,
            PathClass::VersionAdvance => counts.version_advance += 1,
        }
    }
    let carries = Carries::of(observation.paths.iter().map(|(_, c)| *c));
    let declared_text = observation
        .declared
        .map(|v| v.to_string())
        .unwrap_or_else(|| "unknown".into());

    let unverified = |why: String, trunk: Option<Trunk>| {
        let id = format!(
            "{}@{}",
            observation.subject,
            trunk.as_ref().map_or("unknown", |t| t.version.as_str())
        );
        VersionObligation {
            schema: OBLIGATION_SCHEMA.into(),
            id,
            subject: observation.subject.clone(),
            trunk,
            declared: declared_text.clone(),
            carries,
            paths: counts.clone(),
            contract: contract.clone(),
            cadence: CadenceRequirement {
                policy: policy.cadence,
                required: Impact::None,
                floor: "unknown".into(),
            },
            minimum: "unknown".into(),
            effective: Impact::None,
            state: ObligationState::Unverified,
            reasons: vec![why],
            remedy: Some(format!(
                "git fetch origin, then majordomus release obligation --base {}",
                policy.trunk()
            )),
        }
    };

    let trunk = match observation.trunk.clone() {
        Ok(t) => t,
        Err(why) => return unverified(why, None),
    };
    if let Some(why) = observation.unclassified.clone() {
        return unverified(why, Some(trunk));
    }
    let Some(trunk_version) = Version::parse(&trunk.version) else {
        return unverified(
            format!("the trunk declares '{}'", trunk.version),
            Some(trunk),
        );
    };
    let Some(declared) = observation.declared else {
        return unverified(
            format!("{} declares no version of three numbers", version::MANIFEST),
            Some(trunk),
        );
    };
    let contract_floor = contract.floor.as_deref().and_then(Version::parse);
    let d = decide(
        trunk_version,
        declared,
        contract_floor,
        policy.cadence,
        carries,
    );

    let mut reasons = Vec::new();
    match (&contract.unmeasured, &contract.baseline) {
        (Some(why), _) => reasons.push(format!(
            "the public contract could not be measured, so only the cadence binds here: {}",
            why.lines().next().unwrap_or_default()
        )),
        (None, Some(base)) => reasons.push(format!(
            "the public contract requires {} since {base}{}",
            contract.required.as_str(),
            contract
                .floor
                .as_ref()
                .map(|f| format!(", so at least {f}"))
                .unwrap_or_default()
        )),
        (None, None) => {}
    }
    reasons.push(match carries {
        Carries::Work => format!(
            "the change set carries work ({} authored path(s)), and the cadence is {}: at least {}",
            counts.work,
            policy.cadence.as_str(),
            trunk_version.raised_to(d.cadence)
        ),
        other => format!(
            "the change set carries {} and no work, so the cadence owes nothing",
            other.as_str()
        ),
    });
    if !trunk.contained {
        reasons.push(format!(
            "this tree does not contain {} ({}); its advance is recomputed once the trunk is merged",
            trunk.reference,
            &trunk.commit[..trunk.commit.len().min(12)]
        ));
    }

    let remedy = match d.state {
        ObligationState::Owed => Some(
            "majordomus release advance — raises the version through the one writer; then \
             scripts/derive"
                .to_string(),
        ),
        ObligationState::Behind => Some(format!(
            "merge {} into this branch, then majordomus release advance",
            trunk.reference
        )),
        _ => None,
    };

    VersionObligation {
        schema: OBLIGATION_SCHEMA.into(),
        id: format!("{}@{}", observation.subject, trunk.version),
        subject: observation.subject,
        trunk: Some(trunk),
        declared: declared.to_string(),
        carries,
        paths: counts,
        contract,
        cadence: CadenceRequirement {
            policy: policy.cadence,
            required: d.cadence,
            floor: trunk_version.raised_to(d.cadence).to_string(),
        },
        minimum: d.minimum.to_string(),
        effective: d.effective,
        state: d.state,
        reasons,
        remedy,
    }
}

/// The obligation of the tree at `root` against `base`, or against the policy's trunk.
///
/// ```
/// use majordomus_cli::release::obligation::{obligation, ReleasePolicy};
/// use majordomus_cli::capability::registry::CapabilityRegistry;
/// let dir = tempfile::tempdir().unwrap();
/// let registry = CapabilityRegistry::builder().build().unwrap();
/// let o = obligation(dir.path(), &registry, &[], &ReleasePolicy::default(), Some("origin/master"));
/// assert_eq!(o.state.as_str(), "unverified");
/// ```
pub fn obligation(
    root: &Path,
    registry: &CapabilityRegistry,
    objects: &[Object],
    policy: &ReleasePolicy,
    base: Option<&str>,
) -> VersionObligation {
    let trunk_ref = base.unwrap_or_else(|| policy.trunk());
    let observation = observe(root, trunk_ref);
    // The contract is not measured when the trunk could not be read: the verdict is
    // unverified either way, and the analysis is the slow half.
    let contract = if observation.trunk.is_ok() {
        contract_requirement(root, registry, objects)
    } else {
        ContractRequirement {
            baseline: None,
            required: Impact::None,
            floor: None,
            unmeasured: Some("not measured: the trunk could not be read".into()),
        }
    };
    assemble(observation, contract, policy)
}

/// The release policy a repository declares, or the default — no cadence — when the policy
/// cannot be read. An unreadable policy is `doctor`'s finding, not a cadence.
///
/// ```
/// use majordomus_cli::release::obligation::{policy_of, ReleasePolicy};
/// let dir = tempfile::tempdir().unwrap();
/// assert_eq!(policy_of(dir.path()), ReleasePolicy::default());
/// ```
pub fn policy_of(root: &Path) -> ReleasePolicy {
    crate::repository::Repository::open(root)
        .ok()
        .and_then(|repo| crate::policy::LoadedPolicy::load(&repo).ok())
        .map(|l| l.policy.release)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    /// The matrix the obligation is defined by: semantic requirement × cadence → effective.
    /// The contract floor is expressed as the release raised by the semantic level.
    #[test]
    fn the_effective_requirement_is_the_larger_of_the_contract_and_the_cadence() {
        let trunk = v("1.5.0");
        let rows: &[(Impact, Impact, Impact, &str)] = &[
            (Impact::None, Impact::None, Impact::None, "1.5.0"),
            (Impact::Patch, Impact::None, Impact::Patch, "1.5.1"),
            (Impact::Minor, Impact::None, Impact::Minor, "1.6.0"),
            (Impact::Major, Impact::None, Impact::Major, "2.0.0"),
            (Impact::None, Impact::Minor, Impact::Minor, "1.6.0"),
            (Impact::Patch, Impact::Minor, Impact::Minor, "1.6.0"),
            (Impact::Minor, Impact::Minor, Impact::Minor, "1.6.0"),
            (Impact::Major, Impact::Minor, Impact::Major, "2.0.0"),
        ];
        for (semantic, cadence, effective, minimum) in rows {
            let d = decide(
                trunk,
                trunk,
                Some(trunk.raised_to(*semantic)),
                *cadence,
                Carries::Work,
            );
            assert_eq!(
                d.effective, *effective,
                "semantic {semantic:?} cadence {cadence:?}"
            );
            assert_eq!(
                d.minimum.to_string(),
                *minimum,
                "semantic {semantic:?} cadence {cadence:?}"
            );
        }
    }

    #[test]
    fn the_arithmetic_holds_across_zero_major_and_multi_digit_versions() {
        let cases = [
            ("0.42.7", "0.43.0"),
            ("1.8.9", "1.9.0"),
            ("0.9.0", "0.10.0"),
            ("10.99.12", "10.100.0"),
        ];
        for (trunk, minimum) in cases {
            let d = decide(v(trunk), v(trunk), None, Impact::Minor, Carries::Work);
            assert_eq!(d.minimum.to_string(), minimum, "from {trunk}");
        }
        let d = decide(
            v("1.8.9"),
            v("1.8.9"),
            Some(v("2.0.0")),
            Impact::Minor,
            Carries::Work,
        );
        assert_eq!(d.minimum.to_string(), "2.0.0");
    }

    /// Below 1.0 the policy (ADR 0051) floors a breaking change at a minor, so the contract
    /// and the cadence agree on the minor: the obligation does not invent a second rule.
    #[test]
    fn zero_major_follows_the_contract_policy() {
        use crate::release::compat::Policy;
        let base = v("0.42.3");
        let policy = Policy::for_version(base);
        for implied in [Impact::Minor, Impact::Major] {
            let floor = base.raised_to(policy.required_of(implied));
            let d = decide(base, base, Some(floor), Impact::Minor, Carries::Work);
            assert_eq!(d.minimum.to_string(), "0.43.0", "{implied:?} on 0.x");
        }
    }

    /// Scenario A, B, C: an ordinary completion advances once; asking again finds it
    /// satisfied; the next independent completion advances from the version that won.
    #[test]
    fn a_satisfied_obligation_is_satisfied_however_often_it_is_asked() {
        let first = decide(v("1.4.0"), v("1.4.0"), None, Impact::Minor, Carries::Work);
        assert_eq!(
            (first.state, first.minimum.to_string()),
            (ObligationState::Owed, "1.5.0".into())
        );
        for _ in 0..3 {
            let again = decide(
                v("1.4.0"),
                first.minimum,
                None,
                Impact::Minor,
                Carries::Work,
            );
            assert_eq!(again.state, ObligationState::Satisfied);
            assert_eq!(
                again.minimum, first.minimum,
                "a retry never raises the minimum"
            );
        }
        let next = decide(v("1.5.0"), v("1.5.0"), None, Impact::Minor, Carries::Work);
        assert_eq!(next.minimum.to_string(), "1.6.0");
    }

    /// Scenario G and H: two branches from 1.10.0 both advance to 1.11.0. Once A lands, B's
    /// 1.11.0 is the trunk's own version — owed, not satisfied — and it advances to 1.12.0.
    #[test]
    fn the_second_of_two_concurrent_integrations_advances_from_the_first() {
        let trunk = v("1.10.0");
        let a = decide(trunk, trunk, None, Impact::Minor, Carries::Work).minimum;
        let b = decide(trunk, trunk, None, Impact::Minor, Carries::Work).minimum;
        assert_eq!(
            (a.to_string(), b.to_string()),
            ("1.11.0".into(), "1.11.0".into())
        );
        // A lands: the trunk is 1.11.0, and B (merged with it) still declares 1.11.0
        let stale = decide(a, b, None, Impact::Minor, Carries::Work);
        assert_eq!(
            stale.state,
            ObligationState::Owed,
            "B may not reuse A's version"
        );
        assert_eq!(stale.minimum.to_string(), "1.12.0");
        // a branch that did not merge the trunk and declares less is behind it
        let behind = decide(v("1.12.0"), v("1.11.0"), None, Impact::Minor, Carries::Work);
        assert_eq!(behind.state, ObligationState::Behind);
    }

    /// A minor and a major integrating in either order end on a version that covers both.
    #[test]
    fn a_major_and_a_minor_land_in_either_order() {
        // A minor first, then B breaking (its contract floor measured from release 1.20.0)
        let a = decide(
            v("1.20.0"),
            v("1.20.0"),
            Some(v("1.20.0")),
            Impact::Minor,
            Carries::Work,
        );
        let b = decide(
            a.minimum,
            a.minimum,
            Some(v("2.0.0")),
            Impact::Minor,
            Carries::Work,
        );
        assert_eq!(
            (a.minimum.to_string(), b.minimum.to_string()),
            ("1.21.0".into(), "2.0.0".into())
        );
        // B breaking first, then A
        let b = decide(
            v("1.20.0"),
            v("1.20.0"),
            Some(v("2.0.0")),
            Impact::Minor,
            Carries::Work,
        );
        let a = decide(
            b.minimum,
            b.minimum,
            Some(v("1.20.0")),
            Impact::Minor,
            Carries::Work,
        );
        assert_eq!(
            (b.minimum.to_string(), a.minimum.to_string()),
            ("2.0.0".into(), "2.1.0".into())
        );
    }

    /// Scenario I: the pipeline's own follow-ups carry no work, so one accepted change set
    /// produces exactly one advance and the sequence terminates.
    #[test]
    fn machine_follow_ups_never_raise_the_version_they_record() {
        let mut trunk = v("0.50.0");
        let mut advances = 0;
        let sequence = [
            Carries::Work,
            Carries::VersionAdvance,
            Carries::GeneratedSync,
            Carries::ReleaseEvidence,
            Carries::GeneratedSync,
        ];
        for carries in sequence {
            let d = decide(trunk, trunk, Some(trunk), Impact::Minor, carries);
            if d.state == ObligationState::Owed {
                advances += 1;
                trunk = d.minimum;
            }
        }
        assert_eq!(advances, 1, "one accepted change set, one advance");
        assert_eq!(trunk.to_string(), "0.51.0");
    }

    /// Scenario L: a version above the minimum is accepted; one below it is not.
    #[test]
    fn a_larger_version_satisfies_and_a_smaller_one_does_not() {
        let over = decide(v("1.5.0"), v("1.9.0"), None, Impact::Minor, Carries::Work);
        assert_eq!(over.state, ObligationState::Satisfied);
        let patch = decide(v("1.5.0"), v("1.5.1"), None, Impact::Minor, Carries::Work);
        assert_eq!(
            patch.state,
            ObligationState::Owed,
            "a patch does not cover a minor cadence"
        );
    }

    #[test]
    fn a_path_is_a_version_advance_only_when_the_writer_explains_it() {
        let manifest_base = "[package]\nversion = \"0.5.0\"\n".to_string();
        let manifest_head = "[package]\nversion = \"0.6.0\"\n".to_string();
        let built = "[package]\nversion = \"0.6.0\"\n\n[dependencies]\nx = \"1\"\n".to_string();
        let derived: BTreeSet<String> = ["share/version.txt".to_string()].into();
        let base = |_: &str| Some(manifest_base.clone());
        let head = |_: &str| Some(manifest_head.clone());
        assert_eq!(
            classify(version::MANIFEST, &derived, &base, &head),
            PathClass::VersionAdvance
        );
        let head_with_dep = |_: &str| Some(built.clone());
        assert_eq!(
            classify(version::MANIFEST, &derived, &base, &head_with_dep),
            PathClass::Work
        );
        assert_eq!(
            classify("share/version.txt", &derived, &base, &head),
            PathClass::Derived
        );
        let absent = |_: &str| None;
        assert_eq!(
            classify(".ai/repo/releases/v0.6.0.yaml", &derived, &absent, &head),
            PathClass::ReleaseEvidence,
            "a record the change set adds"
        );
        assert_eq!(
            classify(".ai/repo/releases/v0.6.0.yaml", &derived, &base, &head),
            PathClass::Work,
            "a published record edited"
        );
        assert_eq!(
            classify(".ai/repo/releases/README.md", &derived, &base, &head),
            PathClass::Work
        );
        assert_eq!(
            classify("lib/finish.sh", &derived, &base, &head),
            PathClass::Work
        );
        let none = |_: &str| None;
        assert_eq!(
            classify(version::MANIFEST, &derived, &none, &head),
            PathClass::Work,
            "a new manifest is work"
        );
    }

    #[test]
    fn an_unreadable_trunk_is_unverified_and_names_its_remedy() {
        let o = assemble(
            Observation {
                subject: "feature/x".into(),
                trunk: Err("no trunk".into()),
                declared: Some(v("0.1.0")),
                paths: vec![("lib/a.sh".into(), PathClass::Work)],
                unclassified: None,
            },
            ContractRequirement {
                baseline: None,
                required: Impact::None,
                floor: None,
                unmeasured: Some("x".into()),
            },
            &ReleasePolicy {
                cadence: Impact::Minor,
                trunk: None,
            },
        );
        assert_eq!(o.state, ObligationState::Unverified);
        assert!(o.remedy.unwrap().contains("git fetch"));
        assert_eq!(o.paths.work, 1);
    }

    #[test]
    fn an_owed_obligation_explains_both_inputs_and_its_remedy() {
        let trunk = Trunk {
            reference: "origin/master".into(),
            commit: "0123456789abcdef".into(),
            version: "1.8.0".into(),
            contained: false,
        };
        let o = assemble(
            Observation {
                subject: "feature/x".into(),
                trunk: Ok(trunk),
                declared: Some(v("1.8.0")),
                paths: vec![
                    ("lib/a.sh".into(), PathClass::Work),
                    ("docs/generated/x.json".into(), PathClass::Derived),
                ],
                unclassified: None,
            },
            ContractRequirement {
                baseline: Some("v1.8.0".into()),
                required: Impact::Major,
                floor: Some("2.0.0".into()),
                unmeasured: None,
            },
            &ReleasePolicy {
                cadence: Impact::Minor,
                trunk: None,
            },
        );
        assert_eq!(
            (o.state, o.minimum.as_str(), o.effective),
            (ObligationState::Owed, "2.0.0", Impact::Major)
        );
        assert_eq!(o.id, "feature/x@1.8.0");
        assert_eq!(o.cadence.floor, "1.9.0");
        assert!(o.reasons[0].contains("requires major since v1.8.0"));
        assert!(o.reasons[1].contains("cadence is minor"));
        assert!(o.reasons[2].contains("does not contain"));
        assert!(o.remedy.unwrap().contains("release advance"));
        assert_eq!((o.paths.work, o.paths.derived), (1, 1));
    }

    fn plan(
        baseline: &str,
        required: Impact,
        diagnostics: Vec<crate::release::compat::Diagnostic>,
    ) -> crate::release::compat::VersionPlan {
        use crate::release::compat::*;
        VersionPlan {
            policy: Policy::for_version(v(baseline)),
            baseline: Baseline {
                version: baseline.into(),
                reference: format!("v{baseline}"),
                read_at: "c".into(),
                commit: "c".into(),
                recorded: true,
                atoms: 1,
                fingerprint: "sha256:a".into(),
            },
            declared_version: baseline.into(),
            tool_version: baseline.into(),
            writers_agree: true,
            atoms: 1,
            fingerprint: "sha256:b".into(),
            implied: required,
            required,
            declared: Impact::None,
            required_version: v(baseline).raised_to(required).to_string(),
            status: Status::Ok,
            breaking: false,
            changes: Vec::new(),
            commits: CommitEvidence {
                commits: 0,
                implied: Impact::None,
                breaking: Vec::new(),
            },
            understated: false,
            diagnostics,
        }
    }

    /// The contract half from each answer the analysis can give: a sound plan is a floor, an
    /// unsound one carries its errors and no floor, and no plan carries why.
    #[test]
    fn the_contract_requirement_follows_the_analysis_answer() {
        let sound = contract_of(Ok(plan("1.8.0", Impact::Major, Vec::new())));
        assert_eq!(sound.baseline.as_deref(), Some("v1.8.0"));
        assert_eq!(
            (sound.required, sound.floor.as_deref()),
            (Impact::Major, Some("2.0.0"))
        );
        assert!(sound.unmeasured.is_none());

        let error = crate::release::compat::Diagnostic {
            id: "tag-commit-mismatch".into(),
            severity: crate::release::compat::Severity::Error,
            message: "the tag moved".into(),
        };
        let unsound = contract_of(Ok(plan("1.8.0", Impact::Minor, vec![error])));
        assert_eq!(unsound.baseline.as_deref(), Some("v1.8.0"));
        assert!(
            unsound.floor.is_none(),
            "an unsound plan authorises no floor"
        );
        assert_eq!(unsound.unmeasured.as_deref(), Some("the tag moved"));

        let none = contract_of(Err("nothing published".into()));
        assert_eq!(none.unmeasured.as_deref(), Some("nothing published"));
        assert_eq!(none.required, Impact::None);
    }

    #[test]
    fn every_class_carriage_and_state_has_its_one_word() {
        let classes = [
            (PathClass::Derived, "derived"),
            (PathClass::ReleaseEvidence, "release-evidence"),
            (PathClass::VersionAdvance, "version-advance"),
            (PathClass::Work, "work"),
        ];
        for (c, w) in classes {
            assert_eq!(c.as_str(), w);
            assert_eq!(
                serde_json::to_value(c).unwrap(),
                w,
                "the JSON word is the printed one"
            );
        }
        let carriages = [
            (Carries::Work, "work"),
            (Carries::ReleaseEvidence, "release-evidence"),
            (Carries::VersionAdvance, "version-advance"),
            (Carries::GeneratedSync, "generated-sync"),
            (Carries::Nothing, "nothing"),
        ];
        for (c, w) in carriages {
            assert_eq!(c.as_str(), w);
            assert_eq!(serde_json::to_value(c).unwrap(), w);
        }
        let states = [
            (ObligationState::Satisfied, "satisfied", true),
            (ObligationState::NotOwed, "not-owed", true),
            (ObligationState::Owed, "owed", false),
            (ObligationState::Behind, "behind", false),
            (ObligationState::Unverified, "unverified", false),
        ];
        for (s, w, holds) in states {
            assert_eq!((s.as_str(), s.holds()), (w, holds));
            assert_eq!(serde_json::to_value(s).unwrap(), w);
        }
    }

    fn trunk(version: &str) -> Trunk {
        Trunk {
            reference: "origin/master".into(),
            commit: "0123456789abcdef".into(),
            version: version.into(),
            contained: true,
        }
    }

    fn quiet_contract() -> ContractRequirement {
        ContractRequirement {
            baseline: None,
            required: Impact::None,
            floor: None,
            unmeasured: None,
        }
    }

    /// A trunk whose version is not three numbers, or a tree that declares none, decides
    /// nothing: unverified, with the trunk it read carried for the reader.
    #[test]
    fn a_version_that_is_not_three_numbers_is_unverified() {
        let policy = ReleasePolicy {
            cadence: Impact::Minor,
            trunk: None,
        };
        let o = assemble(
            Observation {
                subject: "feature/x".into(),
                trunk: Ok(trunk("next")),
                declared: Some(v("1.0.0")),
                paths: Vec::new(),
                unclassified: None,
            },
            quiet_contract(),
            &policy,
        );
        assert_eq!(o.state, ObligationState::Unverified);
        assert!(o.reasons[0].contains("the trunk declares 'next'"));
        assert_eq!(o.id, "feature/x@next");

        let o = assemble(
            Observation {
                subject: "feature/x".into(),
                trunk: Ok(trunk("1.0.0")),
                declared: None,
                paths: Vec::new(),
                unclassified: None,
            },
            quiet_contract(),
            &policy,
        );
        assert_eq!(o.state, ObligationState::Unverified);
        assert!(o.reasons[0].contains("declares no version of three numbers"));
        assert_eq!(o.declared, "unknown");
    }

    /// With neither a measurement nor a reason, the contract adds no line, and a change set of
    /// projections owes nothing over a trunk this tree contains.
    #[test]
    fn a_quiet_contract_says_nothing_and_projections_owe_nothing() {
        let o = assemble(
            Observation {
                subject: "chore/derive".into(),
                trunk: Ok(trunk("1.0.0")),
                declared: Some(v("1.0.0")),
                paths: vec![("docs/generated/x.json".into(), PathClass::Derived)],
                unclassified: None,
            },
            quiet_contract(),
            &ReleasePolicy {
                cadence: Impact::Minor,
                trunk: None,
            },
        );
        assert_eq!(o.reasons.len(), 1, "{:?}", o.reasons);
        assert!(o.reasons[0].contains("carries generated-sync and no work"));
        assert_eq!((o.state, o.remedy), (ObligationState::NotOwed, None));
    }

    #[test]
    fn a_manifest_this_tree_lacks_or_cannot_read_is_work() {
        let derived = BTreeSet::new();
        let base_text = "[package]\nversion = \"0.5.0\"\n".to_string();
        let base = |_: &str| Some(base_text.clone());
        let none = |_: &str| None;
        assert_eq!(
            classify(version::MANIFEST, &derived, &base, &none),
            PathClass::Work,
            "deleted here"
        );
        let unreadable = |_: &str| Some("[package]\nname = \"x\"\n".to_string());
        assert_eq!(
            classify(version::LOCK, &derived, &base, &unreadable),
            PathClass::Work,
            "no version to compare"
        );
    }

    #[test]
    fn the_work_sample_is_capped_and_the_count_is_whole() {
        let paths: Vec<(String, PathClass)> = (0..WORK_EXAMPLES + 3)
            .map(|i| (format!("lib/{i:02}.sh"), PathClass::Work))
            .collect();
        let o = assemble(
            Observation {
                subject: "feature/x".into(),
                trunk: Ok(trunk("1.0.0")),
                declared: Some(v("1.0.0")),
                paths,
                unclassified: None,
            },
            quiet_contract(),
            &ReleasePolicy {
                cadence: Impact::Minor,
                trunk: None,
            },
        );
        assert_eq!(o.paths.work, WORK_EXAMPLES + 3);
        assert_eq!(o.paths.work_examples.len(), WORK_EXAMPLES);
    }

    /// A release record is evidence once: when the change set adds it. Editing or deleting a
    /// published record is authored work, so a hand edit cannot ride the record's exemption.
    #[test]
    fn a_release_record_is_evidence_only_when_the_change_set_adds_it() {
        let derived = BTreeSet::new();
        let text = |_: &str| Some("schema: release/v1\nversion: 1.5.0\n".to_string());
        let other = |_: &str| Some("schema: release/v1\nversion: 1.5.0\nnote: x\n".to_string());
        let none = |_: &str| None;
        let record = ".ai/repo/releases/v1.5.0.yaml";
        assert_eq!(
            classify(record, &derived, &none, &text),
            PathClass::ReleaseEvidence
        );
        assert_eq!(classify(record, &derived, &text, &other), PathClass::Work);
        assert_eq!(
            classify(record, &derived, &text, &none),
            PathClass::Work,
            "a deleted record"
        );
        assert_eq!(
            classify(".ai/repo/releases/old/v1.0.0.yaml", &derived, &none, &text),
            PathClass::Work,
            "only a record the reader loads is one"
        );
        // an added record's projections, once the unit has named them, are derived
        let changed = vec![
            record.to_string(),
            "site/static/releases/v1.5.0.json".to_string(),
        ];
        let unit = release_unit(&changed, &none, &text);
        assert_eq!(
            classify("site/static/releases/v1.5.0.json", &unit, &none, &text),
            PathClass::Derived
        );
        // an edited record names no projections at all
        assert!(release_unit(&changed, &text, &other).is_empty());
    }

    /// The change set of #748 (e08aec8651), path for path where it matters: a new record, its
    /// public metadata, the stable pointer, and `.gitattributes` gaining the one line that
    /// marks the new metadata `merge=derived` — plus the generated block's path count, a
    /// comment. The trunk's attributes cannot mark a file the trunk does not have, so before
    /// the unit was read from the generator the JSON was work and the record owed a minor.
    #[test]
    fn a_release_record_with_its_generated_metadata_and_attribute_line_is_one_unit() {
        let attrs_before = "# 165 path(s) from docs/generated/artifacts.json\n\
                            site/static/releases/latest.json merge=derived\n\
                            site/static/releases/v0.12.0.json merge=derived\n";
        let attrs_after = "# 166 path(s) from docs/generated/artifacts.json\n\
                           site/static/releases/latest.json merge=derived\n\
                           site/static/releases/v0.12.0.json merge=derived\n\
                           site/static/releases/v0.13.0.json   merge=derived\n";
        let changed: Vec<String> = [
            ".ai/repo/releases/v0.13.0.yaml",
            ".gitattributes",
            "site/static/releases/latest.json",
            "site/static/releases/v0.13.0.json",
        ]
        .map(String::from)
        .to_vec();
        let base_with = |attrs: &'static str| {
            move |p: &str| match p {
                ".gitattributes" => Some(attrs.to_string()),
                "site/static/releases/latest.json" => Some("{}".to_string()),
                _ => None,
            }
        };
        let head_with = |attrs: &'static str| {
            move |p: &str| match p {
                ".gitattributes" => Some(attrs.to_string()),
                _ => Some("{}".to_string()),
            }
        };
        // the trunk marks latest.json, and not the new metadata
        let mut derived: BTreeSet<String> = ["site/static/releases/latest.json".to_string()].into();
        derived.extend(release_unit(
            &changed,
            &base_with(attrs_before),
            &head_with(attrs_after),
        ));
        let classes: Vec<PathClass> = changed
            .iter()
            .map(|p| {
                classify(
                    p,
                    &derived,
                    &base_with(attrs_before),
                    &head_with(attrs_after),
                )
            })
            .collect();
        assert_eq!(
            classes,
            [
                PathClass::ReleaseEvidence,
                PathClass::Derived,
                PathClass::Derived,
                PathClass::Derived
            ]
        );
        assert_eq!(Carries::of(classes), Carries::ReleaseEvidence);

        // .gitattributes that also marks a path the generator does not write from the record
        let sneaky = "site/static/releases/latest.json merge=derived\n\
                      site/static/releases/v0.12.0.json merge=derived\n\
                      site/static/releases/v0.13.0.json merge=derived\n\
                      lib/finish.sh merge=derived\n";
        assert!(
            !release_unit(&changed, &base_with(attrs_before), &head_with(sneaky))
                .contains(".gitattributes")
        );
        // ... or that removes an attribute
        let removes = "site/static/releases/v0.13.0.json merge=derived\n\
                       site/static/releases/v0.12.0.json merge=derived\n";
        assert!(
            !release_unit(&changed, &base_with(attrs_before), &head_with(removes))
                .contains(".gitattributes")
        );
        // ... or that changes another attribute's value
        let retargets = "site/static/releases/latest.json merge=union\n\
                         site/static/releases/v0.12.0.json merge=derived\n\
                         site/static/releases/v0.13.0.json merge=derived\n";
        assert!(
            !release_unit(&changed, &base_with(attrs_before), &head_with(retargets))
                .contains(".gitattributes")
        );
        // ... or that only rewrites a comment, which marks nothing the record needs
        let comment = "# 166 path(s)\nsite/static/releases/latest.json merge=derived\n\
                       site/static/releases/v0.12.0.json merge=derived\n";
        assert!(
            !release_unit(&changed, &base_with(attrs_before), &head_with(comment))
                .contains(".gitattributes")
        );
        // no record added: nothing is a projection of one
        let without: Vec<String> = changed[1..].to_vec();
        assert!(
            release_unit(&without, &base_with(attrs_before), &head_with(attrs_after)).is_empty()
        );
    }

    /// Trunk attributes nobody could read classify nothing, and the verdict is unverified —
    /// never the empty set of projections that would make every derived path work.
    #[test]
    fn an_unclassified_change_set_is_unverified() {
        let o = assemble(
            Observation {
                subject: "feature/x".into(),
                trunk: Ok(trunk("1.0.0")),
                declared: Some(v("1.0.0")),
                paths: Vec::new(),
                unclassified: Some("git check-attr failed".into()),
            },
            quiet_contract(),
            &ReleasePolicy {
                cadence: Impact::Minor,
                trunk: None,
            },
        );
        assert_eq!(o.state, ObligationState::Unverified);
        assert!(o.reasons[0].contains("git check-attr failed"));
        assert_eq!(o.trunk.map(|t| t.version), Some("1.0.0".into()));
    }
}
