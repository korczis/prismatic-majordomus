//! The preflight: whether Majordomus is actually in force in this checkout, and what proves
//! each part of that claim.
//!
//! The [`super::RepositoryEnvironment`] says what this checkout *is* — its branch, its
//! toolchains, where a server published its address. It does not say what is *proven*: an
//! address that accepts a connection is not a server of this version serving this checkout,
//! a rule file on disk is not a rule anything enforces, and a recorded test run is not a run
//! of the tree in front of you. Until this module, the entry banner drew a `✓` beside the
//! Cockpit on the strength of a TCP connection while the server behind it was two versions
//! old. The preflight is the value that refuses that.
//!
//! # One verdict per claim, and no verdict without evidence
//!
//! Every [`Check`] carries a [`Verdict`] and the [`Evidence`] behind it. The three verdicts
//! that assert something is in force — [`Verdict::Verified`], [`Verdict::Active`] and
//! [`Verdict::Fresh`] — cannot be constructed without evidence: [`Check::new`] turns such a
//! claim into [`Verdict::Unknown`] and says why. That is the whole rule
//! `project.entry-reports-only-evidence` enforces, and it is enforced in the type rather
//! than in each renderer.
//!
//! # Two halves: observing and deriving
//!
//! [`observe`] reads — the lease, one loopback request, the session and task records, the
//! evidence ledger, one `git log` of the deployment ref — and returns [`Observations`], plain
//! data. [`derive`] turns observations into a [`Preflight`] and reads nothing, so every
//! verdict in this file is a test over a value rather than over a machine. The command line,
//! the HTTP route, the MCP tool and resource, the Cockpit and the entry banner all render
//! the value [`derive`] returns.
//!
//! # What it never does
//!
//! It never builds the index on the entry path (the rule tally comes from the cache a full
//! resolution wrote, and says when it was counted), never contacts anything but the loopback
//! address a server of this checkout published, never runs a test, a build or a generator,
//! and never writes outside `.ai/local/`.
//!
//! ```
//! use majordomus_cli::environment::preflight::{derive, Observations, Verdict};
//!
//! // a checkout nobody has observed anything about
//! let preflight = derive(&Observations::empty("demo", 0));
//! // nothing is claimed in force, and nothing absent is dressed up as present
//! assert!(preflight.sections.iter().flat_map(|s| &s.checks).all(|c| !c.verdict.proves()));
//! assert_eq!(preflight.check("integration.server").unwrap().verdict, Verdict::Unknown);
//! assert_eq!(preflight.check("verification.deployment").unwrap().verdict, Verdict::Unavailable);
//! ```

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::capability::builtin::continuity::{self, Freshness, Thresholds};
use crate::capability::builtin::server::{standing_of, ServerStanding};
use crate::evidence::Ledger;
use crate::graph::{Graph, Node};
use crate::lease::{self, LeaseFile};
use crate::policy::LoadedPolicy;
use crate::rules::{RuleState, RulesReport};

use super::cache::{Cache, Entry};
use super::{ProjectionState, RepositoryEnvironment, Resolution};

/// The schema identity of [`Preflight`].
pub const SCHEMA: &str = "majordomus/preflight";

/// The version of [`SCHEMA`]; it rises when a reader of the old shape would misread the new.
pub const SCHEMA_VERSION: u32 = 1;

/// The ref the site deployment is read from: the remote-tracking copy of the branch
/// `scripts/pages` publishes, whose every commit names the source commit it was built from
/// (`source: <sha>`). A local ref, so reading it contacts nothing — and it is only as recent
/// as the last fetch, which every verdict built on it says.
pub const PAGES_REF: &str = "refs/remotes/origin/gh-pages";

/// How long the one loopback request to a published server may take. Loopback answers in
/// about a millisecond or is not there; anything slower is a machine under load, and a
/// shell prompt is not the place to wait for it.
pub const ANSWER_BUDGET: Duration = Duration::from_millis(250);

/// How long the peer board may take when a person asked for the preflight. The board gathers
/// every checkout's, which is tens of milliseconds on a quiet machine and more on a loaded one;
/// entry never asks it, so this budget is spent only when somebody is waiting for the answer.
pub const BOARD_BUDGET: Duration = Duration::from_secs(1);

/// The path under the local half where a full resolution leaves the rule tally for the
/// entry path to read. It lives in the environment cache file, as one more tier.
pub const RULES_TIER: &str = "rules";

/// The fingerprint of the cache tier holding the join of decisions to the task in progress.
/// The value carries the task, scope and commit it was joined for, and the preflight judges
/// it against those, so the fingerprint only names the tier.
pub const ADR_RELEVANCE_TIER: &str = "adr-relevance";

/// What the repository can show about one claim.
///
/// Nine words, not a boolean, because the actions they call for differ: `stale` is re-run
/// the proof, `unavailable` is nothing is there, `unknown` is nobody looked, and
/// `not_applicable` is there is nothing to look for.
///
/// ```
/// use majordomus_cli::environment::preflight::Verdict;
/// assert!(Verdict::Verified.proves() && Verdict::Active.proves() && Verdict::Fresh.proves());
/// assert!(!Verdict::Stale.proves() && !Verdict::NotApplicable.proves());
/// assert_eq!(Verdict::NotApplicable.as_str(), "not_applicable");
/// // the most urgent first: a failure outranks a stale proof, which outranks an absence
/// assert!(Verdict::Failed.urgency() < Verdict::Stale.urgency());
/// assert_eq!(Verdict::Verified.urgency(), None);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[schemars(rename = "PreflightVerdict")]
pub enum Verdict {
    /// Proven against this tree by a recorded result or a direct answer.
    Verified,
    /// In force now, observed directly, with nothing recorded to verify.
    Active,
    /// Judged recent enough to act on, against a declared threshold or the current commit.
    Fresh,
    /// Evidence exists, but it is about another commit, tree or moment.
    Stale,
    /// In force, but in a form that is not the one this executable would serve.
    Degraded,
    /// Nothing is there to observe.
    Unavailable,
    /// Observed, and it did not hold.
    Failed,
    /// Nobody looked, or the looking did not finish.
    Unknown,
    /// There is nothing here to judge.
    NotApplicable,
}

impl Verdict {
    /// The word as serialised and printed.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::Verdict;
    /// assert_eq!(Verdict::Degraded.as_str(), "degraded");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Verified => "verified",
            Verdict::Active => "active",
            Verdict::Fresh => "fresh",
            Verdict::Stale => "stale",
            Verdict::Degraded => "degraded",
            Verdict::Unavailable => "unavailable",
            Verdict::Failed => "failed",
            Verdict::Unknown => "unknown",
            Verdict::NotApplicable => "not_applicable",
        }
    }

    /// Does this verdict assert that something is in force? Only these three need evidence
    /// to be constructed, and only these three may be drawn as a success.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::Verdict;
    /// assert!(Verdict::Fresh.proves());
    /// assert!(!Verdict::Unknown.proves());
    /// ```
    pub fn proves(self) -> bool {
        matches!(self, Verdict::Verified | Verdict::Active | Verdict::Fresh)
    }

    /// How urgently a person should look, lowest first; `None` for a verdict that needs no
    /// attention.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::Verdict;
    /// assert_eq!(Verdict::Failed.urgency(), Some(0));
    /// assert_eq!(Verdict::NotApplicable.urgency(), None);
    /// ```
    pub fn urgency(self) -> Option<u8> {
        match self {
            Verdict::Failed => Some(0),
            Verdict::Degraded => Some(1),
            Verdict::Stale => Some(2),
            Verdict::Unavailable => Some(3),
            Verdict::Unknown => Some(4),
            Verdict::Verified | Verdict::Active | Verdict::Fresh | Verdict::NotApplicable => None,
        }
    }

    /// The status word the Cockpit's design system files this verdict under.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::Verdict;
    /// assert_eq!(Verdict::Verified.status(), "ok");
    /// assert_eq!(Verdict::Failed.status(), "fail");
    /// ```
    pub fn status(self) -> &'static str {
        match self {
            Verdict::Verified | Verdict::Active | Verdict::Fresh => "ok",
            Verdict::Stale | Verdict::Degraded | Verdict::Unknown => "warn",
            Verdict::Failed => "fail",
            Verdict::Unavailable | Verdict::NotApplicable => "info",
        }
    }
}

/// One thing a verdict rests on: where it was read and what was seen there.
///
/// ```
/// use majordomus_cli::environment::preflight::Evidence;
/// let e = Evidence::new(".ai/repo/evidence/ledger.json", "6 recorded runs");
/// assert_eq!(e.source, ".ai/repo/evidence/ledger.json");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "PreflightEvidence")]
pub struct Evidence {
    /// The file, command, route or capability that was read.
    pub source: String,
    /// What it said, in one phrase.
    pub observed: String,
}

impl Evidence {
    /// Evidence read from `source`: the file, command or route that was read, and the phrase
    /// saying what it held at the moment it was read.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::Evidence;
    /// assert_eq!(Evidence::new("GET /", "leaseholder").observed, "leaseholder");
    /// ```
    pub fn new(source: impl Into<String>, observed: impl Into<String>) -> Self {
        Evidence {
            source: source.into(),
            observed: observed.into(),
        }
    }
}

/// One claim, its verdict, and what the verdict rests on.
///
/// ```
/// use majordomus_cli::environment::preflight::{Check, Evidence, Verdict};
///
/// // a claim of something in force without evidence is not accepted as one
/// let bare = Check::new("integration.mcp", "MCP", Verdict::Verified, "healthy", vec![]);
/// assert_eq!(bare.verdict, Verdict::Unknown);
/// assert!(bare.summary.contains("no evidence"));
///
/// let backed = Check::new(
///     "integration.mcp", "MCP", Verdict::Verified, "served",
///     vec![Evidence::new("GET /", "surface mcp ready")],
/// );
/// assert_eq!(backed.verdict, Verdict::Verified);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "PreflightCheck")]
pub struct Check {
    /// A stable dotted id, `<section>.<claim>`.
    pub id: String,
    /// The short name a person reads.
    pub title: String,
    /// What the repository can show.
    pub verdict: Verdict,
    /// The verdict in one sentence, with the numbers that decided it.
    pub summary: String,
    /// Everything the verdict rests on. Never empty for a verdict that proves something.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<Evidence>,
    /// The command that shows more, or that changes the verdict.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

impl Check {
    /// A check. A verdict that [`Verdict::proves`] something and arrives without evidence
    /// becomes [`Verdict::Unknown`], with the summary saying so.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::{Check, Verdict};
    /// let c = Check::new("x.y", "Y", Verdict::Stale, "counted at another commit", vec![]);
    /// assert_eq!(c.verdict, Verdict::Stale, "only a claim of force needs evidence");
    /// ```
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        verdict: Verdict,
        summary: impl Into<String>,
        evidence: Vec<Evidence>,
    ) -> Self {
        let summary = summary.into();
        let (verdict, summary) = if verdict.proves() && evidence.is_empty() {
            (
                Verdict::Unknown,
                format!(
                    "claimed {} with no evidence, so not claimed: {summary}",
                    verdict.as_str()
                ),
            )
        } else {
            (verdict, summary)
        };
        Check {
            id: id.into(),
            title: title.into(),
            verdict,
            summary,
            evidence,
            next: None,
        }
    }

    /// The same check, naming the command that shows more.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::{Check, Verdict};
    /// let c = Check::new("x.y", "Y", Verdict::Unknown, "not probed", vec![])
    ///     .next("majordomus serve status");
    /// assert_eq!(c.next.as_deref(), Some("majordomus serve status"));
    /// ```
    pub fn next(mut self, command: impl Into<String>) -> Self {
        self.next = Some(command.into());
        self
    }
}

/// A group of checks: repository, session, governance, integration, verification.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, Observations, Section};
/// let p = derive(&Observations::empty("demo", 0));
/// let ids: Vec<&str> = p.sections.iter().map(|s| s.id.as_str()).collect();
/// assert_eq!(ids, ["repository", "session", "governance", "integration", "verification"]);
/// // every check in a section carries the section's id as its prefix
/// let session: &Section = &p.sections[1];
/// assert!(session.checks.iter().all(|c| c.id.starts_with("session.")));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "PreflightSection")]
pub struct Section {
    /// A stable id, the prefix of every check in it.
    pub id: String,
    /// The heading a person reads.
    pub title: String,
    /// The checks, in the order they are rendered.
    pub checks: Vec<Check>,
}

/// Whether Majordomus is in force in this checkout, claim by claim, with the evidence.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, Observations, Preflight};
/// let p = derive(&Observations::empty("demo", 0));
/// assert_eq!(p.schema, Preflight::schema_id());
/// assert_eq!(p.verdicts().len(), p.sections.iter().map(|s| s.checks.len()).sum::<usize>());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Preflight {
    /// `majordomus/preflight/v1`.
    pub schema: String,
    /// When the observations were taken, RFC 3339 in UTC.
    pub generated_at: String,
    /// How the environment behind it was resolved.
    pub resolution: Resolution,
    /// The repository's name.
    pub repository: String,
    /// The branch, when git named one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// The commit HEAD names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// The groups of checks.
    pub sections: Vec<Section>,
    /// The ids of every check that needs a look, most urgent first.
    pub attention: Vec<String>,
}

impl Preflight {
    /// The schema identity a document of this shape carries.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::Preflight;
    /// assert_eq!(Preflight::schema_id(), "majordomus/preflight/v1");
    /// ```
    pub fn schema_id() -> String {
        format!("{SCHEMA}/v{SCHEMA_VERSION}")
    }

    /// One check by its dotted id, or `None` for an id no section declares — which a caller
    /// must not read as a check that passed.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::{derive, Observations};
    /// let p = derive(&Observations::empty("demo", 0));
    /// assert!(p.check("session.episode").is_some());
    /// assert!(p.check("nothing.here").is_none());
    /// ```
    pub fn check(&self, id: &str) -> Option<&Check> {
        self.sections
            .iter()
            .flat_map(|s| &s.checks)
            .find(|c| c.id == id)
    }

    /// Every check's verdict by id: the part two surfaces must agree on.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::{derive, Observations, Verdict};
    /// let v = derive(&Observations::empty("demo", 0)).verdicts();
    /// assert_eq!(v["verification.coverage"], Verdict::Unavailable);
    /// ```
    pub fn verdicts(&self) -> BTreeMap<String, Verdict> {
        self.sections
            .iter()
            .flat_map(|s| &s.checks)
            .map(|c| (c.id.clone(), c.verdict))
            .collect()
    }
}

// ---------------------------------------------------------------- observations

/// What git said about the work tree, taken from the snapshot's one `git status` rather than
/// asked again.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, GitObservation, Observations, Verdict};
/// let mut o = Observations::empty("demo", 0);
/// assert_eq!(derive(&o).check("repository.git").unwrap().verdict, Verdict::Unavailable);
/// o.git = Some(GitObservation { branch: None, head: Some("a".repeat(40)), clean: false, changed: 3 });
/// let git = derive(&o);
/// let c = git.check("repository.git").unwrap();
/// assert_eq!(c.verdict, Verdict::Active);
/// assert!(c.summary.contains("detached") && c.summary.contains("3 path(s) changed"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GitObservation {
    /// The branch, when HEAD names one.
    pub branch: Option<String>,
    /// The commit HEAD names.
    pub head: Option<String>,
    /// Whether nothing is staged, modified, untracked or conflicted.
    pub clean: bool,
    /// Paths that are not clean.
    pub changed: u32,
}

/// The open episode, read from `session-current.yaml`, with whether it is this checkout's.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, EpisodeObservation, Observations, Verdict};
/// let mut o = Observations::empty("demo", 0);
/// o.episode = Some(EpisodeObservation { id: "s-1".into(), this_checkout: false, ..Default::default() });
/// // an episode another checkout opened is not this work
/// assert_eq!(derive(&o).check("session.episode").unwrap().verdict, Verdict::Degraded);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EpisodeObservation {
    /// The episode id.
    pub id: String,
    /// When it opened.
    pub started_at: String,
    /// The provider that opened it, when one did.
    pub provider: String,
    /// The commit its briefing was written at.
    pub start_head: String,
    /// `clean` or `dirty` when its briefing was written.
    pub start_working_tree: String,
    /// Whether the record belongs to this checkout.
    pub this_checkout: bool,
}

/// The task record, read from `current.yaml` through the continuity module's own reader.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, Observations, TaskObservation, Verdict};
/// let mut o = Observations::empty("demo", 0);
/// o.task = Some(TaskObservation { id: "t-1".into(), outcome: "active".into(), ..Default::default() });
/// assert_eq!(derive(&o).check("session.task").unwrap().verdict, Verdict::Active);
/// o.task = Some(TaskObservation { id: "t-1".into(), outcome: "handed_over".into(), ..Default::default() });
/// assert_eq!(derive(&o).check("session.task").unwrap().verdict, Verdict::NotApplicable);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaskObservation {
    /// The task id.
    pub id: String,
    /// What it is.
    pub task: String,
    /// `active` (or empty, for an older record) while it is in progress; the outcome once it
    /// ended.
    pub outcome: String,
    /// When it started.
    pub started_at: String,
    /// The paths it declared as its scope, as the record holds them.
    pub scope: Vec<String>,
}

/// The handover a resuming worker would be given, judged by continuity's own freshness rule.
///
/// ```
/// use majordomus_cli::capability::builtin::continuity::Freshness;
/// use majordomus_cli::environment::preflight::{derive, HandoverObservation, Observations, Verdict};
/// let mut o = Observations::empty("demo", 0);
/// o.handover = Some(HandoverObservation {
///     path: "h.md".into(), freshness: Freshness::Aging, reason: "5h old".into(), divergence: "exact".into(),
/// });
/// assert_eq!(derive(&o).check("session.handover").unwrap().verdict, Verdict::Fresh);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoverObservation {
    /// Repository-relative path.
    pub path: String,
    /// How old it is, against the policy's thresholds.
    pub freshness: Freshness,
    /// Why, in one phrase.
    pub reason: String,
    /// Where its commit sits relative to HEAD.
    pub divergence: String,
}

/// Whether the policy parsed, was refused, or was never read by this call.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, Observations, PolicyObservation, Verdict};
/// let mut o = Observations::empty("demo", 0);
/// o.policy = PolicyObservation::Refused("unknown key".into());
/// assert_eq!(derive(&o).check("governance.policy").unwrap().verdict, Verdict::Failed);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PolicyObservation {
    /// It parsed.
    Parsed,
    /// It did not; the reason.
    Refused(String),
    /// Nobody read it.
    #[default]
    NotRead,
}

/// The rule corpus, counted by [`crate::rules::report`] against a commit.
///
/// ```
/// use majordomus_cli::environment::preflight::RulesTally;
/// use majordomus_cli::evidence::Ledger;
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// let report = majordomus_cli::rules::report(&repo.index().unwrap(), &Ledger::empty());
/// let tally = RulesTally::of(&report);
/// assert_eq!(tally.rules, report.coverage.rules);
/// assert_eq!(tally.proven, 0, "an empty ledger proves nothing");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "PreflightRulesTally")]
pub struct RulesTally {
    /// The commit it was counted at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// `clean`, `dirty` or `unknown` when it was counted.
    pub working_tree: String,
    /// Rules discovered.
    pub rules: usize,
    /// Of those, blocking.
    pub blocking: usize,
    /// Of those, the repository's own (`project.`); the rest are vendored.
    pub project: usize,
    /// Rules a CI gate runs.
    pub gated: usize,
    /// Rules in each state, by the state's printed word.
    pub states: BTreeMap<String, usize>,
    /// Rules proven against the tree they were counted at.
    pub proven: usize,
    /// Rules whose latest recorded run failed.
    pub failing: usize,
    /// Rules that declare a reader enforces them.
    pub reviewed: usize,
    /// Rules naming neither a validator nor a test.
    pub unproven: usize,
    /// Rules whose class their proof does not support.
    pub findings: usize,
}

impl RulesTally {
    /// The tally of one report: the counts entry needs, stamped with the commit and tree the
    /// report was derived against, so a later reader can tell when it went stale.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::RulesTally;
    /// use majordomus_cli::rules::RulesReport;
    /// let empty = RulesReport { head: None, working_tree: "clean".into(), rules: vec![],
    ///     states: Default::default(), coverage: Default::default(), findings: vec![], satisfied: true };
    /// assert_eq!(RulesTally::of(&empty).rules, 0);
    /// ```
    pub fn of(report: &RulesReport) -> Self {
        let count = |s: RuleState| report.states.get(s.label()).copied().unwrap_or(0);
        RulesTally {
            head: report.head.clone(),
            working_tree: report.working_tree.clone(),
            rules: report.coverage.rules,
            blocking: report.coverage.blocking,
            project: report
                .rules
                .iter()
                .filter(|r| r.rule.id.starts_with("project."))
                .count(),
            gated: report.coverage.gated,
            states: report.states.clone(),
            proven: count(RuleState::Proven),
            failing: count(RuleState::Failing),
            reviewed: count(RuleState::Reviewed),
            unproven: count(RuleState::Unproven),
            findings: report.findings.len(),
        }
    }
}

/// Where the rule tally came from: counted now, taken from the cache, or not there at all.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, Observations, RulesObservation, RulesTally, Verdict};
/// let mut o = Observations::empty("demo", 0);
/// assert_eq!(derive(&o).check("governance.rules").unwrap().verdict, Verdict::Unknown);
/// // a cached tally with no commit to compare against cannot be current
/// o.rules = RulesObservation::Cached(RulesTally { rules: 4, ..Default::default() });
/// assert_eq!(derive(&o).check("governance.rules").unwrap().verdict, Verdict::Stale);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulesObservation {
    /// Counted in this process.
    Counted(RulesTally),
    /// Taken from the cache a full resolution wrote.
    Cached(RulesTally),
    /// Neither.
    Absent,
}

/// One declared relation between a decision and the active task: the decision's `related`
/// list names a file or a test whose path lies inside a path the task declared as its scope,
/// or names a directory the declared path lies inside.
///
/// The relation is read from the `adrs` graph's `put_in_force` edges, which is the one
/// resolution of an ADR's typed references there is; nothing here matches words.
///
/// ```
/// use majordomus_cli::environment::preflight::AdrRelation;
/// let r = AdrRelation {
///     adr: "adr-0066".into(),
///     status: Some("proposed".into()),
///     reference: "test:apps/majordomus-cli/tests/preflight.rs".into(),
///     scope: "apps/majordomus-cli".into(),
/// };
/// assert_eq!(
///     r.describe(),
///     "adr-0066 (proposed) → test apps/majordomus-cli/tests/preflight.rs → scope apps/majordomus-cli",
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "PreflightAdrRelation")]
pub struct AdrRelation {
    /// The id the decision declares, `adr-NNNN`.
    pub adr: String,
    /// The decision's status word, when it declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// The reference the decision declares, `file:<path>` or `test:<path>`.
    pub reference: String,
    /// The path of the task's scope the reference meets.
    pub scope: String,
}

impl AdrRelation {
    /// The relation as one line of evidence: the decision, the path it names, and the scope
    /// path that path meets.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::AdrRelation;
    /// let r = AdrRelation { adr: "adr-0011".into(), status: None,
    ///     reference: "file:lib".into(), scope: "lib/session.sh".into() };
    /// assert_eq!(r.describe(), "adr-0011 → file lib → scope lib/session.sh");
    /// ```
    pub fn describe(&self) -> String {
        let (kind, path) = self
            .reference
            .split_once(':')
            .unwrap_or(("file", self.reference.as_str()));
        let status = self
            .status
            .as_deref()
            .map(|s| format!(" ({s})"))
            .unwrap_or_default();
        format!("{}{status} → {kind} {path} → scope {}", self.adr, self.scope)
    }
}

/// The decisions joined to one task: which ADRs name a path the task's scope meets, at the
/// commit the join was made.
///
/// Relevance here means exactly that declared relation and nothing wider: a decision that
/// names no file or test is never relevant to any task, and one naming a file under a broad
/// scope is relevant to every task with that scope. A decision withdrawn from force — its
/// status `superseded`, `rejected` or `deprecated`, or another decision's `supersedes` naming
/// it — is not joined.
///
/// ```
/// use majordomus_cli::environment::preflight::AdrRelevance;
/// use majordomus_cli::graph::derive;
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// let index = repo.index().unwrap();
/// let registry = majordomus_cli::capability::CapabilityRegistry::builder().build().unwrap();
/// let graph = derive("adrs", &registry, &index).unwrap();
/// let joined = AdrRelevance::join(&graph, "t-1", &["no/such/path".into()], Some("abc"));
/// assert_eq!(joined.task, "t-1");
/// assert!(joined.relations.is_empty(), "a scope nothing names reaches no decision");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "PreflightAdrRelevance")]
pub struct AdrRelevance {
    /// The task id the join was made for.
    pub task: String,
    /// The scope it was joined against, normalised.
    pub scope: Vec<String>,
    /// The commit it was joined at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// Every relation found, sorted.
    pub relations: Vec<AdrRelation>,
}

/// A decision status that takes the decision out of force.
const WITHDRAWN: &[&str] = &["superseded", "rejected", "deprecated"];

impl AdrRelevance {
    /// Join the `adrs` graph to a task's scope: every `put_in_force` edge into a `file` or
    /// `test` node whose path lies under a scope path, or above one.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::AdrRelevance;
    /// use majordomus_cli::graph::{Edge, Graph, GraphMetadata, Node};
    /// let node = |id: &str, kind: &str, label: &str, source: Option<&str>| Node {
    ///     id: id.into(), kind: kind.into(), label: label.into(), summary: None, route: None,
    ///     source: source.map(Into::into), status: None, external: false, facts: Default::default(),
    /// };
    /// let graph = Graph {
    ///     id: "adrs".into(), title: String::new(), description: String::new(), source: String::new(),
    ///     node_kinds: Default::default(), edge_kinds: Default::default(),
    ///     nodes: vec![
    ///         node("adr:a", "adr", "adr-0001", None),
    ///         node("file:src/x.rs", "file", "src/x.rs", Some("src/x.rs")),
    ///     ],
    ///     edges: vec![Edge { source: "adr:a".into(), target: "file:src/x.rs".into(), kind: "put_in_force".into() }],
    ///     metadata: GraphMetadata { nodes: 2, edges: 1, acyclic: true, truncated: false },
    /// };
    /// let hit = AdrRelevance::join(&graph, "t", &["src/".into()], None);
    /// assert_eq!(hit.adrs(), vec!["adr-0001"]);
    /// assert_eq!(hit.scope, vec!["src"]);
    /// // a sibling path whose name only begins the same is not inside the scope
    /// assert!(AdrRelevance::join(&graph, "t", &["sr".into()], None).relations.is_empty());
    /// ```
    pub fn join(graph: &Graph, task: &str, scope: &[String], head: Option<&str>) -> Self {
        let scope = normalised_scope(scope);
        let nodes: BTreeMap<&str, &Node> =
            graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        // a decision another stands in for is out of force whatever its own status says
        let superseded: std::collections::BTreeSet<&str> = graph
            .edges
            .iter()
            .filter(|e| e.kind == "supersedes")
            .map(|e| e.target.as_str())
            .collect();
        let mut relations = Vec::new();
        for e in graph.edges.iter().filter(|e| e.kind == "put_in_force") {
            if superseded.contains(e.source.as_str()) {
                continue;
            }
            let (Some(adr), Some(target)) =
                (nodes.get(e.source.as_str()), nodes.get(e.target.as_str()))
            else {
                continue;
            };
            if adr.kind != "adr"
                || !matches!(target.kind.as_str(), "file" | "test")
                || adr
                    .status
                    .as_deref()
                    .is_some_and(|s| WITHDRAWN.contains(&s))
            {
                continue;
            }
            let Some(path) = target.source.as_deref() else {
                continue;
            };
            let path = path.trim_end_matches('/');
            for s in &scope {
                if is_within(s, path) || is_within(path, s) {
                    relations.push(AdrRelation {
                        adr: adr.label.clone(),
                        status: adr.status.clone(),
                        reference: format!("{}:{path}", target.kind),
                        scope: s.clone(),
                    });
                }
            }
        }
        relations.sort();
        relations.dedup();
        AdrRelevance {
            task: task.into(),
            scope,
            head: head.map(Into::into),
            relations,
        }
    }

    /// The distinct decisions joined, sorted.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::AdrRelevance;
    /// assert!(AdrRelevance::default().adrs().is_empty());
    /// ```
    pub fn adrs(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.relations.iter().map(|r| r.adr.as_str()).collect();
        ids.dedup();
        ids
    }
}

/// A task's declared scope as the join compares it: trimmed, without a trailing slash, and
/// without the paths no repository-relative path can lie under.
fn normalised_scope(scope: &[String]) -> Vec<String> {
    let mut out: Vec<String> = scope
        .iter()
        .map(|s| s.trim().trim_start_matches("./").trim_end_matches('/'))
        .filter(|s| !s.is_empty() && !s.starts_with('/') && !s.split('/').any(|p| p == ".."))
        .map(Into::into)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Is `path` the same as `under`, or below it? A whole path segment, never a prefix of one.
fn is_within(under: &str, path: &str) -> bool {
    under == "." || path == under || path.starts_with(&format!("{under}/"))
}

/// Where the join of decisions to the task came from: made in this process, taken from the
/// cache, or not there at all.
///
/// ```
/// use majordomus_cli::environment::preflight::{
///     derive, AdrRelevance, AdrRelevanceObservation, Observations, TaskObservation, Verdict,
/// };
/// let mut o = Observations::empty("demo", 0);
/// o.adrs = Some(3);
/// o.task = Some(TaskObservation { id: "t-1".into(), outcome: "active".into(),
///     scope: vec!["src".into()], ..Default::default() });
/// // a join made for another task says nothing about this one
/// o.adr_relevance = AdrRelevanceObservation::Cached(AdrRelevance { task: "t-0".into(), ..Default::default() });
/// assert_eq!(derive(&o).check("governance.adrs").unwrap().verdict, Verdict::Unknown);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AdrRelevanceObservation {
    /// Joined in this process, over the index.
    Joined(AdrRelevance),
    /// Taken from the cache a full preflight wrote.
    Cached(AdrRelevance),
    /// Neither.
    #[default]
    Absent,
}

/// What the lease named and what the address it names answered, if it was asked.
///
/// ```
/// use majordomus_cli::capability::builtin::server::ServerStanding;
/// use majordomus_cli::environment::preflight::{derive, Observations, ServerObservation, Verdict};
/// let mut o = Observations::empty("demo", 0);
/// o.server = ServerObservation {
///     standing: Some(ServerStanding::Outdated),
///     reason: Some("the server is serving version 0.6.1".into()),
///     ..Default::default()
/// };
/// assert_eq!(derive(&o).check("integration.server").unwrap().verdict, Verdict::Degraded);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerObservation {
    /// Where it stands; `None` when there is a lease and nobody asked the address.
    pub standing: Option<ServerStanding>,
    /// Why, when it is not ready.
    pub reason: Option<String>,
    /// The address it published.
    pub url: Option<String>,
    /// Its process id.
    pub pid: Option<u32>,
    /// The version it serves.
    pub version: Option<String>,
    /// Every surface its index lists, with whether it is ready; empty when it did not answer.
    pub surfaces: Vec<(String, String, bool)>,
    /// Whether the answer came from this very process, which holds the lease.
    pub this_process: bool,
}

/// The peer board's tally, when somebody asked it.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, Observations, PeersObservation, Verdict};
/// let mut o = Observations::empty("demo", 0);
/// o.peers = Ok(PeersObservation { attached: 3, detached: 1, announced: 2 });
/// let p = derive(&o);
/// assert_eq!(p.check("integration.peers").unwrap().verdict, Verdict::Active);
/// assert!(p.check("integration.peers").unwrap().summary.starts_with("3 attached"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PeersObservation {
    /// Peers attached now.
    pub attached: usize,
    /// Peers gone whose claims the board still shows.
    pub detached: usize,
    /// Attached peers that announced what they are doing.
    pub announced: usize,
}

/// The evidence ledger, joined to the tree in front of you.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, LedgerObservation, Observations, Verdict};
/// let mut o = Observations::empty("demo", 0);
/// o.ledger = LedgerObservation::Unreadable("version 9".into());
/// assert_eq!(derive(&o).check("verification.tests").unwrap().verdict, Verdict::Failed);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum LedgerObservation {
    /// No ledger file.
    #[default]
    Absent,
    /// A ledger this executable cannot read; the reason.
    Unreadable(String),
    /// The ledger, joined to HEAD.
    Read {
        /// Recorded executions.
        total: usize,
        /// Of those, made at HEAD on a clean tree, whose test still hashes as recorded.
        current: usize,
        /// Of the current ones, those that did not pass.
        current_failing: usize,
        /// The commit of the newest execution.
        newest_commit: String,
        /// When the newest execution was recorded.
        newest_at: String,
    },
}

/// The newest commit of the deployment ref, and the source commit it says it was built from.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, DeploymentObservation, Observations, Verdict};
/// let mut o = Observations::empty("demo", 0);
/// o.deployment = DeploymentObservation::Published { commit: "d".into(), at: "t".into(), source: None };
/// // a deployment that does not name its source proves nothing about any commit
/// assert_eq!(derive(&o).check("verification.deployment").unwrap().verdict, Verdict::Unknown);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DeploymentObservation {
    /// The ref is not in this clone.
    #[default]
    NoRef,
    /// The newest deployment commit, and the source commit it names.
    Published {
        /// The deployment commit.
        commit: String,
        /// When it was committed.
        at: String,
        /// The source commit it was built from, when its message names one.
        source: Option<String>,
    },
}

/// Everything [`derive`] decides from. Plain data: a test builds one by hand.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, GitObservation, Observations, Verdict};
/// let mut o = Observations::empty("demo", 0);
/// o.git = Some(GitObservation { branch: Some("main".into()), head: Some("a".repeat(40)), clean: true, changed: 0 });
/// assert_eq!(derive(&o).check("repository.git").unwrap().verdict, Verdict::Active);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observations {
    /// The repository's name.
    pub repository: String,
    /// When they were taken, RFC 3339.
    pub generated_at: String,
    /// The same instant, seconds since the epoch.
    pub now: i64,
    /// How the environment was resolved.
    pub resolution: Resolution,
    /// Version control.
    pub git: Option<GitObservation>,
    /// The open episode.
    pub episode: Option<EpisodeObservation>,
    /// The task record.
    pub task: Option<TaskObservation>,
    /// The handover a resuming worker would get.
    pub handover: Option<HandoverObservation>,
    /// The policy.
    pub policy: PolicyObservation,
    /// The rule tally.
    pub rules: RulesObservation,
    /// Architecture decisions the index holds, when counted.
    pub adrs: Option<usize>,
    /// The decisions joined to the active task's scope, when a join was made.
    pub adr_relevance: AdrRelevanceObservation,
    /// The shared server.
    pub server: ServerObservation,
    /// The peer board; `None` with the reason when it was not asked.
    pub peers: Result<PeersObservation, String>,
    /// The evidence ledger.
    pub ledger: LedgerObservation,
    /// Provider projections against their templates.
    pub projections: Vec<(String, ProjectionState)>,
    /// The deployment ref.
    pub deployment: DeploymentObservation,
}

impl Observations {
    /// Observations of nothing at all: every reading absent.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::Observations;
    /// let o = Observations::empty("demo", 0);
    /// assert!(o.git.is_none() && o.episode.is_none());
    /// ```
    pub fn empty(repository: &str, now: i64) -> Self {
        Observations {
            repository: repository.into(),
            generated_at: "1970-01-01T00:00:00Z".into(),
            now,
            resolution: Resolution::Fast,
            git: None,
            episode: None,
            task: None,
            handover: None,
            policy: PolicyObservation::NotRead,
            rules: RulesObservation::Absent,
            adrs: None,
            adr_relevance: AdrRelevanceObservation::Absent,
            server: ServerObservation::default(),
            peers: Err("not asked".into()),
            ledger: LedgerObservation::Absent,
            projections: Vec::new(),
            deployment: DeploymentObservation::NoRef,
        }
    }
}

// ---------------------------------------------------------------- deriving

fn short(commit: &str) -> &str {
    &commit[..12.min(commit.len())]
}

/// The preflight these observations support. Reads nothing.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, DeploymentObservation, GitObservation, Observations, Verdict};
/// let head = "b".repeat(40);
/// let mut o = Observations::empty("demo", 0);
/// o.git = Some(GitObservation { branch: None, head: Some(head.clone()), clean: true, changed: 0 });
/// o.deployment = DeploymentObservation::Published { commit: "d".into(), at: "t".into(), source: Some(head) };
/// assert_eq!(derive(&o).check("verification.deployment").unwrap().verdict, Verdict::Verified);
/// ```
pub fn derive(o: &Observations) -> Preflight {
    let head = o.git.as_ref().and_then(|g| g.head.clone());
    let clean = o.git.as_ref().map(|g| g.clean);
    let sections = vec![
        Section {
            id: "repository".into(),
            title: "Repository".into(),
            checks: vec![git_check(o)],
        },
        Section {
            id: "session".into(),
            title: "Session".into(),
            checks: vec![
                episode_check(o),
                task_check(o),
                context_check(o, head.as_deref(), clean),
                handover_check(o),
            ],
        },
        Section {
            id: "governance".into(),
            title: "Governance".into(),
            checks: vec![
                policy_check(o),
                rules_check(o, head.as_deref()),
                adrs_check(o, head.as_deref()),
            ],
        },
        Section {
            id: "integration".into(),
            title: "Integration".into(),
            checks: vec![
                server_check(o),
                surface_check(o, "mcp", "integration.mcp", "MCP"),
                surface_check(o, "api", "integration.api", "API"),
                surface_check(o, "cockpit", "integration.cockpit", "Cockpit"),
                peers_check(o),
            ],
        },
        Section {
            id: "verification".into(),
            title: "Verification".into(),
            checks: vec![
                tests_check(o),
                coverage_check(),
                enforcement_check(o, head.as_deref(), clean),
                projections_check(o),
                docs_check(),
                deployment_check(o, head.as_deref()),
            ],
        },
    ];
    let mut attention: Vec<(u8, usize, String)> = sections
        .iter()
        .flat_map(|s| &s.checks)
        .enumerate()
        .filter_map(|(i, c)| c.verdict.urgency().map(|u| (u, i, c.id.clone())))
        .collect();
    attention.sort();
    Preflight {
        schema: Preflight::schema_id(),
        generated_at: o.generated_at.clone(),
        resolution: o.resolution,
        repository: o.repository.clone(),
        branch: o.git.as_ref().and_then(|g| g.branch.clone()),
        head,
        sections,
        attention: attention.into_iter().map(|(_, _, id)| id).collect(),
    }
}

fn git_check(o: &Observations) -> Check {
    match &o.git {
        Some(g) => Check::new(
            "repository.git",
            "git",
            Verdict::Active,
            format!(
                "{} · {}",
                g.branch.as_deref().unwrap_or("detached"),
                if g.clean {
                    "clean".to_string()
                } else {
                    format!("{} path(s) changed", g.changed)
                }
            ),
            vec![Evidence::new(
                "git status --porcelain=v2 --branch",
                format!("HEAD {}", g.head.as_deref().map(short).unwrap_or("unborn")),
            )],
        ),
        None => Check::new(
            "repository.git",
            "git",
            Verdict::Unavailable,
            "git could not be asked about this checkout",
            vec![],
        ),
    }
}

const SESSION_RECORD: &str = ".ai/local/state/session-current.yaml";
const TASK_RECORD: &str = ".ai/local/state/current.yaml";

fn episode_check(o: &Observations) -> Check {
    match &o.episode {
        Some(e) if e.this_checkout => Check::new(
            "session.episode",
            "episode",
            Verdict::Active,
            format!(
                "{} opened {}{}",
                e.id,
                e.started_at,
                if e.provider.is_empty() {
                    String::new()
                } else {
                    format!(" by {}", e.provider)
                }
            ),
            vec![Evidence::new(
                SESSION_RECORD,
                format!("session_id {}", e.id),
            )],
        )
        .next("majordomus session"),
        Some(e) => Check::new(
            "session.episode",
            "episode",
            Verdict::Degraded,
            format!(
                "the open episode {} belongs to another checkout; nothing about it is this work",
                e.id
            ),
            vec![Evidence::new(
                SESSION_RECORD,
                "worktree names another checkout",
            )],
        ),
        None => Check::new(
            "session.episode",
            "episode",
            Verdict::Unavailable,
            "no episode is open; a provider's start event opens one",
            vec![],
        )
        .next("majordomus session open"),
    }
}

/// Whether a task record is still in progress. `majordomus start` writes `outcome: active`
/// and `finish` or `handover` replaces it; a record written before that key existed has none.
fn in_progress(t: &TaskObservation) -> bool {
    t.outcome.is_empty() || t.outcome == "active"
}

fn task_check(o: &Observations) -> Check {
    match &o.task {
        Some(t) if in_progress(t) => Check::new(
            "session.task",
            "task",
            Verdict::Active,
            format!("{}: {}", t.id, t.task),
            vec![Evidence::new(
                TASK_RECORD,
                format!("started {}, in progress", t.started_at),
            )],
        )
        .next("majordomus check"),
        Some(t) => Check::new(
            "session.task",
            "task",
            Verdict::NotApplicable,
            format!(
                "no task is in progress; the last, {}, ended {}",
                t.id, t.outcome
            ),
            vec![Evidence::new(TASK_RECORD, format!("outcome {}", t.outcome))],
        )
        .next("majordomus start \"<task>\" --scope <paths>"),
        None => Check::new(
            "session.task",
            "task",
            Verdict::NotApplicable,
            "no task has been started in this checkout",
            vec![],
        )
        .next("majordomus start \"<task>\" --scope <paths>"),
    }
}

/// The briefing an episode was given is exact at the commit and tree it was written at. It
/// goes stale when HEAD moves, when the tree changes under it, or when a task starts after
/// it — the three things a briefing is about.
fn context_check(o: &Observations, head: Option<&str>, clean: Option<bool>) -> Check {
    let Some(e) = o.episode.as_ref().filter(|e| e.this_checkout) else {
        return Check::new(
            "session.context",
            "context",
            Verdict::Unknown,
            "no episode of this checkout, so no briefing to judge",
            vec![],
        );
    };
    let evidence = vec![Evidence::new(
        SESSION_RECORD,
        format!(
            "briefing written at {} ({})",
            short(&e.start_head),
            if e.start_working_tree.is_empty() {
                "tree unrecorded"
            } else {
                &e.start_working_tree
            }
        ),
    )];
    let Some(head) = head else {
        return Check::new(
            "session.context",
            "context",
            Verdict::Unknown,
            "git named no HEAD to compare the briefing with",
            evidence,
        );
    };
    let mut why: Vec<String> = Vec::new();
    if e.start_head.is_empty() {
        why.push("the episode recorded no commit".into());
    } else if e.start_head != head {
        why.push(format!(
            "HEAD moved from {} to {}",
            short(&e.start_head),
            short(head)
        ));
    }
    if e.start_working_tree == "clean" && clean == Some(false) {
        why.push("the tree changed since".into());
    }
    if let Some(t) = &o.task {
        if in_progress(t) && !t.started_at.is_empty() && t.started_at > e.started_at {
            why.push(format!("task {} started after it", t.id));
        }
    }
    if why.is_empty() {
        Check::new(
            "session.context",
            "context",
            Verdict::Fresh,
            format!("the briefing is exact at {}", short(head)),
            evidence,
        )
    } else {
        Check::new(
            "session.context",
            "context",
            Verdict::Stale,
            format!("the briefing is stale: {}", why.join("; ")),
            evidence,
        )
        .next("majordomus context")
    }
}

fn handover_check(o: &Observations) -> Check {
    let Some(h) = &o.handover else {
        return Check::new(
            "session.handover",
            "handover",
            Verdict::NotApplicable,
            "no handover is recorded for this branch",
            vec![],
        );
    };
    let verdict = match h.freshness {
        Freshness::Fresh | Freshness::Aging => Verdict::Fresh,
        Freshness::Stale | Freshness::Invalid => Verdict::Stale,
        Freshness::Unknown => Verdict::Unknown,
    };
    Check::new(
        "session.handover",
        "handover",
        verdict,
        format!(
            "{} and {}: {}",
            h.freshness.as_str(),
            h.divergence,
            h.reason
        ),
        vec![Evidence::new(
            &h.path,
            format!("session.freshness judged it {}", h.freshness.as_str()),
        )],
    )
    .next("majordomus handover --resolve")
}

fn policy_check(o: &Observations) -> Check {
    const POLICY: &str = ".ai/repo/policy.yaml";
    match &o.policy {
        PolicyObservation::Parsed => Check::new(
            "governance.policy",
            "policy",
            Verdict::Active,
            "the policy parses and every command reads it",
            vec![Evidence::new(POLICY, "parsed by LoadedPolicy")],
        ),
        PolicyObservation::Refused(reason) => Check::new(
            "governance.policy",
            "policy",
            Verdict::Failed,
            format!("the policy does not parse: {reason}"),
            vec![Evidence::new(POLICY, "refused")],
        )
        .next("majordomus doctor"),
        PolicyObservation::NotRead => Check::new(
            "governance.policy",
            "policy",
            Verdict::Unknown,
            "the policy was not read",
            vec![],
        ),
    }
}

fn tally_of(o: &Observations) -> Option<(&RulesTally, &'static str)> {
    match &o.rules {
        RulesObservation::Counted(t) => Some((t, "rules.report over the index")),
        RulesObservation::Cached(t) => Some((
            t,
            ".ai/local/state/environment/snapshot.json (rules.report, cached)",
        )),
        RulesObservation::Absent => None,
    }
}

fn rules_check(o: &Observations, head: Option<&str>) -> Check {
    let Some((t, source)) = tally_of(o) else {
        return Check::new(
            "governance.rules",
            "rules",
            Verdict::Unknown,
            "not counted: the rule corpus is counted from the index, which entry never builds",
            vec![],
        )
        .next("majordomus env preflight --full");
    };
    let summary = format!(
        "{} discovered ({} blocking; {} project, {} vendored) · {} run by a CI gate",
        t.rules,
        t.blocking,
        t.project,
        t.rules - t.project,
        t.gated
    );
    let evidence = vec![Evidence::new(
        source,
        format!(
            "counted at {}",
            t.head.as_deref().map(short).unwrap_or("an unknown commit")
        ),
    )];
    match (t.head.as_deref(), head) {
        (Some(counted), Some(now)) if counted == now => Check::new(
            "governance.rules",
            "rules",
            Verdict::Active,
            summary,
            evidence,
        ),
        _ => Check::new(
            "governance.rules",
            "rules",
            Verdict::Stale,
            format!("{summary}; counted at another commit"),
            evidence,
        )
        .next("majordomus env preflight --full"),
    }
}

/// The decisions: how many the index holds, and which of them the task in progress is joined
/// to by a declared relation — an ADR's `related` file or test meeting a path of the task's
/// scope. Nothing wider is claimed: no task, no join; a join for another task or scope says
/// nothing; a join at another commit is stale.
fn adrs_check(o: &Observations, head: Option<&str>) -> Check {
    const ID: &str = "governance.adrs";
    let Some(n) = o.adrs else {
        return Check::new(
            ID,
            "ADRs",
            Verdict::Unknown,
            "not counted: the index was not built and no cache holds a count",
            vec![],
        )
        .next("majordomus env status");
    };
    let indexed = Evidence::new("the index, kind adr", format!("{n} object(s)"));
    let Some(task) = o.task.as_ref().filter(|t| in_progress(t)) else {
        return Check::new(
            ID,
            "ADRs",
            Verdict::Active,
            format!("{n} indexed; no task is in progress, so none is claimed relevant"),
            vec![indexed],
        )
        .next("majordomus adr list");
    };
    let (joined, source) = match &o.adr_relevance {
        AdrRelevanceObservation::Joined(r) => (Some(r), "the adrs graph, joined to the task's scope"),
        AdrRelevanceObservation::Cached(r) => (
            Some(r),
            ".ai/local/state/environment/snapshot.json (the adrs graph joined to the task's scope, cached)",
        ),
        AdrRelevanceObservation::Absent => (None, ""),
    };
    let scope = normalised_scope(&task.scope);
    let Some(r) = joined.filter(|r| r.task == task.id && r.scope == scope) else {
        return Check::new(
            ID,
            "ADRs",
            Verdict::Unknown,
            format!(
                "{n} indexed; relevance to task {} not joined: the join reads the index, which entry never builds",
                task.id
            ),
            vec![indexed],
        )
        .next("majordomus env preflight --full");
    };
    let ids = r.adrs();
    let mut summary = format!("{n} indexed · {} relevant to task {}", ids.len(), task.id);
    if scope.is_empty() {
        summary.push_str(" (it declares no scope)");
    } else if !ids.is_empty() {
        // the summary is one line; every relation is in the evidence
        let _ = write!(summary, ": {}", ids[..ids.len().min(5)].join(", "));
        if ids.len() > 5 {
            let _ = write!(summary, " and {} more", ids.len() - 5);
        }
    }
    let mut evidence = vec![
        indexed,
        Evidence::new(
            source,
            format!(
                "joined at {} over scope {}",
                r.head.as_deref().map(short).unwrap_or("an unknown commit"),
                if scope.is_empty() {
                    "(none)".to_string()
                } else {
                    scope.join(", ")
                }
            ),
        ),
    ];
    evidence.extend(
        r.relations
            .iter()
            .map(|rel| Evidence::new("related (adrs graph, put_in_force)", rel.describe())),
    );
    match (r.head.as_deref(), head) {
        (Some(joined_at), Some(now)) if joined_at == now => {
            Check::new(ID, "ADRs", Verdict::Active, summary, evidence).next("majordomus adr list")
        }
        _ => Check::new(
            ID,
            "ADRs",
            Verdict::Stale,
            format!("{summary}; joined at another commit"),
            evidence,
        )
        .next("majordomus env preflight --full"),
    }
}

fn server_check(o: &Observations) -> Check {
    let s = &o.server;
    let lease = Evidence::new(
        ".ai/local/state/mcp/server.json",
        match (s.pid, &s.url) {
            (Some(pid), Some(url)) => format!("pid {pid} published {url}"),
            (Some(pid), None) => format!("pid {pid}, no address yet"),
            _ => "no lease".into(),
        },
    );
    let reason = s.reason.clone().unwrap_or_default();
    match s.standing {
        Some(ServerStanding::Ready) => {
            let mut evidence = vec![lease];
            evidence.push(if s.this_process {
                Evidence::new("this process", "holds the checkout's lease")
            } else {
                Evidence::new(
                    "GET /",
                    "answers as this checkout's leaseholder, at this executable's version",
                )
            });
            Check::new(
                "integration.server",
                "server",
                Verdict::Verified,
                format!(
                    "version {} serves this checkout at {}",
                    s.version.as_deref().unwrap_or("?"),
                    s.url.as_deref().unwrap_or("?")
                ),
                evidence,
            )
        }
        Some(ServerStanding::Outdated) => Check::new(
            "integration.server",
            "server",
            Verdict::Degraded,
            reason,
            vec![lease, Evidence::new("GET /", "answers, from other code")],
        )
        .next("majordomus serve stop && majordomus serve ensure"),
        Some(ServerStanding::Stale) => Check::new(
            "integration.server",
            "server",
            Verdict::Failed,
            reason,
            vec![lease],
        )
        .next("majordomus serve ensure"),
        Some(ServerStanding::Starting) => Check::new(
            "integration.server",
            "server",
            Verdict::Unknown,
            "a server is binding and has not published an address yet",
            vec![lease],
        )
        .next("majordomus serve status"),
        Some(ServerStanding::Absent) => Check::new(
            "integration.server",
            "server",
            Verdict::Unavailable,
            "no server holds this checkout's lease",
            vec![],
        )
        .next("majordomus serve ensure"),
        None => Check::new(
            "integration.server",
            "server",
            Verdict::Unknown,
            "a lease names a server, and this reading did not ask it",
            vec![],
        )
        .next("majordomus serve status"),
    }
}

fn surface_check(o: &Observations, surface: &str, id: &str, title: &str) -> Check {
    let s = &o.server;
    let listed = s.surfaces.iter().find(|(sid, _, _)| sid == surface);
    let at = |path: &str| {
        s.url
            .as_deref()
            .map(|u| super::services::join(u, path))
            .unwrap_or_else(|| path.to_string())
    };
    match (s.standing, listed) {
        (Some(ServerStanding::Ready), Some((_, path, true))) => Check::new(
            id,
            title,
            Verdict::Verified,
            format!("served at {}", at(path)),
            vec![Evidence::new(
                if s.this_process {
                    "this process"
                } else {
                    "GET /"
                },
                format!("surface {surface} ready"),
            )],
        ),
        (Some(ServerStanding::Outdated), Some((_, path, true))) => Check::new(
            id,
            title,
            Verdict::Degraded,
            format!(
                "served at {} by an outdated server: {}",
                at(path),
                s.reason.as_deref().unwrap_or("another version")
            ),
            vec![Evidence::new("GET /", format!("surface {surface} ready"))],
        ),
        (Some(ServerStanding::Ready | ServerStanding::Outdated), Some((_, _, false))) => {
            Check::new(
                id,
                title,
                Verdict::Failed,
                format!("the server answers and lists {surface} as not ready"),
                vec![Evidence::new(
                    "GET /",
                    format!("surface {surface} not ready"),
                )],
            )
            .next("majordomus serve status")
        }
        (Some(ServerStanding::Ready | ServerStanding::Outdated), None) => Check::new(
            id,
            title,
            Verdict::Failed,
            format!("the server answers and does not list {surface}"),
            vec![Evidence::new("GET /", format!("no surface {surface}"))],
        ),
        (Some(ServerStanding::Absent), _) => Check::new(
            id,
            title,
            Verdict::Unavailable,
            "nothing serves it: no server holds this checkout's lease",
            vec![],
        )
        .next("majordomus serve ensure"),
        (Some(ServerStanding::Stale), _) => Check::new(
            id,
            title,
            Verdict::Unavailable,
            "nothing serves it: the lease names a server that does not answer",
            vec![],
        )
        .next("majordomus serve ensure"),
        (Some(ServerStanding::Starting) | None, _) => Check::new(
            id,
            title,
            Verdict::Unknown,
            "the server behind it was not asked, or is still binding",
            vec![],
        ),
    }
}

fn peers_check(o: &Observations) -> Check {
    match &o.peers {
        Ok(p) => {
            let summary = format!(
                "{} attached ({} announced) · {} gone with claims still shown",
                p.attached, p.announced, p.detached
            );
            let evidence = vec![Evidence::new("GET /api/v1/peers", "the board answered")];
            if o.server.standing == Some(ServerStanding::Outdated) {
                Check::new(
                    "integration.peers",
                    "peers",
                    Verdict::Degraded,
                    format!("{summary}, on an outdated server's board"),
                    evidence,
                )
            } else {
                Check::new(
                    "integration.peers",
                    "peers",
                    Verdict::Active,
                    summary,
                    evidence,
                )
            }
        }
        Err(reason) => Check::new(
            "integration.peers",
            "peers",
            if o.server.standing == Some(ServerStanding::Absent) {
                Verdict::Unavailable
            } else {
                Verdict::Unknown
            },
            reason.clone(),
            vec![],
        )
        .next("majordomus env preflight"),
    }
}

fn tests_check(o: &Observations) -> Check {
    const LEDGER: &str = crate::evidence::ledger::LEDGER_PATH;
    match &o.ledger {
        LedgerObservation::Absent => Check::new(
            "verification.tests",
            "tests",
            Verdict::Unavailable,
            "no execution is recorded: the evidence ledger is absent",
            vec![],
        )
        .next("majordomus evidence record"),
        LedgerObservation::Unreadable(reason) => Check::new(
            "verification.tests",
            "tests",
            Verdict::Failed,
            format!("the evidence ledger cannot be read: {reason}"),
            vec![Evidence::new(LEDGER, "unreadable")],
        ),
        LedgerObservation::Read { total: 0, .. } => Check::new(
            "verification.tests",
            "tests",
            Verdict::Unavailable,
            "the evidence ledger records no execution",
            vec![Evidence::new(LEDGER, "0 executions")],
        )
        .next("majordomus evidence record"),
        LedgerObservation::Read {
            total,
            current,
            current_failing,
            newest_commit,
            newest_at,
        } => {
            let evidence = vec![Evidence::new(
                LEDGER,
                format!(
                    "{total} execution(s); newest at {} on {newest_at}",
                    short(newest_commit)
                ),
            )];
            if *current_failing > 0 {
                Check::new(
                    "verification.tests",
                    "tests",
                    Verdict::Failed,
                    format!(
                        "{current_failing} of the {current} run(s) recorded at HEAD did not pass"
                    ),
                    evidence,
                )
                .next("majordomus-cli evidence report")
            } else if current == total {
                Check::new(
                    "verification.tests",
                    "tests",
                    Verdict::Verified,
                    format!(
                        "all {total} recorded run(s) were made at HEAD on a clean tree and passed"
                    ),
                    evidence,
                )
            } else {
                Check::new(
                    "verification.tests",
                    "tests",
                    Verdict::Stale,
                    format!(
                        "{current} of {total} recorded run(s) measured this tree; the rest prove another commit, a changed tree or a changed test"
                    ),
                    evidence,
                )
                .next("majordomus-cli evidence report")
            }
        }
    }
}

fn coverage_check() -> Check {
    Check::new(
        "verification.coverage",
        "coverage",
        Verdict::Unavailable,
        "no coverage measurement is recorded in the repository: scripts/rust-coverage measures and gates it and leaves no record this can read",
        vec![],
    )
    .next("scripts/rust-coverage")
}

fn enforcement_check(o: &Observations, head: Option<&str>, clean: Option<bool>) -> Check {
    let Some((t, source)) = tally_of(o) else {
        return Check::new(
            "verification.enforcement",
            "enforcement",
            Verdict::Unknown,
            "not derived: the rule proofs are joined over the index, which entry never builds",
            vec![],
        )
        .next("majordomus env preflight --full");
    };
    let owed = t.rules - t.reviewed - t.unproven;
    let summary = format!(
        "{} of {owed} rule(s) with an executable proof are proven against the tree · {} gated with no recorded verdict · {} finding(s)",
        t.proven,
        t.states.get(RuleState::Gated.label()).copied().unwrap_or(0),
        t.findings
    );
    let evidence = vec![Evidence::new(
        source,
        format!(
            "states {}",
            t.states
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )];
    let at_head = t.head.is_some() && t.head.as_deref() == head;
    if t.failing > 0 {
        Check::new(
            "verification.enforcement",
            "enforcement",
            Verdict::Failed,
            format!("{} rule(s) failing · {summary}", t.failing),
            evidence,
        )
        .next("majordomus-cli rules report")
    } else if !at_head || (t.working_tree == "clean" && clean == Some(false)) {
        Check::new(
            "verification.enforcement",
            "enforcement",
            Verdict::Stale,
            format!("{summary}; derived at another commit or tree"),
            evidence,
        )
        .next("majordomus env preflight --full")
    } else if t.proven == owed && t.findings == 0 {
        Check::new(
            "verification.enforcement",
            "enforcement",
            Verdict::Verified,
            summary,
            evidence,
        )
    } else {
        Check::new(
            "verification.enforcement",
            "enforcement",
            Verdict::Degraded,
            summary,
            evidence,
        )
        .next("majordomus-cli rules report")
    }
}

fn projections_check(o: &Observations) -> Check {
    if o.projections.is_empty() {
        return Check::new(
            "verification.projections",
            "projections",
            Verdict::NotApplicable,
            "the policy declares no provider projection",
            vec![],
        );
    }
    let off: Vec<&str> = o
        .projections
        .iter()
        .filter(|(_, s)| matches!(s, ProjectionState::Stale | ProjectionState::Absent))
        .map(|(id, _)| id.as_str())
        .collect();
    let unknown = o
        .projections
        .iter()
        .filter(|(_, s)| *s == ProjectionState::Unknown)
        .count();
    let evidence = vec![Evidence::new(
        "policy projections[] rendered against their templates",
        format!("{} projection(s)", o.projections.len()),
    )];
    if !off.is_empty() {
        Check::new(
            "verification.projections",
            "projections",
            Verdict::Stale,
            format!("{} differ from their templates", off.join(", ")),
            evidence,
        )
        .next("majordomus generate")
    } else if unknown > 0 {
        Check::new(
            "verification.projections",
            "projections",
            Verdict::Unknown,
            format!(
                "{unknown} projection(s) could not be compared: the distribution was not found"
            ),
            vec![],
        )
    } else {
        Check::new(
            "verification.projections",
            "projections",
            Verdict::Verified,
            format!(
                "{} provider projection(s) match their templates",
                o.projections.len()
            ),
            evidence,
        )
    }
}

fn docs_check() -> Check {
    Check::new(
        "verification.docs",
        "generated docs",
        Verdict::Unknown,
        "no generation check is recorded for this tree, and entry does not run the generator",
        vec![],
    )
    .next("majordomus generate --check")
}

fn deployment_check(o: &Observations, head: Option<&str>) -> Check {
    match &o.deployment {
        DeploymentObservation::NoRef => Check::new(
            "verification.deployment",
            "deployment",
            Verdict::Unavailable,
            format!("no {PAGES_REF} in this clone: nothing is known about a deployment"),
            vec![],
        )
        .next("git fetch origin gh-pages"),
        DeploymentObservation::Published {
            commit,
            at,
            source: None,
        } => Check::new(
            "verification.deployment",
            "deployment",
            Verdict::Unknown,
            format!(
                "the newest deployment commit {} names no source commit",
                short(commit)
            ),
            vec![Evidence::new(
                PAGES_REF,
                format!("{} at {at}", short(commit)),
            )],
        ),
        DeploymentObservation::Published {
            commit,
            at,
            source: Some(source),
        } => {
            let evidence = vec![Evidence::new(
                PAGES_REF,
                format!(
                    "{} at {at}: source {} (as last fetched)",
                    short(commit),
                    short(source)
                ),
            )];
            if Some(source.as_str()) == head {
                Check::new(
                    "verification.deployment",
                    "deployment",
                    Verdict::Verified,
                    format!(
                        "the site was built from HEAD {} and published {at}",
                        short(source)
                    ),
                    evidence,
                )
                .next("scripts/pages verify")
            } else {
                Check::new(
                    "verification.deployment",
                    "deployment",
                    Verdict::Stale,
                    format!(
                        "deployed from {} at {at}; HEAD is {}",
                        short(source),
                        head.map(short).unwrap_or("unknown")
                    ),
                    evidence,
                )
                .next("git fetch origin gh-pages")
            }
        }
    }
}

// ---------------------------------------------------------------- observing

/// What a reading may cost: whether it may ask the server, ask the peer board, and use the
/// checkout's cache for answers that spawn git.
///
/// ```
/// use majordomus_cli::environment::preflight::Probe;
/// let entry = Probe::entry();
/// assert!(entry.server && !entry.peers, "entry asks the server and not the board");
/// assert!(!Probe::sealed().server);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Probe {
    /// Ask the published server's address.
    pub server: bool,
    /// Ask the peer board, which gathers every checkout's and costs tens of milliseconds.
    pub peers: bool,
    /// Read and write the checkout's cache for answers that cost git processes. Never for a
    /// served request: a request with a side effect on the repository would be a defect.
    pub cache: bool,
}

impl Probe {
    /// What entering a directory pays for: one loopback request, no board.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::Probe;
    /// assert!(Probe::entry().server);
    /// ```
    pub fn entry() -> Self {
        Probe {
            server: true,
            peers: false,
            cache: true,
        }
    }

    /// What a person asking for the preflight pays for.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::Probe;
    /// assert!(Probe::asked().peers);
    /// ```
    pub fn asked() -> Self {
        Probe {
            server: true,
            peers: true,
            cache: true,
        }
    }

    /// Nothing outside the process: what a test and a served request use.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::Probe;
    /// assert!(!Probe::sealed().peers);
    /// ```
    pub fn sealed() -> Self {
        Probe {
            server: false,
            peers: false,
            cache: false,
        }
    }
}

/// Read everything a preflight is decided from.
///
/// `environment` is the snapshot already resolved for this call (its version control, layer
/// counts and provider projections are used as they are, never re-read); `policy` is the
/// policy the caller read, or why it could not; `rules` is a tally counted in this process,
/// or `None` to take the one a full resolution cached.
///
/// ```
/// use majordomus_cli::environment::preflight::{derive, observe, Probe, Verdict};
/// use majordomus_cli::environment::{resolve, EnvironmentQuery, Inputs};
/// use majordomus_cli::synthetic::SyntheticRepository;
///
/// let repo = SyntheticRepository::small().unwrap();
/// let repository = majordomus_cli::Repository::open(repo.root()).unwrap();
/// let env = resolve(
///     &Inputs { repository: &repository, share: None, index: None, registry: None, policy: None },
///     &EnvironmentQuery::fast().sealed(),
/// );
/// let p = derive(&observe(repo.root(), &env, Err("not read".into()), None, Probe::sealed()));
/// // no lease: nothing serves this checkout, and that needs no probe to know
/// assert_eq!(p.check("integration.server").unwrap().verdict, Verdict::Unavailable);
/// ```
pub fn observe(
    root: &Path,
    environment: &RepositoryEnvironment,
    policy: Result<&LoadedPolicy, String>,
    rules: Option<RulesTally>,
    probe: Probe,
) -> Observations {
    let local = environment.repository.local_path.as_str();
    let state = root.join(local).join("state");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let git = environment.vcs.tree().map(|t| GitObservation {
        branch: t.branch.clone(),
        head: t.head.clone(),
        clean: t.clean,
        changed: t.dirty_files() + t.untracked,
    });
    let head = git.as_ref().and_then(|g| g.head.clone());
    let branch = git
        .as_ref()
        .and_then(|g| g.branch.clone())
        .unwrap_or_else(|| "DETACHED".into());

    let episode = read_scalars(&state.join("session-current.yaml")).and_then(|f| {
        let id = f.get("session_id")?.clone();
        let worktree = f.get("worktree").cloned().unwrap_or_default();
        Some(EpisodeObservation {
            id,
            started_at: f.get("started_at").cloned().unwrap_or_default(),
            provider: f.get("provider").cloned().unwrap_or_default(),
            start_head: f.get("start_head").cloned().unwrap_or_default(),
            start_working_tree: f.get("start_working_tree").cloned().unwrap_or_default(),
            this_checkout: worktree.is_empty() || Path::new(&worktree) == root,
        })
    });
    let task = continuity::read_task(&state.join("current.yaml")).map(|t| TaskObservation {
        id: t.id,
        task: t.task,
        outcome: t.outcome,
        started_at: t.started_at,
        scope: t.scope,
    });

    let (policy_observation, thresholds) = match &policy {
        Ok(p) => (
            PolicyObservation::Parsed,
            Thresholds {
                fresh_minutes: p.policy.session.freshness.fresh_minutes,
                stale_minutes: p.policy.session.freshness.stale_minutes,
            },
        ),
        Err(reason) if reason.is_empty() => (PolicyObservation::NotRead, Thresholds::default()),
        Err(reason) => (
            PolicyObservation::Refused(reason.clone()),
            Thresholds::default(),
        ),
    };
    let handover = continuity::resolve_record(
        root,
        &state.join("handovers"),
        &branch,
        head.as_deref(),
        thresholds,
        now,
    )
    .0
    .map(|r| HandoverObservation {
        path: r.path,
        freshness: r.freshness,
        reason: r.freshness_reason,
        divergence: r.divergence.as_str().to_string(),
    });

    let rules = match rules {
        Some(t) => RulesObservation::Counted(t),
        None => match cached_rules(root, local) {
            Some(t) => RulesObservation::Cached(t),
            None => RulesObservation::Absent,
        },
    };

    let server = observe_server(root, local, probe.server);
    let peers = if !probe.peers {
        Err("not asked on entry: the board gathers every checkout's and costs a round trip".into())
    } else {
        match (&server.standing, &server.url) {
            (Some(ServerStanding::Ready | ServerStanding::Outdated), Some(url)) => ask_peers(url),
            (Some(ServerStanding::Absent), _) => {
                Err("no server, so no board: the board lives in the shared server".into())
            }
            _ => Err("the server was not answering, so the board was not asked".into()),
        }
    };

    Observations {
        repository: environment.repository.name.clone(),
        generated_at: crate::peers::rfc3339(std::time::SystemTime::now()),
        now,
        resolution: environment.resolution,
        git,
        episode,
        task,
        handover,
        policy: policy_observation,
        rules,
        adrs: environment.layer.kind("adr"),
        adr_relevance: match cached_adr_relevance(root, local) {
            Some(r) => AdrRelevanceObservation::Cached(r),
            None => AdrRelevanceObservation::Absent,
        },
        server,
        peers,
        ledger: observe_ledger(root, local, environment.vcs.tree(), probe.cache),
        projections: environment
            .providers
            .iter()
            .map(|p| (p.id.clone(), p.state))
            .collect(),
        deployment: observe_deployment(root),
    }
}

/// The flat `key: value` lines of a small state record, with quotes removed. The records
/// this reads are written by the shell tool as scalars only.
fn read_scalars(path: &Path) -> Option<BTreeMap<String, String>> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut out = BTreeMap::new();
    for line in text.lines() {
        if line.starts_with(' ') || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let v = v.trim().trim_matches('"').trim_matches('\'');
            out.insert(k.trim().to_string(), v.to_string());
        }
    }
    Some(out)
}

fn observe_server(root: &Path, local: &str, probe: bool) -> ServerObservation {
    // A served request is its own evidence: this process holds the lease, so the surfaces it
    // is serving the request from are the checkout's, at this executable's version.
    if let Some(doc) = lease::held().filter(|d| d.root.as_path() == root) {
        return ServerObservation {
            standing: Some(ServerStanding::Ready),
            reason: None,
            url: doc.url.clone(),
            pid: Some(doc.pid),
            version: doc.version.clone(),
            surfaces: served_surfaces(),
            this_process: true,
        };
    }
    let path = lease::lease_file(root, local);
    let file = LeaseFile::read(&path);
    let doc = file.document().cloned();
    let mut observation = ServerObservation {
        url: doc.as_ref().and_then(|d| d.url.clone()),
        pid: doc.as_ref().map(|d| d.pid),
        version: doc.as_ref().and_then(|d| d.version.clone()),
        ..Default::default()
    };
    if matches!(file, LeaseFile::Absent) {
        observation.standing = Some(ServerStanding::Absent);
        return observation;
    }
    if !probe {
        return observation;
    }
    let reply = doc
        .as_ref()
        .and_then(|d| d.url.as_deref())
        .and_then(|url| lease::probe_reply(url, root, ANSWER_BUDGET));
    let (standing, reason) = standing_of(
        &file,
        lease::file_age(&path),
        |_| reply.is_some(),
        crate::VERSION,
    );
    observation.standing = Some(standing);
    observation.reason = reason;
    if let Some(body) = reply {
        observation.surfaces = body["surfaces"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| {
                        Some((
                            s["id"].as_str()?.to_string(),
                            s["path"].as_str().unwrap_or_default().to_string(),
                            s["ready"].as_bool().unwrap_or(false),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
    }
    observation
}

/// The surfaces this process serves, from the constants that decide their routes. Only a
/// process holding the lease asks, and every one of them is mounted by the router it runs.
fn served_surfaces() -> Vec<(String, String, bool)> {
    [
        ("mcp", crate::http::mcp::PATH),
        (
            "api",
            crate::capability::HttpExposure::PREFIX.trim_end_matches('/'),
        ),
        ("cockpit", crate::cockpit::PREFIX),
    ]
    .into_iter()
    .map(|(id, path)| (id.to_string(), path.to_string(), true))
    .collect()
}

fn ask_peers(url: &str) -> Result<PeersObservation, String> {
    let reply = crate::mcp::bridge::request(url, "GET", "/api/v1/peers", &[], None, BOARD_BUDGET)
        .map_err(|e| {
        format!(
            "the board did not answer within {} ms: {e}",
            BOARD_BUDGET.as_millis()
        )
    })?;
    if reply.status != 200 {
        return Err(format!("the board answered {}", reply.status));
    }
    let mut v: Value = serde_json::from_str(&reply.body).map_err(|e| e.to_string())?;
    let peers: Vec<crate::peers::Peer> =
        serde_json::from_value(v["peers"].take()).map_err(|e| e.to_string())?;
    Ok(PeersObservation::of(&peers))
}

impl PeersObservation {
    /// The tally of a board's peers: the same count whether the board was asked over the
    /// loopback address or read in the process that holds it.
    ///
    /// ```
    /// use majordomus_cli::environment::preflight::PeersObservation;
    /// use majordomus_cli::peers::{PeerBoard, Transport};
    /// let board = PeerBoard::new();
    /// let a = board.attach(Transport::Http);
    /// board.announce(&a, "writing docs", vec!["docs".into()]);
    /// board.attach(Transport::Stdio);
    /// let t = PeersObservation::of(&board.list());
    /// assert_eq!((t.attached, t.announced, t.detached), (2, 1, 0));
    /// ```
    pub fn of(peers: &[crate::peers::Peer]) -> Self {
        PeersObservation {
            attached: peers.iter().filter(|p| p.attached).count(),
            detached: peers.iter().filter(|p| !p.attached).count(),
            announced: peers
                .iter()
                .filter(|p| p.attached && !p.claims.is_empty())
                .count(),
        }
    }
}

fn observe_ledger(
    root: &Path,
    local: &str,
    tree: Option<&super::GitWorkingTree>,
    use_cache: bool,
) -> LedgerObservation {
    if !Ledger::present(root) {
        return LedgerObservation::Absent;
    }
    let ledger = match Ledger::load(root) {
        Ok(l) => l,
        Err(e) => return LedgerObservation::Unreadable(e.to_string()),
    };
    // What `evidence.report` calls proven, decided by its own comparison: the tree in front
    // of you is byte for byte the tree the run measured, the ledger's own row excluded —
    // recording dirties the tree and committing the record moves HEAD, so without that
    // exclusion no run could ever be current. One comparison per distinct recorded commit,
    // two git processes each — which is most of what entry would otherwise cost, so the
    // answers are cached under everything they depend on: HEAD, every path git reports
    // changed, and the ledger file.
    let fingerprint = tree.map(|t| {
        super::cache::fingerprint_of(
            root,
            &[crate::evidence::ledger::LEDGER_PATH.to_string()],
            &[t.head.as_deref().unwrap_or(""), &t.changed_paths.join("\n")],
        )
    });
    let mut cache = use_cache.then(|| Cache::load(root, local));
    let cached = match (&cache, &fingerprint) {
        (Some(c), Some(f)) => c
            .tiers
            .ledger_trees
            .as_ref()
            .and_then(|e| e.fresh(f, None))
            .cloned(),
        _ => None,
    };
    let unchanged: BTreeMap<String, bool> = match cached {
        Some(answers) => answers,
        None => {
            let mut answers: BTreeMap<String, bool> = BTreeMap::new();
            for e in &ledger.executions {
                answers.entry(e.commit.clone()).or_insert_with(|| {
                    crate::evidence::changed_since(root, &e.commit).is_some_and(|changed| {
                        changed
                            .iter()
                            .all(|p| p == crate::evidence::ledger::LEDGER_PATH)
                    })
                });
            }
            if let (Some(c), Some(f)) = (cache.as_mut(), fingerprint) {
                c.tiers.ledger_trees = Some(Cache::entry(f, answers.clone()));
                if let Err(e) = c.store(root, local) {
                    tracing::debug!(error = %e, "the ledger comparison could not be cached");
                }
            }
            answers
        }
    };
    let current: Vec<_> = ledger
        .executions
        .iter()
        .filter(|e| {
            e.working_tree == "clean"
                && unchanged.get(e.commit.as_str()) == Some(&true)
                && e.digest_matches(root) == Some(true)
        })
        .collect();
    let newest = ledger.executions.iter().max_by(|a, b| a.at.cmp(&b.at));
    LedgerObservation::Read {
        total: ledger.executions.len(),
        current: current.len(),
        current_failing: current.iter().filter(|e| !e.outcome.proves()).count(),
        newest_commit: newest.map(|e| e.commit.clone()).unwrap_or_default(),
        newest_at: newest.map(|e| e.at.clone()).unwrap_or_default(),
    }
}

fn observe_deployment(root: &Path) -> DeploymentObservation {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["log", "-1", "--format=%H%n%cI%n%B", PAGES_REF, "--"])
        .output();
    let Ok(out) = out else {
        return DeploymentObservation::NoRef;
    };
    if !out.status.success() {
        return DeploymentObservation::NoRef;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines();
    let commit = lines.next().unwrap_or_default().to_string();
    let at = lines.next().unwrap_or_default().to_string();
    let source = lines
        .filter_map(|l| l.trim().strip_prefix("source:"))
        .map(|s| s.trim().to_string())
        .find(|s| s.len() >= 7 && s.chars().all(|c| c.is_ascii_hexdigit()));
    DeploymentObservation::Published { commit, at, source }
}

/// Join the index's decisions to the task in progress in this checkout, over the `adrs`
/// graph. `None` when no task is in progress: relevance is to a task, and a branch name or a
/// recent commit is not one.
///
/// ```
/// use majordomus_cli::environment::preflight::join_adrs;
/// use majordomus_cli::synthetic::SyntheticRepository;
/// let repo = SyntheticRepository::small().unwrap();
/// let registry = majordomus_cli::capability::CapabilityRegistry::builder().build().unwrap();
/// // no task was started in the fixture, so nothing is joined and nothing is guessed
/// assert!(join_adrs(repo.root(), ".ai/local", &registry, &repo.index().unwrap(), None).is_none());
/// ```
pub fn join_adrs(
    root: &Path,
    local: &str,
    registry: &crate::capability::CapabilityRegistry,
    index: &crate::index::Index,
    head: Option<&str>,
) -> Option<AdrRelevance> {
    let task = continuity::read_task(&root.join(local).join("state").join("current.yaml"))?;
    if !(task.outcome.is_empty() || task.outcome == "active") {
        return None;
    }
    let graph = crate::graph::derive("adrs", registry, index)?;
    Some(AdrRelevance::join(&graph, &task.id, &task.scope, head))
}

/// The join of decisions to a task a full preflight left for the entry path, when there is
/// one. It carries the task, scope and commit it was made for, and the preflight trusts it
/// for nothing else.
///
/// ```
/// use majordomus_cli::environment::preflight::cached_adr_relevance;
/// let dir = tempfile::tempdir().unwrap();
/// assert!(cached_adr_relevance(dir.path(), ".ai/local").is_none(), "a miss is not an error");
/// ```
pub fn cached_adr_relevance(root: &Path, local: &str) -> Option<AdrRelevance> {
    Cache::load(root, local)
        .tiers
        .adr_relevance
        .map(|entry| entry.value)
}

/// Leave a join of decisions to a task for the entry path. Never fatal, like
/// [`store_rules`]: an unwritable checkout gets a preflight that says nothing was joined.
///
/// ```
/// use majordomus_cli::environment::preflight::{cached_adr_relevance, store_adr_relevance, AdrRelevance};
/// let dir = tempfile::tempdir().unwrap();
/// store_adr_relevance(dir.path(), ".ai/local", &AdrRelevance { task: "t-1".into(), ..Default::default() });
/// assert_eq!(cached_adr_relevance(dir.path(), ".ai/local").unwrap().task, "t-1");
/// ```
pub fn store_adr_relevance(root: &Path, local: &str, relevance: &AdrRelevance) {
    let mut cache = Cache::load(root, local);
    cache.tiers.adr_relevance = Some(Entry {
        fingerprint: ADR_RELEVANCE_TIER.into(),
        written_at: super::cache::now_seconds(),
        value: relevance.clone(),
    });
    if let Err(e) = cache.store(root, local) {
        tracing::debug!(error = %e, "the join of decisions to the task could not be cached");
    }
}

/// The rule tally a full resolution left for the entry path, when there is one.
///
/// ```
/// use majordomus_cli::environment::preflight::cached_rules;
/// let dir = tempfile::tempdir().unwrap();
/// assert!(cached_rules(dir.path(), ".ai/local").is_none(), "a miss is not an error");
/// ```
pub fn cached_rules(root: &Path, local: &str) -> Option<RulesTally> {
    Cache::load(root, local)
        .tiers
        .rules
        .map(|entry| entry.value)
}

/// Leave a rule tally for the entry path. Never fatal: a checkout that cannot be written
/// still gets a preflight, just one that says the rules were not counted.
///
/// ```
/// use majordomus_cli::environment::preflight::{cached_rules, store_rules, RulesTally};
/// let dir = tempfile::tempdir().unwrap();
/// store_rules(dir.path(), ".ai/local", &RulesTally { rules: 3, ..Default::default() });
/// assert_eq!(cached_rules(dir.path(), ".ai/local").unwrap().rules, 3);
/// ```
pub fn store_rules(root: &Path, local: &str, tally: &RulesTally) {
    let mut cache = Cache::load(root, local);
    cache.tiers.rules = Some(Entry {
        fingerprint: RULES_TIER.into(),
        written_at: super::cache::now_seconds(),
        value: tally.clone(),
    });
    if let Err(e) = cache.store(root, local) {
        tracing::debug!(error = %e, "the rule tally could not be cached");
    }
}

// ---------------------------------------------------------------- rendering

/// The mark a terminal draws beside a verdict. A success mark only for a verdict that
/// proves something, which is the one guarantee a person reading a banner needs.
///
/// ```
/// use majordomus_cli::environment::preflight::{mark, Verdict};
/// assert_eq!(mark(Verdict::Verified, true), "✓");
/// assert_eq!(mark(Verdict::Stale, true), "◐");
/// assert_eq!(mark(Verdict::Failed, false), "x");
/// ```
pub fn mark(verdict: Verdict, unicode: bool) -> &'static str {
    match (verdict, unicode) {
        (v, true) if v.proves() => "✓",
        (v, false) if v.proves() => "ok",
        (Verdict::Stale | Verdict::Degraded, true) => "◐",
        (Verdict::Stale | Verdict::Degraded, false) => "~",
        (Verdict::Failed, true) => "✗",
        (Verdict::Failed, false) => "x",
        (Verdict::Unavailable, true) => "○",
        (Verdict::Unavailable, false) => "o",
        (Verdict::NotApplicable, _) => "-",
        (_, _) => "?",
    }
}

/// The short form an entry prints: who and where, one line of the claims that matter most,
/// and the most urgent thing to look at.
///
/// ```
/// use majordomus_cli::environment::preflight::{compact, derive, Observations};
/// let text = compact(&derive(&Observations::empty("demo", 0)), true);
/// assert!(text.starts_with("◆ demo"));
/// assert!(text.contains("server ?"), "{text}");
/// assert!(!text.contains('✓'), "nothing observed, nothing marked proven: {text}");
/// ```
pub fn compact(p: &Preflight, unicode: bool) -> String {
    let bullet = if unicode { "·" } else { "-" };
    let episode = p
        .check("session.episode")
        .filter(|c| c.verdict.proves())
        .and_then(|c| c.evidence.first())
        .and_then(|e| e.observed.strip_prefix("session_id "))
        .map(|id| format!("episode {id}"))
        .unwrap_or_else(|| "no episode".into());
    let mut out = format!(
        "{} {} {bullet} {} {bullet} {episode}\n ",
        if unicode { "◆" } else { ">" },
        p.repository,
        p.branch.as_deref().unwrap_or("detached"),
    );
    let shown = [
        ("governance.rules", "rules"),
        ("session.context", "context"),
        ("integration.server", "server"),
        ("integration.mcp", "mcp"),
        ("integration.cockpit", "cockpit"),
        ("verification.tests", "tests"),
        ("verification.enforcement", "enforced"),
        ("verification.deployment", "deploy"),
    ];
    let parts: Vec<String> = shown
        .iter()
        .filter_map(|(id, label)| {
            p.check(id)
                .map(|c| format!("{label} {}", mark(c.verdict, unicode)))
        })
        .collect();
    let _ = write!(out, " {}", parts.join(&format!(" {bullet} ")));
    if let Some(first) = p.attention.first().and_then(|id| p.check(id)) {
        let _ = write!(
            out,
            "\n  {} {} {}: {}",
            mark(first.verdict, unicode),
            first.title,
            first.verdict.as_str(),
            first.summary
        );
        if p.attention.len() > 1 {
            let _ = write!(
                out,
                " (+{} more: majordomus env preflight)",
                p.attention.len() - 1
            );
        }
    }
    out.push('\n');
    out
}

/// The whole preflight, a section per heading and a line per claim, with its evidence.
///
/// ```
/// use majordomus_cli::environment::preflight::{full, derive, Observations};
/// let text = full(&derive(&Observations::empty("demo", 0)), true);
/// assert!(text.starts_with("MAJORDOMUS PREFLIGHT"));
/// assert!(text.contains("Verification"));
/// ```
pub fn full(p: &Preflight, unicode: bool) -> String {
    let mut out = format!(
        "MAJORDOMUS PREFLIGHT  {} {} {}\n",
        p.repository,
        p.branch.as_deref().unwrap_or("detached"),
        p.head.as_deref().map(short).unwrap_or("")
    );
    for section in &p.sections {
        let _ = write!(out, "\n{}\n", section.title);
        for c in &section.checks {
            let _ = writeln!(
                out,
                "  {:<13} {} {:<14} {}",
                c.title,
                mark(c.verdict, unicode),
                c.verdict.as_str(),
                c.summary
            );
            for e in &c.evidence {
                let _ = writeln!(
                    out,
                    "  {:<13}   {:<14} {}: {}",
                    "", "", e.source, e.observed
                );
            }
            if let Some(next) = &c.next {
                if c.verdict.urgency().is_some() {
                    let _ = writeln!(out, "  {:<13}   {:<14} next: {next}", "", "");
                }
            }
        }
    }
    if p.attention.is_empty() {
        out.push_str("\nNothing needs a look.\n");
    } else {
        let _ = writeln!(out, "\nNeeds a look: {}", p.attention.join(", "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head() -> String {
        "c".repeat(40)
    }

    fn at_head() -> Observations {
        let mut o = Observations::empty("demo", 0);
        o.git = Some(GitObservation {
            branch: Some("master".into()),
            head: Some(head()),
            clean: true,
            changed: 0,
        });
        o
    }

    #[test]
    fn every_check_that_proves_something_names_its_evidence() {
        let mut o = at_head();
        o.policy = PolicyObservation::Parsed;
        o.adrs = Some(3);
        o.rules = RulesObservation::Counted(RulesTally {
            head: Some(head()),
            working_tree: "clean".into(),
            rules: 2,
            proven: 2,
            ..Default::default()
        });
        for c in derive(&o).sections.iter().flat_map(|s| &s.checks) {
            if c.verdict.proves() {
                assert!(!c.evidence.is_empty(), "{} proves with no evidence", c.id);
            }
        }
    }

    #[test]
    fn attention_is_ordered_by_urgency() {
        let mut o = at_head();
        o.policy = PolicyObservation::Refused("bad".into());
        let p = derive(&o);
        assert_eq!(
            p.attention.first().map(String::as_str),
            Some("governance.policy")
        );
    }

    #[test]
    fn a_handover_judged_stale_is_stale_whatever_its_divergence() {
        let mut o = at_head();
        o.handover = Some(HandoverObservation {
            path: "h.md".into(),
            freshness: Freshness::Stale,
            reason: "10d old".into(),
            divergence: "advanced".into(),
        });
        assert_eq!(
            derive(&o).check("session.handover").unwrap().verdict,
            Verdict::Stale
        );
    }

    #[test]
    fn the_scalar_reader_takes_top_level_keys_only() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("s.yaml");
        std::fs::write(
            &f,
            "session_id: s-1\n# c\nnested:\n  inner: x\nowner: \"k\"\n",
        )
        .unwrap();
        let m = read_scalars(&f).unwrap();
        assert_eq!(m["session_id"], "s-1");
        assert_eq!(m["owner"], "k");
        assert!(!m.contains_key("inner"));
    }
}
