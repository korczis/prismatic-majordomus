//! The one path that decides a disposition. Pure: observations in, an [`PullRequestAssessment`] out.
//!
//! The order of the questions is the policy ([`IntegrationGate::ALL`]). Every question is
//! asked and every answer is reported, but the first that fails decides: a pull request that
//! is a draft is a draft whatever its checks say, and says what its checks say too; and
//! `Ready` is reached only when every gate passed. Nothing upstream of this file decides
//! eligibility and nothing downstream re-decides it.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::model::{
    ChangeShape, CheckKind, CheckRunState, DependencyCertainty, DependencyState, EvaluatedAgainst,
    EvidenceKind, EvidenceSource, GateResult, IntegrationEvidence, IntegrationGate,
    IntegrationRisk, OverlapKind, PathOverlap, PullRequestAssessment, PullRequestDependency,
    PullRequestDisposition, PullRequestObservation, PullRequestReview, ReasonCode,
    RelationToMaster, RequiredCheck, RequiredCheckState, ReviewPolicy,
};

/// What the repository requires before a merge, as observed from the forge and the
/// repository — never a list kept here beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationPolicy {
    /// The branch integrated into.
    pub base: String,
    /// The checks the base requires, from its branch protection and rulesets, each with the
    /// app bound to it. `None` when they could not be read: every required-check verdict is
    /// then `unknown`, never `passed`. An empty set is unknown too (owner decision D5): a
    /// base that requires nothing proves nothing about a head, so nothing merges onto it.
    pub required_checks: Option<Vec<RequiredCheck>>,
    /// What the base requires of reviews; `None` when unread.
    pub review_policy: Option<ReviewPolicy>,
    /// Contexts whose skip counts as a pass. Empty unless a repository says otherwise: a
    /// skipped required check is otherwise `missing`.
    #[serde(default)]
    pub skipped_permitted: Vec<String>,
    /// The labels that have an effect, each with its effect: [`LABEL_POLICY`], copied.
    pub labels: Vec<LabelPolicy>,
    /// The merge method the executor asks the forge for: `merge` when the repository allows a
    /// merge commit, and `None` when it does not. There is no other: the derived-file driver
    /// resolves merges, and a squash or rebase would replay commits it never saw, so with
    /// `None` every pull request the relation to master does not already decide is held
    /// (`merge_commit_not_allowed`), never merged otherwise.
    pub merge_method: Option<String>,
    /// Whether the base requires a branch to be up to date before it merges (the
    /// protection's `required_status_checks.strict`, or a ruleset's): the forge-side half of
    /// the guard against a merge onto a master nobody tested with the change. The executor's
    /// half is its parent check after every merge. `Some(false)` is the owner's to change
    /// (decision D8) and decides no disposition; `None` when unread.
    #[serde(default)]
    pub up_to_date_required: Option<bool>,
}

/// What a label does to a pull request that carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LabelEffect {
    /// It holds the pull request whatever else is true: `blocked`, with `label:NAME`.
    Hold,
    /// A person marked the work obsolete (owner decision D3): `obsolete`, with
    /// `label_obsolete:NAME`, listed by cleanup for a person and never closed automatically.
    Obsolete,
}

/// One label with an effect. The forge's label names are compared with `name`
/// case-insensitively.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LabelPolicy {
    /// The label, lower-case.
    pub name: std::borrow::Cow<'static, str>,
    /// What it does.
    pub effect: LabelEffect,
}

impl LabelPolicy {
    const fn hold(name: &'static str) -> Self {
        LabelPolicy {
            name: std::borrow::Cow::Borrowed(name),
            effect: LabelEffect::Hold,
        }
    }

    const fn obsolete(name: &'static str) -> Self {
        LabelPolicy {
            name: std::borrow::Cow::Borrowed(name),
            effect: LabelEffect::Obsolete,
        }
    }

    /// Whether `label`, as the forge spells it, is this one.
    pub fn names(&self, label: &str) -> bool {
        self.name.eq_ignore_ascii_case(label)
    }
}

/// The label policy: every label that has an effect, and the effect. The one table — the
/// policy ([`IntegrationPolicy::labels`]) copies it and the classifier reads that copy; no
/// other list of labels exists. There is no label that opts out of the executor's refresh
/// (owner decision D11): a label holds a pull request, or marks it obsolete (D3), and
/// nothing else.
pub const LABEL_POLICY: &[LabelPolicy] = &[
    LabelPolicy::hold("do-not-merge"),
    LabelPolicy::hold("do not merge"),
    LabelPolicy::hold("blocked"),
    LabelPolicy::hold("hold"),
    LabelPolicy::hold("on-hold"),
    LabelPolicy::hold("wip"),
    LabelPolicy::hold("manual-merge"),
    LabelPolicy::obsolete("obsolete"),
];

/// Path prefixes whose change makes a pull request high-risk to integrate.
const HIGH_RISK: &[&str] = &[
    ".github/",
    ".githooks/",
    "scripts/ci/",
    "scripts/derive",
    "scripts/merge-derived",
    ".ai/repo/rules/",
    ".ai/repo/policy.yaml",
    ".ai/repo/ci/",
    ".ai/manifest.yaml",
    "share/schemas/",
    RELEASES,
    "apps/majordomus-cli/Cargo.toml",
    "Cargo.lock",
    "apps/majordomus-cli/Cargo.lock",
    "SECURITY.md",
];

/// Where the release records live: a pull request changing one changes a release, and two
/// that do are a release overlap whatever files they name.
pub const RELEASES: &str = ".ai/repo/releases/";

/// Path prefixes of executable code: medium risk.
const CODE: &[&str] = &["apps/", "bin/", "lib/", "scripts/"];

/// More authored paths than this is a large change.
const LARGE: usize = 40;

/// The state of each required check on one head, in the order the base requires them.
///
/// For each context, only the reports that may stand for it are read: when the base binds
/// the context to an app, a status context or another app's check run of that name is not
/// it. Of those, a report still running makes the check pending; otherwise the newest
/// completed report is the verdict, so a failure followed by a passing re-run has passed.
/// A skip is a pass only for a context in `skipped_permitted`; otherwise it is `missing`.
pub fn required_check_states(
    checks: &[super::model::CheckObservation],
    required: &[RequiredCheck],
    skipped_permitted: &[String],
) -> Vec<(String, RequiredCheckState)> {
    required
        .iter()
        .map(|req| {
            let reports: Vec<&super::model::CheckObservation> = checks
                .iter()
                .filter(|c| c.name == req.context)
                .filter(|c| match req.app_id {
                    None => true,
                    // bound to an app: a check run from it, or one whose app is unreported
                    Some(app) => c.kind == CheckKind::CheckRun && c.app_id.is_none_or(|a| a == app),
                })
                .collect();
            let state = if reports.is_empty() {
                RequiredCheckState::Missing
            } else if reports.iter().any(|c| c.state == CheckRunState::Pending) {
                RequiredCheckState::Pending
            } else {
                let newest = reports
                    .iter()
                    .filter(|c| !c.completed_at.is_empty())
                    .max_by(|a, b| a.completed_at.cmp(&b.completed_at));
                // without a time to order them by, the worst report stands
                let states: Vec<CheckRunState> = match newest {
                    Some(c) => vec![c.state],
                    None => reports.iter().map(|c| c.state).collect(),
                };
                if states.contains(&CheckRunState::Failed) {
                    RequiredCheckState::Failed
                } else if states.iter().all(|s| *s == CheckRunState::Passed) {
                    RequiredCheckState::Passed
                } else if skipped_permitted.iter().any(|p| p == &req.context) {
                    RequiredCheckState::Skipped
                } else {
                    // skipped or neutral is not a pass of a required check
                    RequiredCheckState::Missing
                }
            };
            (req.context.clone(), state)
        })
        .collect()
}

/// The required-check verdict for one head: the worst of [`required_check_states`].
/// Unread requirements are `unknown`, and so is an empty set (owner decision D5): a base
/// that requires no check has proved nothing about this head. The module is private, so
/// the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::{CheckObservation, CheckRunState, RequiredCheck, RequiredCheckState};
/// use majordomus_cli::integration::classify::required_checks;
/// let ci = |state| vec![CheckObservation { name: "ci".into(), state, ..Default::default() }];
/// let req: Vec<RequiredCheck> = vec!["ci".into()];
/// assert_eq!(required_checks(&ci(CheckRunState::Passed), Some(&req), &[]), RequiredCheckState::Passed);
/// // a green check that is not the required one proves nothing
/// let other = vec![CheckObservation { name: "suite".into(), state: CheckRunState::Passed, ..Default::default() }];
/// assert_eq!(required_checks(&other, Some(&req), &[]), RequiredCheckState::Missing);
/// // unread protection is never a pass, and neither is a base that requires nothing
/// assert_eq!(required_checks(&ci(CheckRunState::Passed), None, &[]), RequiredCheckState::Unknown);
/// assert_eq!(required_checks(&ci(CheckRunState::Passed), Some(&[]), &[]), RequiredCheckState::Unknown);
/// ```
pub fn required_checks(
    checks: &[super::model::CheckObservation],
    required: Option<&[RequiredCheck]>,
    skipped_permitted: &[String],
) -> RequiredCheckState {
    let Some(required) = required else {
        return RequiredCheckState::Unknown;
    };
    if required.is_empty() {
        return RequiredCheckState::Unknown;
    }
    required_check_states(checks, required, skipped_permitted)
        .into_iter()
        .fold(RequiredCheckState::Passed, |v, (_, s)| worse(v, s))
}

fn worse(a: RequiredCheckState, b: RequiredCheckState) -> RequiredCheckState {
    let rank = |r| match r {
        RequiredCheckState::Passed => 0,
        RequiredCheckState::Skipped => 1,
        RequiredCheckState::Pending => 2,
        RequiredCheckState::Missing => 3,
        RequiredCheckState::Unknown => 4,
        RequiredCheckState::Failed => 5,
    };
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

/// The review state of one head under the base's review policy.
///
/// A reviewer asking for changes holds it whatever else is true, and the forge's own
/// `REVIEW_REQUIRED` is never "not required": a rule this policy does not list can require
/// a review. An approval counts only on the commit it was given on. When the policy
/// requires approvals, the head's are counted against its number; approvals of another
/// commit that would have made it are `stale`, whatever the forge's own decision says
/// (with dismissal off, the forge keeps counting them); and with enough approvals on the
/// head, a code-owner requirement the forge still reports unmet is `code_owners_pending`.
/// When the policy could not be read, the forge's decision is believed only where a review
/// on the head stands behind it; anything else is `unknown`, never a pass.
pub fn review_state(
    pr: &PullRequestObservation,
    policy: Option<&ReviewPolicy>,
) -> PullRequestReview {
    let decision = pr.review_decision.as_str();
    if decision == "CHANGES_REQUESTED" {
        return PullRequestReview::ChangesRequested;
    }
    let approved = |r: &&super::model::ReviewObservation| r.state == "APPROVED";
    let on_head = pr
        .latest_reviews
        .iter()
        .filter(approved)
        .filter(|r| r.commit == pr.head_sha)
        .count() as u64;
    let elsewhere = pr
        .latest_reviews
        .iter()
        .filter(approved)
        .filter(|r| !r.commit.is_empty() && r.commit != pr.head_sha)
        .count() as u64;
    let needed = policy.map(|p| {
        if p.approvals == 0 && !p.code_owners {
            0
        } else {
            p.approvals.max(1)
        }
    });
    match needed {
        None => match decision {
            "REVIEW_REQUIRED" => PullRequestReview::Pending,
            "APPROVED" if on_head > 0 => PullRequestReview::Approved,
            "APPROVED" if elsewhere > 0 => PullRequestReview::Stale,
            _ => PullRequestReview::Unknown,
        },
        Some(0) => match decision {
            "APPROVED" => PullRequestReview::Approved,
            "REVIEW_REQUIRED" => PullRequestReview::Pending,
            _ => PullRequestReview::NotRequired,
        },
        Some(n) if on_head >= n => {
            if decision == "REVIEW_REQUIRED" {
                if policy.is_some_and(|p| p.code_owners) {
                    PullRequestReview::CodeOwnersPending
                } else {
                    PullRequestReview::Pending
                }
            } else {
                PullRequestReview::Approved
            }
        }
        Some(n) if elsewhere > 0 && on_head + elsewhere >= n => PullRequestReview::Stale,
        Some(_) => PullRequestReview::Pending,
    }
}

/// The markers that declare a dependency, lower-case. Each must open its line.
pub const DEPENDENCY_MARKERS: &[&str] = &["depends on", "stacked on", "requires", "land after"];

/// Dependencies declared in a body: a line that opens with `Depends on #N`, `Stacked on #N`,
/// `Requires #N` or `Land after #N`, case-insensitively ([`marked_numbers`]). Prose that
/// merely mentions a number, or uses a marker's words mid-sentence, is not a declaration.
///
/// ```text
/// use crate::integration::declared_dependencies;
/// assert_eq!(declared_dependencies("Stacked on #601.\nSee #12 for context."), vec![601]);
/// assert!(declared_dependencies("a regression introduced after #540").is_empty());
/// ```text
pub fn declared_dependencies(body: &str) -> Vec<u64> {
    marked_numbers(body, DEPENDENCY_MARKERS)
}

/// The marker by which a body declares the pull request that replaces it, lower-case.
pub const SUPERSEDED_BY_MARKERS: &[&str] = &["superseded by"];

/// The marker by which a body declares the pull requests it replaces, lower-case.
pub const SUPERSEDES_MARKERS: &[&str] = &["supersedes"];

/// What one body declares about replacement, read by [`marked_numbers`] like a dependency.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Supersessions {
    /// `Superseded by #N`: the pull requests that replace this one.
    pub superseded_by: Vec<u64>,
    /// `Supersedes #N`: the pull requests this one replaces.
    pub supersedes: Vec<u64>,
}

/// The supersessions declared in a body: a line that opens with `Superseded by #N` names this
/// one's successor, and a line that opens with `Supersedes #N` names the pull requests this
/// one replaces, from the other side. Line-anchored, as dependencies are: the words
/// mid-sentence are prose.
///
/// ```text
/// use crate::integration::classify::declared_supersessions;
/// let s = declared_supersessions("Supersedes #12 and #14.\nsuperseded by #20 in spirit");
/// assert_eq!((s.supersedes, s.superseded_by), (vec![12, 14], vec![20]));
/// assert_eq!(declared_supersessions("this supersedes #3").supersedes, Vec::<u64>::new());
/// ```text
pub fn declared_supersessions(body: &str) -> Supersessions {
    Supersessions {
        superseded_by: marked_numbers(body, SUPERSEDED_BY_MARKERS),
        supersedes: marked_numbers(body, SUPERSEDES_MARKERS),
    }
}

/// The pull-request numbers a body declares under any of `markers`, in ascending order.
///
/// A declaration is line-anchored: the line opens — after leading space, at most one
/// bullet (`-`, `*`, `+` or `1.`), any quote marks (`>`) and emphasis (`*`, `_`) — with a
/// marker that ends at a word boundary, followed by one or more `#N`, separated by commas,
/// `and` or `&`. A marker's words anywhere else on a line are prose. The one parser for
/// every declaration a body can make, so that dependencies and any later kind of
/// declaration agree on what counts as one.
pub fn marked_numbers(body: &str, markers: &[&str]) -> Vec<u64> {
    let mut out = BTreeSet::new();
    for line in body.lines() {
        let lower = line.to_ascii_lowercase();
        let opening = line_opening(&lower);
        for marker in markers {
            let Some(rest) = opening.strip_prefix(marker) else {
                continue;
            };
            // "requirements", "dependson": another word, not the marker — the trim below
            // removes no letter, so `numbers` finds no `#` at the start of one
            out.extend(numbers(
                rest.trim_start_matches([':', '*', '_', '`', '(', ' ']),
            ));
        }
    }
    out.into_iter().collect()
}

/// A line without what may precede a declaration: space, quote marks, one list bullet and
/// emphasis.
fn line_opening(line: &str) -> &str {
    let mut t = line.trim_start().trim_start_matches(['>', ' ']);
    let ordered = t.trim_start_matches(|c: char| c.is_ascii_digit());
    if ordered.len() < t.len() {
        if let Some(rest) = ordered.strip_prefix(". ") {
            t = rest;
        }
    } else if let Some(rest) = ["- ", "* ", "+ "].iter().find_map(|b| t.strip_prefix(b)) {
        t = rest;
    }
    t.trim_start().trim_start_matches(['*', '_'])
}

/// The `#N` list at the start of `t`: "#644 and #645", "#14, #15 & #16".
fn numbers(mut t: &str) -> Vec<u64> {
    let mut out = Vec::new();
    while let Some(stripped) = t.strip_prefix('#') {
        let digits: String = stripped
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let Ok(n) = digits.parse::<u64>() else { break };
        out.push(n);
        t = stripped[digits.len()..]
            .trim_start_matches([',', ')', '`', '*', '_', '.'])
            .trim_start();
        t = ["and ", "& "]
            .iter()
            .find_map(|sep| t.strip_prefix(sep))
            .unwrap_or(t)
            .trim_start();
    }
    out
}

/// The planning risk of a change, with its factors. `known` is false when git could not
/// say what the head is to master: its paths are unknown, and a change nobody has read is
/// high-risk (`paths_unknown`), never "documentation only". A head that raises the crate's
/// version (`version_bump:V`) and one that shares a version bump or a release with another
/// open pull request (`overlapping_version_bump:#N`, `overlapping_release:#N`) are high-risk
/// too: whichever lands second must be re-derived on the first.
pub fn risk_of(
    authored: &[String],
    known: bool,
    version_bump: Option<&str>,
    overlaps: &[PathOverlap],
) -> (IntegrationRisk, Vec<String>) {
    let mut factors = Vec::new();
    let mut risk = IntegrationRisk::Low;
    if !known {
        risk = IntegrationRisk::High;
        factors.push("paths_unknown".into());
    }
    let high: BTreeSet<&str> = authored
        .iter()
        .filter_map(|p| HIGH_RISK.iter().find(|h| p.starts_with(**h)).copied())
        .collect();
    if !high.is_empty() {
        risk = IntegrationRisk::High;
        factors.extend(high.iter().map(|h| format!("touches {h}")));
    }
    if authored.len() > LARGE {
        risk = IntegrationRisk::High;
        factors.push(format!(
            "{} authored paths (more than {LARGE})",
            authored.len()
        ));
    }
    let shared: Vec<String> = version_bump
        .map(|v| format!("version_bump:{v}"))
        .into_iter()
        .chain(overlaps.iter().filter_map(|o| match o.kind {
            OverlapKind::VersionBump => Some(format!("overlapping_version_bump:#{}", o.number)),
            OverlapKind::Release => Some(format!("overlapping_release:#{}", o.number)),
            OverlapKind::Authored => None,
        }))
        .collect();
    if !shared.is_empty() {
        risk = IntegrationRisk::High;
        factors.extend(shared);
    }
    if risk == IntegrationRisk::Low
        && authored
            .iter()
            .any(|p| CODE.iter().any(|c| p.starts_with(c)))
    {
        risk = IntegrationRisk::Medium;
        factors.push("changes executable code".into());
    }
    if factors.is_empty() {
        factors.push("documentation, tests or content only".into());
    }
    (risk, factors)
}

/// What `number` has in common with each other open pull request, one entry per other: a
/// version bump when both raise the crate's version (by their [`ChangeShape`]s), else a
/// release when both change a record under [`RELEASES`], else the authored paths both
/// change. The paths are those both change, with — for a version or release overlap — the
/// paths that make it one: the manifest, or either's release records.
pub fn overlaps(
    number: u64,
    authored: &BTreeMap<u64, Vec<String>>,
    shapes: &BTreeMap<u64, ChangeShape>,
) -> Vec<PathOverlap> {
    let Some(mine) = authored.get(&number) else {
        return Vec::new();
    };
    let bumps = |n: &u64| shapes.get(n).is_some_and(|s| s.version_bump.is_some());
    let releases = |paths: &[String]| -> BTreeSet<String> {
        paths
            .iter()
            .filter(|p| p.starts_with(RELEASES))
            .cloned()
            .collect()
    };
    let my_releases = releases(mine);
    let mine: BTreeSet<&String> = mine.iter().collect();
    authored
        .iter()
        .filter(|(n, _)| **n != number)
        .filter_map(|(n, theirs)| {
            let their_releases = releases(theirs);
            let (kind, evidence) = if bumps(&number) && bumps(n) {
                let manifest = crate::release::version::MANIFEST.to_string();
                (OverlapKind::VersionBump, BTreeSet::from([manifest]))
            } else if !my_releases.is_empty() && !their_releases.is_empty() {
                let both = my_releases.union(&their_releases).cloned().collect();
                (OverlapKind::Release, both)
            } else {
                (OverlapKind::Authored, BTreeSet::new())
            };
            let paths: Vec<String> = theirs
                .iter()
                .filter(|p| mine.contains(p))
                .cloned()
                .chain(evidence)
                .collect::<BTreeSet<String>>()
                .into_iter()
                .collect();
            (!paths.is_empty()).then_some(PathOverlap {
                number: *n,
                paths,
                kind,
            })
        })
        .collect()
}

/// Everything [`classify`] reads about the rest of the queue.
#[derive(Debug, Clone, Default)]
pub struct QueueContext {
    /// The numbers of every open pull request.
    pub open: BTreeSet<u64>,
    /// Head branch name → number, for stacked pull requests: branches of this repository
    /// only, since a fork's branch name says nothing about a branch here.
    pub heads: BTreeMap<String, u64>,
    /// Authored paths of every open pull request, by number.
    pub authored: BTreeMap<u64, Vec<String>>,
    /// What each open pull request's merge changes, by kind, where git could say.
    pub shapes: BTreeMap<u64, ChangeShape>,
    /// The declared successors of each open pull request, by its number, in successor order:
    /// from its own body (`Superseded by #N`) and from any other's, open or not
    /// (`Supersedes #N`).
    pub superseded_by: BTreeMap<u64, Vec<Successor>>,
    /// What became of each declared dependency that is not open, by its number, as the
    /// forge reported it. A dependency neither open nor here is unread.
    pub dependency_states: BTreeMap<u64, DependencyState>,
    /// The open pull requests caught in a cycle of confirmed dependencies, each with the
    /// others of its cycle: none of them can ever land first.
    pub cycles: BTreeMap<u64, Vec<u64>>,
}

/// The pull requests `pr` is confirmed to depend on: those its body declares
/// ([`declared_dependencies`]), never itself, and the one it is stacked on — the open pull
/// request whose head branch is its base, when its base is not the integration base. The
/// one place the edges are decided, so the classifier and the cycle search read the same.
pub fn confirmed_dependencies(
    pr: &PullRequestObservation,
    base: &str,
    heads: &BTreeMap<String, u64>,
) -> (Vec<u64>, Option<u64>) {
    let declared: Vec<u64> = declared_dependencies(&pr.body)
        .into_iter()
        .filter(|n| *n != pr.number)
        .collect();
    // only a pull request that targets another branch can be stacked, and never on itself
    let stacked_on = (pr.base_ref != base)
        .then(|| heads.get(&pr.base_ref).copied())
        .flatten()
        .filter(|n| *n != pr.number);
    (declared, stacked_on)
}

/// One pull request declared to replace another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Successor {
    /// The successor.
    pub number: u64,
    /// Whose body declared it: the replaced one's own (`Superseded by`), or the successor's
    /// (`Supersedes`).
    pub declared_in: u64,
    /// What became of it.
    pub state: SuccessorState,
}

/// What became of a declared successor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuccessorState {
    /// Still open.
    Open,
    /// No longer open, and git finds its head in master: it landed.
    Landed {
        /// Its head, as the forge reported it.
        head_sha: String,
        /// Whether the forge calls it merged.
        merged: bool,
    },
    /// No longer open, and its head is not in master: closed unmerged, or merged by a squash
    /// or a rebase, which leaves no commit of its head on master.
    NotLanded {
        /// Whether the forge calls it merged.
        merged: bool,
    },
    /// Not open, and the forge or git could not say what became of it.
    Unread,
}

fn ev(
    kind: EvidenceKind,
    status: impl Into<String>,
    detail: impl Into<String>,
    source: &EvidenceSource,
) -> IntegrationEvidence {
    IntegrationEvidence {
        kind,
        status: status.into(),
        detail: detail.into(),
        source: Some(source.clone()),
    }
}

/// What a failing gate says: its reasons, and the disposition and next action it decides
/// when it is the first to fail. `decides` is `None` for a gate that fails only because an
/// earlier one did — such a gate is never the first.
struct Failure {
    reasons: Vec<ReasonCode>,
    decides: Option<(PullRequestDisposition, Option<String>)>,
}

fn fails(
    disposition: PullRequestDisposition,
    reasons: Vec<ReasonCode>,
    next: Option<String>,
) -> Option<Failure> {
    Some(Failure {
        reasons,
        decides: Some((disposition, next)),
    })
}

/// A gate that fails without a finding of its own: an earlier gate's answer is the reason.
fn fails_as_above() -> Option<Failure> {
    Some(Failure {
        reasons: Vec::new(),
        decides: None,
    })
}

/// The disposition of one pull request against one master commit, observed at `observed_at`.
///
/// Every gate of [`IntegrationGate::ALL`] is asked, in that order, and each answers whatever
/// the others said: `gates` holds every answer, `reasons` every failing gate's findings in
/// the same order, and the disposition is the first failing gate's. A draft that also
/// conflicts and fails its check is a draft, and says all three.
pub fn classify(
    pr: &PullRequestObservation,
    relation: &RelationToMaster,
    master_sha: &str,
    observed_at: &str,
    policy: &IntegrationPolicy,
    queue: &QueueContext,
) -> PullRequestAssessment {
    let checks = required_checks(
        &pr.checks,
        policy.required_checks.as_deref(),
        &policy.skipped_permitted,
    );
    let review = review_state(pr, policy.review_policy.as_ref());
    let shape = queue.shapes.get(&pr.number);
    // the merge's whole authored change where it was read: a conflicting head's relation names
    // only the paths it conflicts on
    let authored: Vec<String> = match (shape, relation) {
        (Some(s), _) => s.authored.clone(),
        (
            None,
            RelationToMaster::UpToDate { authored } | RelationToMaster::Behind { authored, .. },
        ) => authored.clone(),
        (None, RelationToMaster::Conflicting { paths }) => paths.clone(),
        _ => queue.authored.get(&pr.number).cloned().unwrap_or_default(),
    };
    let overlaps = overlaps(pr.number, &queue.authored, &queue.shapes);
    let (risk, risk_factors) = risk_of(
        &authored,
        !matches!(relation, RelationToMaster::Unknown { .. }),
        shape.and_then(|s| s.version_bump.as_deref()),
        &overlaps,
    );

    let (declared, stacked_on) = confirmed_dependencies(pr, &policy.base, &queue.heads);
    // satisfied only by a merge: closed without one, or not read, it never landed
    let state_of = |n: u64| {
        if queue.open.contains(&n) {
            DependencyState::Open
        } else {
            queue
                .dependency_states
                .get(&n)
                .copied()
                .unwrap_or(DependencyState::Unread)
        }
    };
    let dependencies: Vec<PullRequestDependency> =
        declared
            .into_iter()
            .chain(stacked_on)
            .fold(Vec::new(), |mut deps, n| {
                if !deps.iter().any(|d: &PullRequestDependency| d.number == n) {
                    let state = state_of(n);
                    deps.push(PullRequestDependency {
                        number: n,
                        certainty: DependencyCertainty::Confirmed,
                        satisfied: state == DependencyState::Merged,
                        state,
                    });
                }
                deps
            });
    let blocking: Vec<&String> = pr
        .labels
        .iter()
        .filter(|l| {
            policy
                .labels
                .iter()
                .any(|p| p.effect == LabelEffect::Hold && p.names(l))
        })
        .collect();
    let obsolete: Vec<&String> = pr
        .labels
        .iter()
        .filter(|l| {
            policy
                .labels
                .iter()
                .any(|p| p.effect == LabelEffect::Obsolete && p.names(l))
        })
        .collect();

    let forge = EvidenceSource::Forge {
        observed_at: observed_at.to_string(),
    };
    let git = EvidenceSource::Git {
        master_sha: master_sha.to_string(),
        head_sha: pr.head_sha.clone(),
    };
    let check_states = policy
        .required_checks
        .as_deref()
        .map(|r| required_check_states(&pr.checks, r, &policy.skipped_permitted))
        .unwrap_or_default();

    let mut evidence = vec![
        match stacked_on {
            _ if pr.base_ref == policy.base => ev(
                EvidenceKind::Base,
                "integration_base",
                format!("targets {}", pr.base_ref),
                &forge,
            ),
            Some(n) => ev(
                EvidenceKind::Base,
                "stacked",
                format!("targets {}, the head of #{n}", pr.base_ref),
                &forge,
            ),
            None => ev(
                EvidenceKind::Base,
                "other_base",
                format!(
                    "targets {}, which is not {} and no open pull request's head",
                    pr.base_ref, policy.base
                ),
                &forge,
            ),
        },
        if pr.draft {
            ev(EvidenceKind::Draft, "draft", "a draft", &forge)
        } else {
            ev(
                EvidenceKind::Draft,
                "ready_for_review",
                "not a draft",
                &forge,
            )
        },
        match &policy.required_checks {
            Some(r) if r.is_empty() => ev(
                EvidenceKind::RequiredChecks,
                "none_required",
                "the base requires no check: a head can prove nothing to it, so nothing merges (D5)",
                &forge,
            ),
            Some(r) => ev(
                EvidenceKind::RequiredChecks,
                word(&checks),
                check_states
                    .iter()
                    .zip(r)
                    .map(|((_, s), req)| format!("{req}: {}", word(s)))
                    .collect::<Vec<_>>()
                    .join(", "),
                &forge,
            ),
            None => ev(
                EvidenceKind::RequiredChecks,
                word(&checks),
                "the branch protection or rulesets could not be read",
                &forge,
            ),
        },
    ];
    for ((_, state), req) in check_states
        .iter()
        .zip(policy.required_checks.iter().flatten())
    {
        evidence.push(ev(
            EvidenceKind::RequiredCheck,
            word(state),
            req.to_string(),
            &forge,
        ));
    }
    evidence.push(ev(
        EvidenceKind::Review,
        word(&review),
        match policy.review_policy {
            Some(p) => format!(
                "{} approval(s) required{}{}; the forge says {}",
                p.approvals,
                if p.code_owners {
                    ", a code owner's among them"
                } else {
                    ""
                },
                if p.dismiss_stale {
                    ", stale approvals dismissed"
                } else {
                    ""
                },
                if pr.review_decision.is_empty() {
                    "nothing"
                } else {
                    pr.review_decision.as_str()
                }
            ),
            None => "the review policy could not be read".into(),
        },
        &forge,
    ));
    for r in &pr.latest_reviews {
        evidence.push(ev(
            EvidenceKind::Review,
            r.state.to_ascii_lowercase(),
            format!(
                "{} on {}",
                r.author,
                if r.commit.is_empty() {
                    "an unreported commit".to_string()
                } else if r.commit == pr.head_sha {
                    "the head".to_string()
                } else {
                    format!("another commit ({})", short(&r.commit))
                }
            ),
            &forge,
        ));
    }
    evidence.push(ev(
        EvidenceKind::RelationToMaster,
        relation_word(relation),
        relation_detail(relation),
        &git,
    ));
    let successors: &[Successor] = queue
        .superseded_by
        .get(&pr.number)
        .map_or(&[], Vec::as_slice);
    for s in successors {
        let declared = if s.declared_in == pr.number {
            format!("its body says superseded by #{}", s.number)
        } else {
            format!("#{} says it supersedes #{}", s.declared_in, pr.number)
        };
        evidence.push(match &s.state {
            SuccessorState::Open => ev(
                EvidenceKind::Supersession,
                "open",
                format!("#{} ({declared}) is open", s.number),
                &forge,
            ),
            SuccessorState::Landed { head_sha, merged } => ev(
                EvidenceKind::Supersession,
                "landed",
                format!(
                    "#{} ({declared}) is {}, and master contains its head {}",
                    s.number,
                    if *merged { "merged" } else { "closed" },
                    short(head_sha)
                ),
                &EvidenceSource::Git {
                    master_sha: master_sha.to_string(),
                    head_sha: head_sha.clone(),
                },
            ),
            SuccessorState::NotLanded { merged } => ev(
                EvidenceKind::Supersession,
                "not_landed",
                format!(
                    "#{} ({declared}) is {}, but master does not contain its head",
                    s.number,
                    if *merged {
                        "merged by a squash or a rebase"
                    } else {
                        "closed unmerged"
                    }
                ),
                &forge,
            ),
            SuccessorState::Unread => ev(
                EvidenceKind::Supersession,
                "unread",
                format!(
                    "#{} ({declared}) is not open, and what became of it could not be read",
                    s.number
                ),
                &forge,
            ),
        });
    }
    let landed: Option<u64> = successors
        .iter()
        .filter(|s| matches!(s.state, SuccessorState::Landed { .. }))
        .map(|s| s.number)
        .min();
    for d in &dependencies {
        evidence.push(ev(
            EvidenceKind::Dependency,
            word(&d.state),
            format!("#{} ({})", d.number, word(&d.certainty)),
            &forge,
        ));
    }
    for l in &blocking {
        evidence.push(ev(EvidenceKind::Label, "blocking", (*l).clone(), &forge));
    }
    if pr.auto_merge {
        evidence.push(ev(
            EvidenceKind::AutoMerge,
            "armed",
            "the forge has auto-merge armed: it would merge on its own, outside the executor",
            &forge,
        ));
    }
    if policy.merge_method.is_none() {
        evidence.push(ev(
            EvidenceKind::RepositorySettings,
            "merge_commit_not_allowed",
            "the repository's settings allow no merge commit; the executor merges only with one, \
             never by a squash or a rebase",
            &forge,
        ));
    }

    // every gate, in policy order; each answers whatever the others said
    let answers: [(IntegrationGate, Option<Failure>); 12] = [
        (
            IntegrationGate::Base,
            match stacked_on {
                _ if pr.base_ref == policy.base => None,
                Some(n) => fails(
                    PullRequestDisposition::WaitingForDependency,
                    vec![ReasonCode::StackedOn { number: n }],
                    Some(format!(
                        "land #{n}; its merge retargets this onto {}",
                        policy.base
                    )),
                ),
                None => fails(
                    PullRequestDisposition::OtherBase,
                    vec![ReasonCode::BaseIs {
                        base: pr.base_ref.clone(),
                    }],
                    None,
                ),
            },
        ),
        (
            IntegrationGate::Draft,
            if pr.draft {
                fails(
                    PullRequestDisposition::Draft,
                    vec![ReasonCode::Draft],
                    Some("mark it ready for review".into()),
                )
            } else {
                None
            },
        ),
        (
            IntegrationGate::Label,
            // obsolete before hold: a person acts on it either way, and the closer word wins
            if !obsolete.is_empty() {
                fails(
                    PullRequestDisposition::Obsolete,
                    obsolete
                        .iter()
                        .map(|l| ReasonCode::LabelObsolete { name: (*l).clone() })
                        .collect(),
                    Some("a person closes it, or removes the label".into()),
                )
            } else if blocking.is_empty() {
                None
            } else {
                fails(
                    PullRequestDisposition::Blocked,
                    blocking
                        .iter()
                        .map(|l| ReasonCode::Label { name: (*l).clone() })
                        .collect(),
                    Some("remove the label when it may land".into()),
                )
            },
        ),
        (
            IntegrationGate::AutoMerge,
            if pr.auto_merge {
                fails(
                    PullRequestDisposition::Unsafe,
                    vec![ReasonCode::AutoMergeArmed],
                    Some(format!(
                        "disarm auto-merge (gh pr merge {} --disable-auto); the executor merges \
                         one at a time against the current master",
                        pr.number
                    )),
                )
            } else {
                None
            },
        ),
        (IntegrationGate::Supersession, supersession(successors)),
        (
            IntegrationGate::RelationToMaster,
            match relation {
                RelationToMaster::Contained => fails(
                    PullRequestDisposition::Redundant,
                    vec![ReasonCode::HeadReachableFromMaster],
                    Some("close it: every commit is on master".into()),
                ),
                RelationToMaster::Superseded => fails(
                    PullRequestDisposition::Redundant,
                    vec![ReasonCode::MergeChangesNothing],
                    Some("close it: merging it into master changes no file".into()),
                ),
                RelationToMaster::PatchIdsUpstream { .. } => fails(
                    PullRequestDisposition::Redundant,
                    vec![ReasonCode::PatchIdsUpstream],
                    Some(
                        "close it: every one of its commits is on master as an equal patch".into(),
                    ),
                ),
                RelationToMaster::DerivedOnly { .. } => fails(
                    PullRequestDisposition::PossiblyRedundant,
                    vec![ReasonCode::OnlyDerivedArtifactsDiffer],
                    Some(
                        "a person confirms the authored change is on master, then closes it".into(),
                    ),
                ),
                RelationToMaster::Unknown { reason } => fails(
                    PullRequestDisposition::Unknown,
                    vec![ReasonCode::RelationUnknown {
                        reason: reason.clone(),
                    }],
                    Some("majordomus prs refresh".into()),
                ),
                RelationToMaster::Conflicting { paths } => fails(
                    PullRequestDisposition::Conflicting,
                    vec![ReasonCode::ConflictsOn { count: paths.len() }],
                    Some(format!(
                        "the author merges {} and resolves {}",
                        policy.base,
                        paths.join(", ")
                    )),
                ),
                RelationToMaster::UpToDate { .. } | RelationToMaster::Behind { .. } => None,
            },
        ),
        (
            IntegrationGate::MergeMethod,
            if policy.merge_method.is_none() {
                fails(
                    PullRequestDisposition::Blocked,
                    vec![ReasonCode::MergeCommitNotAllowed],
                    Some(
                        "allow merge commits in the repository's settings; the executor never \
                         squashes or rebases"
                            .into(),
                    ),
                )
            } else {
                None
            },
        ),
        (IntegrationGate::Dependency, {
            // a stacked pull request's base is said by the base gate, not again here
            let unmet = |state: DependencyState| -> Vec<u64> {
                dependencies
                    .iter()
                    .filter(|d| d.certainty == DependencyCertainty::Confirmed)
                    .filter(|d| d.state == state && Some(d.number) != stacked_on)
                    .map(|d| d.number)
                    .collect()
            };
            let cycle = queue.cycles.get(&pr.number).cloned().unwrap_or_default();
            let closed = unmet(DependencyState::ClosedUnmerged);
            let unread = unmet(DependencyState::Unread);
            let open = unmet(DependencyState::Open);
            // the cycle first: nothing else about the dependencies can resolve it
            if let Some(first) = cycle.first() {
                fails(
                    PullRequestDisposition::Blocked,
                    cycle
                        .iter()
                        .map(|n| ReasonCode::DependencyCycle { number: *n })
                        .collect(),
                    Some(format!(
                        "break the cycle: remove the declaration between #{} and #{first}",
                        pr.number
                    )),
                )
            } else if let Some(first) = closed.first() {
                fails(
                    PullRequestDisposition::Blocked,
                    closed
                        .iter()
                        .map(|n| ReasonCode::DependencyClosedUnmerged { number: *n })
                        .collect(),
                    Some(format!(
                        "#{first} was closed without a merge: reopen and land it, or remove the declaration"
                    )),
                )
            } else if let Some(first) = unread.first() {
                fails(
                    PullRequestDisposition::Unknown,
                    unread
                        .iter()
                        .map(|n| ReasonCode::DependencyUnread { number: *n })
                        .collect(),
                    Some(format!(
                        "majordomus prs refresh; if #{first} is not a pull request, remove the declaration"
                    )),
                )
            } else if let Some(first) = open.first() {
                fails(
                    PullRequestDisposition::WaitingForDependency,
                    open.iter()
                        .map(|n| ReasonCode::DependsOn { number: *n })
                        .collect(),
                    Some(format!("land #{first} first")),
                )
            } else {
                None
            }
        }),
        (
            IntegrationGate::Review,
            match review {
                PullRequestReview::ChangesRequested
                | PullRequestReview::Pending
                | PullRequestReview::Stale
                | PullRequestReview::CodeOwnersPending => fails(
                    PullRequestDisposition::WaitingForReview,
                    vec![ReasonCode::Review { state: review }],
                    Some("a reviewer approves it".into()),
                ),
                PullRequestReview::Unknown => fails(
                    PullRequestDisposition::Unknown,
                    vec![ReasonCode::ReviewPolicyUnread],
                    Some("majordomus prs refresh".into()),
                ),
                PullRequestReview::NotRequired | PullRequestReview::Approved => None,
            },
        ),
        (
            IntegrationGate::NoFailingCheck,
            if checks == RequiredCheckState::Failed {
                fails(
                    PullRequestDisposition::NeedsRepair,
                    vec![ReasonCode::RequiredCheckFailed],
                    Some("the author fixes the failing required check".into()),
                )
            } else {
                None
            },
        ),
        (
            IntegrationGate::Freshness,
            match relation {
                RelationToMaster::UpToDate { .. } => None,
                RelationToMaster::Behind { behind, .. } if pr.cross_repository => fails(
                    PullRequestDisposition::NeedsRepair,
                    vec![
                        ReasonCode::BehindMaster { commits: *behind },
                        ReasonCode::ForkHead,
                    ],
                    Some(format!(
                        "the author merges {} into the fork's branch",
                        policy.base
                    )),
                ),
                RelationToMaster::Behind { behind, .. } => fails(
                    PullRequestDisposition::NeedsRefresh,
                    vec![ReasonCode::BehindMaster { commits: *behind }],
                    Some(format!(
                        "bring {} in with the derived merge driver, derive, push; CI runs again",
                        policy.base
                    )),
                ),
                // the relation gate refused it, and that is the reason
                _ => fails_as_above(),
            },
        ),
        (
            IntegrationGate::RequiredChecks,
            match checks {
                RequiredCheckState::Passed | RequiredCheckState::Skipped => None,
                RequiredCheckState::Pending | RequiredCheckState::Missing => fails(
                    PullRequestDisposition::WaitingForChecks,
                    vec![ReasonCode::RequiredChecks { state: checks }],
                    Some("wait for the required checks on this head".into()),
                ),
                RequiredCheckState::Unknown
                    if policy.required_checks.as_ref().is_some_and(Vec::is_empty) =>
                {
                    fails(
                        PullRequestDisposition::Unknown,
                        vec![ReasonCode::NoRequiredChecks],
                        Some(format!(
                            "require a check on {} in its protection or a ruleset",
                            policy.base
                        )),
                    )
                }
                RequiredCheckState::Unknown => fails(
                    PullRequestDisposition::Unknown,
                    vec![ReasonCode::RequiredChecksUnread],
                    Some("majordomus prs refresh".into()),
                ),
                // the no-failing-check gate said so
                RequiredCheckState::Failed => fails_as_above(),
            },
        ),
    ];

    debug_assert!(
        answers.iter().map(|(g, _)| *g).eq(IntegrationGate::ALL),
        "the gates are answered in policy order"
    );
    let mut gates = Vec::with_capacity(answers.len());
    let mut reasons = Vec::new();
    let mut decided: Option<(PullRequestDisposition, Option<String>)> = None;
    for (gate, failure) in answers {
        gates.push(GateResult {
            gate,
            passed: failure.is_none(),
        });
        if let Some(f) = failure {
            reasons.extend(f.reasons);
            if decided.is_none() {
                decided = f.decides;
            }
        }
    }
    let (disposition, next) = decided.unwrap_or_else(|| {
        reasons = vec![
            ReasonCode::ContainsMaster,
            if checks == RequiredCheckState::Skipped {
                ReasonCode::RequiredChecksSkipped
            } else {
                ReasonCode::RequiredChecksPassed
            },
        ];
        (PullRequestDisposition::Ready, Some("merge it".into()))
    });

    PullRequestAssessment {
        number: pr.number,
        title: pr.title.clone(),
        author: pr.author.clone(),
        head_ref: pr.head_ref.clone(),
        base_ref: pr.base_ref.clone(),
        evaluated_against: EvaluatedAgainst {
            master_sha: master_sha.to_string(),
            head_sha: pr.head_sha.clone(),
            observed_at: observed_at.to_string(),
        },
        lane: disposition.lane(),
        // `superseded` and its `by` together, never one without the other
        superseded_by: landed.filter(|_| disposition == PullRequestDisposition::Superseded),
        issue: None,
        milestone: None,
        disposition,
        reasons,
        gates,
        next_action: next,
        required_checks: checks,
        review,
        relation: relation.clone(),
        dependencies,
        overlaps,
        authored_paths: authored,
        risk,
        risk_factors,
        evidence,
        created_at: pr.created_at.clone(),
        // the audit trail's to say, not the observation's: queue_of adds it
        wait: None,
        rank_factors: None,
        change_shape: shape.cloned(),
    }
}

/// What the supersession gate answers for these declared successors: nothing when there is
/// none. Of several, the strongest decides — one that landed makes it `superseded`, else one
/// still open makes it wait for that one, else one closed without landing leaves it to a person
/// (`possibly_redundant`), else it is `unknown` — and every successor gives its reason, the
/// deciding kind first.
fn supersession(successors: &[Successor]) -> Option<Failure> {
    let of = |want: fn(&SuccessorState) -> bool| -> Vec<u64> {
        successors
            .iter()
            .filter(|s| want(&s.state))
            .map(|s| s.number)
            .collect()
    };
    let landed = of(|s| matches!(s, SuccessorState::Landed { .. }));
    let open = of(|s| matches!(s, SuccessorState::Open));
    let not_landed = of(|s| matches!(s, SuccessorState::NotLanded { .. }));
    let unread = of(|s| matches!(s, SuccessorState::Unread));
    let mut reasons: Vec<ReasonCode> = Vec::new();
    reasons.extend(
        landed
            .iter()
            .map(|&number| ReasonCode::SupersededBy { number }),
    );
    reasons.extend(
        open.iter()
            .map(|&number| ReasonCode::SuccessorOpen { number }),
    );
    reasons.extend(
        not_landed
            .iter()
            .map(|&number| ReasonCode::SuccessorNotLanded { number }),
    );
    reasons.extend(
        unread
            .iter()
            .map(|&number| ReasonCode::SuccessorUnread { number }),
    );
    let (disposition, next) = if let Some(n) = landed.first() {
        (
            PullRequestDisposition::Superseded,
            format!("close it: #{n}, which supersedes it, landed"),
        )
    } else if let Some(n) = open.first() {
        (
            PullRequestDisposition::WaitingForDependency,
            format!("land #{n}, which supersedes it; this one is then closed, never merged"),
        )
    } else if let Some(n) = not_landed.first() {
        (
            PullRequestDisposition::PossiblyRedundant,
            format!(
                "#{n}, which was to supersede it, did not land: a person decides whether this \
                 one is still wanted, and removes the declaration if so"
            ),
        )
    } else {
        // no successor at all, when none is unread either: the gate passes
        let n = unread.first()?;
        (
            PullRequestDisposition::Unknown,
            format!("majordomus prs refresh; #{n} could not be read"),
        )
    };
    fails(disposition, reasons, Some(next))
}

/// The serialised word of any unit enum here.
pub fn word<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        _ => "unknown".into(),
    }
}

/// The relation's word, as evidence and every surface say it.
pub fn relation_word(r: &RelationToMaster) -> &'static str {
    match r {
        RelationToMaster::Contained => "contained",
        RelationToMaster::Superseded => "superseded",
        RelationToMaster::PatchIdsUpstream { .. } => "patch_ids_upstream",
        RelationToMaster::DerivedOnly { .. } => "derived_only",
        RelationToMaster::UpToDate { .. } => "up_to_date",
        RelationToMaster::Behind { .. } => "behind",
        RelationToMaster::Conflicting { .. } => "conflicting",
        RelationToMaster::Unknown { .. } => "unknown",
    }
}

fn relation_detail(r: &RelationToMaster) -> String {
    match r {
        RelationToMaster::Contained => "the head is an ancestor of master".into(),
        RelationToMaster::Superseded => "merging it into master changes no file".into(),
        RelationToMaster::PatchIdsUpstream { commits } => format!(
            "every one of its {commits} commit(s) master lacks is on master as an equal patch"
        ),
        RelationToMaster::DerivedOnly { paths } => {
            format!("only {} derived path(s) would change", paths.len())
        }
        RelationToMaster::UpToDate { authored } => {
            format!("contains master; {} authored path(s)", authored.len())
        }
        RelationToMaster::Behind { behind, authored } => format!(
            "{behind} master commit(s) behind; merges cleanly; {} authored path(s)",
            authored.len()
        ),
        RelationToMaster::Conflicting { paths } => format!("conflicts on {}", paths.join(", ")),
        RelationToMaster::Unknown { reason } => reason.clone(),
    }
}

/// A commit, abbreviated for a person.
fn short(sha: &str) -> &str {
    sha.get(..10).unwrap_or(sha)
}

#[cfg(test)]
mod decision_branches {
    //! The decisions the queue-level tests do not reach, each through `classify` or
    //! `review_state` with the inputs that make it.

    use super::*;
    use crate::integration::forge;
    use crate::integration::model::{
        PullRequestDisposition, ReasonCode, RelationToMaster, RequiredCheck,
    };

    /// One pull request, #1, from forge JSON with `extra` fields over a plain open one.
    fn pr(extra: serde_json::Value) -> PullRequestObservation {
        let mut v = serde_json::json!({
            "number": 1, "title": "t", "author": {"login": "a"}, "headRefName": "fix/1",
            "headRefOid": "h1", "baseRefName": "master", "isDraft": false, "labels": [],
            "createdAt": "2026-10-01T00:00:00Z", "updatedAt": "2026-10-01T00:00:00Z",
            "body": "", "statusCheckRollup": [], "reviewDecision": "",
            "autoMergeRequest": null, "isCrossRepository": false
        });
        for (k, val) in extra.as_object().unwrap() {
            v[k] = val.clone();
        }
        forge::pull_request_of(&v).unwrap()
    }

    fn policy(review: Option<ReviewPolicy>, skipped: &[&str]) -> IntegrationPolicy {
        IntegrationPolicy {
            base: "master".into(),
            required_checks: Some(vec![RequiredCheck::from("ci")]),
            review_policy: review,
            skipped_permitted: skipped.iter().map(|s| s.to_string()).collect(),
            labels: LABEL_POLICY.to_vec(),
            merge_method: Some("merge".into()),
            up_to_date_required: None,
        }
    }

    fn context() -> QueueContext {
        QueueContext {
            open: [1].into(),
            ..Default::default()
        }
    }

    fn up_to_date() -> RelationToMaster {
        RelationToMaster::UpToDate {
            authored: vec!["a.txt".into()],
        }
    }

    #[test]
    fn a_change_nobody_could_read_is_high_risk_and_never_documentation() {
        assert_eq!(
            risk_of(&[], false, None, &[]),
            (IntegrationRisk::High, vec!["paths_unknown".to_string()])
        );
        assert_eq!(
            risk_of(&[], true, None, &[]),
            (
                IntegrationRisk::Low,
                vec!["documentation, tests or content only".to_string()]
            )
        );
        let (risk, factors) = risk_of(&[".ai/repo/releases/v1.yaml".into()], true, None, &[]);
        assert_eq!(risk, IntegrationRisk::High);
        assert_eq!(factors, vec!["touches .ai/repo/releases/".to_string()]);
    }

    #[test]
    fn the_label_constructor_and_the_check_ranking_run() {
        assert_eq!(LabelPolicy::hold("wip").effect, LabelEffect::Hold);
        assert_eq!(
            worse(RequiredCheckState::Passed, RequiredCheckState::Unknown),
            RequiredCheckState::Unknown
        );
        assert_eq!(
            worse(RequiredCheckState::Unknown, RequiredCheckState::Missing),
            RequiredCheckState::Unknown
        );
    }

    #[test]
    fn enough_approvals_still_pending_a_review_the_forge_requires() {
        let p = pr(serde_json::json!({
            "reviewDecision": "REVIEW_REQUIRED",
            "latestReviews": [{"author": {"login": "r"}, "state": "APPROVED", "commit": {"oid": "h1"}}]
        }));
        let required = ReviewPolicy {
            approvals: 1,
            code_owners: false,
            dismiss_stale: false,
        };
        assert_eq!(
            review_state(&p, Some(&required)),
            PullRequestReview::Pending
        );
    }

    #[test]
    fn the_review_evidence_names_code_owners_stale_dismissal_and_an_unreported_commit() {
        let p = pr(serde_json::json!({
            "statusCheckRollup": [{"__typename": "CheckRun", "name": "ci", "status": "COMPLETED", "conclusion": "SUCCESS"}],
            "latestReviews": [{"author": {"login": "r"}, "state": "APPROVED"}]
        }));
        let strict = ReviewPolicy {
            approvals: 1,
            code_owners: true,
            dismiss_stale: true,
        };
        let a = classify(
            &p,
            &up_to_date(),
            "m",
            "t",
            &policy(Some(strict), &[]),
            &context(),
        );
        let details: Vec<&str> = a.evidence.iter().map(|e| e.detail.as_str()).collect();
        assert!(
            details
                .iter()
                .any(|d| d.contains(", a code owner's among them")
                    && d.contains(", stale approvals dismissed")),
            "{details:?}"
        );
        assert!(
            details
                .iter()
                .any(|d| d.contains("r on an unreported commit")),
            "{details:?}"
        );
    }

    #[test]
    fn a_successor_closed_without_a_merge_whose_head_landed_is_said_closed() {
        let p = pr(serde_json::json!({}));
        let mut ctx = context();
        ctx.superseded_by.insert(
            1,
            vec![Successor {
                number: 2,
                declared_in: 1,
                state: SuccessorState::Landed {
                    head_sha: "h2".into(),
                    merged: false,
                },
            }],
        );
        let a = classify(&p, &up_to_date(), "m", "t", &policy(None, &[]), &ctx);
        assert_eq!(a.disposition, PullRequestDisposition::Superseded);
        assert!(
            a.evidence.iter().any(|e| e
                .detail
                .contains("#2 (its body says superseded by #2) is closed")),
            "{:?}",
            a.evidence
        );
    }

    #[test]
    fn a_permitted_skip_is_ready_and_says_so() {
        let p = pr(serde_json::json!({
            "statusCheckRollup": [{"__typename": "CheckRun", "name": "ci", "status": "COMPLETED", "conclusion": "SKIPPED"}]
        }));
        let a = classify(
            &p,
            &up_to_date(),
            "m",
            "t",
            &policy(
                Some(ReviewPolicy {
                    approvals: 0,
                    code_owners: false,
                    dismiss_stale: false,
                }),
                &["ci"],
            ),
            &context(),
        );
        assert_eq!(
            a.disposition,
            PullRequestDisposition::Ready,
            "{:?}",
            a.reasons
        );
        assert!(
            a.reasons.contains(&ReasonCode::RequiredChecksSkipped),
            "{:?}",
            a.reasons
        );
    }
}

#[cfg(test)]
mod label_policy_tests {
    use super::*;

    /// The constructors run where the table is built, at compile time; run once here too.
    #[test]
    fn the_constructors_say_their_effect() {
        let o = LabelPolicy::obsolete("obsolete");
        assert_eq!(o.effect, LabelEffect::Obsolete);
        assert!(o.names("OBSOLETE"));
        assert_eq!(LabelPolicy::hold("wip").effect, LabelEffect::Hold);
    }
}

#[cfg(test)]
mod dependency_resolution {
    //! A declared dependency is satisfied by a merge only (WP14).

    use super::*;
    use crate::integration::forge;
    use crate::integration::model::{RelationToMaster, RequiredCheck};

    fn pr(body: &str) -> PullRequestObservation {
        forge::pull_request_of(&serde_json::json!({
            "number": 1, "title": "t", "author": {"login": "a"}, "headRefName": "fix/1",
            "headRefOid": "h1", "baseRefName": "master", "isDraft": false, "labels": [],
            "createdAt": "2026-10-01T00:00:00Z", "updatedAt": "2026-10-01T00:00:00Z",
            "body": body, "statusCheckRollup": [], "reviewDecision": "",
            "autoMergeRequest": null, "isCrossRepository": false
        }))
        .unwrap()
    }

    fn policy() -> IntegrationPolicy {
        IntegrationPolicy {
            base: "master".into(),
            required_checks: Some(vec![RequiredCheck::from("ci")]),
            review_policy: None,
            skipped_permitted: Vec::new(),
            labels: LABEL_POLICY.to_vec(),
            merge_method: Some("merge".into()),
            up_to_date_required: None,
        }
    }

    fn decided(ctx: &QueueContext) -> PullRequestAssessment {
        let relation = RelationToMaster::UpToDate {
            authored: vec!["a.txt".into()],
        };
        classify(&pr("Depends on #7"), &relation, "m", "t", &policy(), ctx)
    }

    fn gate(a: &PullRequestAssessment) -> bool {
        a.gates
            .iter()
            .find(|g| g.gate == IntegrationGate::Dependency)
            .unwrap()
            .passed
    }

    fn with(state: Option<DependencyState>) -> QueueContext {
        let mut ctx = QueueContext {
            open: [1].into(),
            ..Default::default()
        };
        if let Some(s) = state {
            ctx.dependency_states.insert(7, s);
        }
        ctx
    }

    #[test]
    fn only_a_merge_satisfies_a_dependency() {
        let merged = decided(&with(Some(DependencyState::Merged)));
        assert!(gate(&merged), "{:?}", merged.reasons);
        assert!(merged.dependencies[0].satisfied);
        assert!(merged
            .evidence
            .iter()
            .any(|e| e.kind == EvidenceKind::Dependency && e.status == "merged"));

        let closed = decided(&with(Some(DependencyState::ClosedUnmerged)));
        assert_eq!(closed.disposition, PullRequestDisposition::Blocked);
        assert_eq!(
            closed.reasons[0].to_string(),
            "dependency_closed_unmerged:#7"
        );
        assert!(!closed.dependencies[0].satisfied);

        let unread = decided(&with(None));
        assert_eq!(unread.disposition, PullRequestDisposition::Unknown);
        assert_eq!(unread.reasons[0].to_string(), "dependency_unread:#7");
        assert!(unread
            .next_action
            .as_deref()
            .is_some_and(|n| n.contains("not a pull request")));

        let mut open = with(None);
        open.open.insert(7);
        let waiting = decided(&open);
        assert_eq!(
            waiting.disposition,
            PullRequestDisposition::WaitingForDependency
        );
    }

    #[test]
    fn a_dependency_both_declared_and_stacked_on_is_one_dependency() {
        let mut p = pr("Depends on #7");
        p.base_ref = "fix/7".into();
        let mut ctx = with(None);
        ctx.open.insert(7);
        ctx.heads.insert("fix/7".into(), 7);
        let relation = RelationToMaster::UpToDate {
            authored: vec!["a.txt".into()],
        };
        let a = classify(&p, &relation, "m", "t", &policy(), &ctx);
        assert_eq!(a.dependencies.len(), 1, "{:?}", a.dependencies);
        assert_eq!(a.dependencies[0].number, 7);
    }

    #[test]
    fn a_cycle_is_blocked_before_anything_else_is_said() {
        let mut ctx = with(Some(DependencyState::ClosedUnmerged));
        ctx.cycles.insert(1, vec![7]);
        let a = decided(&ctx);
        assert_eq!(a.disposition, PullRequestDisposition::Blocked);
        assert_eq!(a.reasons[0].to_string(), "dependency_cycle:#7");
        assert!(a
            .next_action
            .as_deref()
            .is_some_and(|n| n.contains("break the cycle")));
    }
}
