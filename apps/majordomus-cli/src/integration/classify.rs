//! The one path that decides a disposition. Pure: observations in, an [`PullRequestAssessment`] out.
//!
//! The order of the questions is the policy. A pull request that is a draft is a draft
//! whatever its checks say; one whose patch is already on master is superseded whatever
//! its conflicts say; and `Ready` is reached only by the last branch, after every other
//! question was answered in its favour. Nothing upstream of this file decides eligibility
//! and nothing downstream re-decides it.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::model::{
    CheckRunState, DependencyCertainty, EvaluatedAgainst, IntegrationEvidence, IntegrationRisk,
    PathOverlap, PullRequestAssessment, PullRequestDependency, PullRequestDisposition,
    PullRequestObservation, PullRequestReview, RelationToMaster, RequiredCheckState,
};

/// What the repository requires before a merge, as observed from the forge and the
/// repository — never a list kept here beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IntegrationPolicy {
    /// The branch integrated into.
    pub base: String,
    /// The status contexts the branch protection requires, as read from the forge.
    /// `None` when the protection could not be read: every required-check verdict is then
    /// `unknown`, never `passed`.
    pub required_checks: Option<Vec<String>>,
    /// Whether the branch protection requires an approving review; `None` when unread.
    pub reviews_required: Option<bool>,
    /// Labels that hold a pull request whatever else is true.
    pub blocking_labels: Vec<String>,
    /// The merge method the executor asks the forge for.
    pub merge_method: String,
}

/// The labels that hold a pull request. One list, here; the forge's own label names are
/// compared case-insensitively against it.
pub const BLOCKING_LABELS: &[&str] = &[
    "do-not-merge",
    "do not merge",
    "blocked",
    "hold",
    "on-hold",
    "wip",
    "manual-merge",
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
    "share/version.txt",
    "apps/majordomus-cli/Cargo.toml",
    "Cargo.lock",
    "apps/majordomus-cli/Cargo.lock",
    "SECURITY.md",
];

/// Path prefixes of executable code: medium risk.
const CODE: &[&str] = &["apps/", "bin/", "lib/", "scripts/"];

/// More authored paths than this is a large change.
const LARGE: usize = 40;

/// The required-check verdict for one head, from the checks reported on it.
///
/// ```text
/// use crate::integration::{required_checks, CheckObservation, CheckRunState, RequiredCheckState};
/// let ci = |state| vec![CheckObservation { name: "ci".into(), state }];
/// let req = Some(vec!["ci".to_string()]);
/// assert_eq!(required_checks(&ci(CheckRunState::Passed), req.as_deref()), RequiredCheckState::Passed);
/// // a green check that is not the required one proves nothing
/// let other = vec![CheckObservation { name: "suite".into(), state: CheckRunState::Passed }];
/// assert_eq!(required_checks(&other, req.as_deref()), RequiredCheckState::Missing);
/// // unread protection is never a pass
/// assert_eq!(required_checks(&ci(CheckRunState::Passed), None), RequiredCheckState::Unknown);
/// ```text
pub fn required_checks(
    checks: &[super::model::CheckObservation],
    required: Option<&[String]>,
) -> RequiredCheckState {
    let Some(required) = required else {
        return RequiredCheckState::Unknown;
    };
    let mut verdict = RequiredCheckState::Passed;
    for name in required {
        // the newest report of a context wins; the forge lists re-runs as separate entries,
        // and any failing one among the latest reports is a failure
        let states: Vec<CheckRunState> = checks
            .iter()
            .filter(|c| &c.name == name)
            .map(|c| c.state)
            .collect();
        let this = if states.is_empty() {
            RequiredCheckState::Missing
        } else if states.contains(&CheckRunState::Pending) {
            RequiredCheckState::Pending
        } else if states.iter().all(|s| *s == CheckRunState::Passed) {
            RequiredCheckState::Passed
        } else if states.contains(&CheckRunState::Failed) {
            RequiredCheckState::Failed
        } else {
            // skipped or neutral is not a pass of a required check
            RequiredCheckState::Missing
        };
        verdict = worse(verdict, this);
    }
    verdict
}

fn worse(a: RequiredCheckState, b: RequiredCheckState) -> RequiredCheckState {
    let rank = |r| match r {
        RequiredCheckState::Passed => 0,
        RequiredCheckState::Pending => 1,
        RequiredCheckState::Missing => 2,
        RequiredCheckState::Unknown => 3,
        RequiredCheckState::Failed => 4,
    };
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

/// The review state under the protection's requirement. The forge's own decision comes
/// first: `REVIEW_REQUIRED` means some rule requires a review that has not been given — a
/// ruleset or code owners can require one the branch protection does not — so it is pending
/// whatever the protection says.
pub fn review_state(decision: &str, required: Option<bool>) -> PullRequestReview {
    match (required, decision) {
        (_, "CHANGES_REQUESTED") => PullRequestReview::ChangesRequested,
        (_, "APPROVED") => PullRequestReview::Approved,
        (_, "REVIEW_REQUIRED") => PullRequestReview::Pending,
        (Some(false), _) => PullRequestReview::NotRequired,
        (Some(true), _) => PullRequestReview::Pending,
        (None, _) => PullRequestReview::Unknown,
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

/// The planning risk of a change, with its factors.
pub fn risk_of(authored: &[String]) -> (IntegrationRisk, Vec<String>) {
    let mut factors = Vec::new();
    let mut risk = IntegrationRisk::Low;
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

/// Authored paths shared with other open pull requests.
pub fn overlaps(number: u64, authored: &BTreeMap<u64, Vec<String>>) -> Vec<PathOverlap> {
    let Some(mine) = authored.get(&number) else {
        return Vec::new();
    };
    let mine: BTreeSet<&String> = mine.iter().collect();
    authored
        .iter()
        .filter(|(n, _)| **n != number)
        .filter_map(|(n, theirs)| {
            let shared: Vec<String> = theirs
                .iter()
                .filter(|p| mine.contains(p))
                .cloned()
                .collect();
            (!shared.is_empty()).then_some(PathOverlap {
                number: *n,
                paths: shared,
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
}

fn ev(kind: &str, status: impl Into<String>, detail: impl Into<String>) -> IntegrationEvidence {
    IntegrationEvidence {
        kind: kind.into(),
        status: status.into(),
        detail: detail.into(),
    }
}

/// The disposition of one pull request against one master commit.
pub fn classify(
    pr: &PullRequestObservation,
    relation: &RelationToMaster,
    master_sha: &str,
    policy: &IntegrationPolicy,
    queue: &QueueContext,
) -> PullRequestAssessment {
    let checks = required_checks(&pr.checks, policy.required_checks.as_deref());
    let review = review_state(&pr.review_decision, policy.reviews_required);
    let authored: Vec<String> = match relation {
        RelationToMaster::UpToDate { authored } | RelationToMaster::Behind { authored, .. } => {
            authored.clone()
        }
        RelationToMaster::Conflicting { paths } => paths.clone(),
        _ => queue.authored.get(&pr.number).cloned().unwrap_or_default(),
    };
    let (risk, risk_factors) = risk_of(&authored);

    let mut dependencies: Vec<PullRequestDependency> = declared_dependencies(&pr.body)
        .into_iter()
        .filter(|n| *n != pr.number)
        .map(|n| PullRequestDependency {
            number: n,
            certainty: DependencyCertainty::Confirmed,
            satisfied: !queue.open.contains(&n),
        })
        .collect();
    // only a pull request that targets another branch can be stacked, and never on itself
    let stacked_on = (pr.base_ref != policy.base)
        .then(|| queue.heads.get(&pr.base_ref).copied())
        .flatten()
        .filter(|n| *n != pr.number);
    if let Some(n) = stacked_on {
        if !dependencies.iter().any(|d| d.number == n) {
            dependencies.push(PullRequestDependency {
                number: n,
                certainty: DependencyCertainty::Confirmed,
                satisfied: false,
            });
        }
    }

    let mut evidence = vec![
        ev(
            "required_checks",
            word(&checks),
            match &policy.required_checks {
                Some(r) => format!("required: {}", r.join(", ")),
                None => "the branch protection could not be read".into(),
            },
        ),
        ev("review", word(&review), pr.review_decision.clone()),
        ev(
            "relation_to_master",
            relation_word(relation),
            relation_detail(relation),
        ),
    ];
    for d in &dependencies {
        evidence.push(ev(
            "dependency",
            if d.satisfied { "satisfied" } else { "open" },
            format!("#{} ({})", d.number, word(&d.certainty)),
        ));
    }
    let blocking: Vec<&String> = pr
        .labels
        .iter()
        .filter(|l| {
            policy
                .blocking_labels
                .iter()
                .any(|b| b.eq_ignore_ascii_case(l))
        })
        .collect();

    let (disposition, reasons, next): (PullRequestDisposition, Vec<String>, Option<String>) = if pr
        .base_ref
        != policy.base
    {
        match stacked_on {
            Some(n) => (
                PullRequestDisposition::WaitingForDependency,
                vec![format!("stacked_on:#{n}")],
                Some(format!(
                    "land #{n}; its merge retargets this onto {}",
                    policy.base
                )),
            ),
            None => (
                PullRequestDisposition::OtherBase,
                vec![format!("base_is:{}", pr.base_ref)],
                None,
            ),
        }
    } else if pr.draft {
        (
            PullRequestDisposition::Draft,
            vec!["draft".into()],
            Some("mark it ready for review".into()),
        )
    } else if !blocking.is_empty() {
        (
            PullRequestDisposition::Blocked,
            blocking.iter().map(|l| format!("label:{l}")).collect(),
            Some("remove the label when it may land".into()),
        )
    } else {
        match relation {
            RelationToMaster::Contained => (
                PullRequestDisposition::Superseded,
                vec!["head_reachable_from_master".into()],
                Some("close it: every commit is on master".into()),
            ),
            RelationToMaster::Superseded => (
                PullRequestDisposition::Superseded,
                vec!["merge_changes_nothing".into()],
                Some("close it: merging it into master changes no file".into()),
            ),
            RelationToMaster::DerivedOnly { .. } => (
                PullRequestDisposition::PossiblyRedundant,
                vec!["only_derived_artifacts_differ".into()],
                Some("a person confirms the authored change is on master, then closes it".into()),
            ),
            RelationToMaster::Unknown { reason } => (
                PullRequestDisposition::Unknown,
                vec![format!("relation_unknown:{reason}")],
                Some("majordomus prs refresh".into()),
            ),
            RelationToMaster::Conflicting { paths } => (
                PullRequestDisposition::Conflicting,
                vec![format!("conflicts_on:{}", paths.len())],
                Some(format!(
                    "the author merges {} and resolves {}",
                    policy.base,
                    paths.join(", ")
                )),
            ),
            RelationToMaster::UpToDate { .. } | RelationToMaster::Behind { .. } => {
                if let Some(d) = dependencies
                    .iter()
                    .find(|d| d.certainty == DependencyCertainty::Confirmed && !d.satisfied)
                {
                    (
                        PullRequestDisposition::WaitingForDependency,
                        vec![format!("depends_on:#{}", d.number)],
                        Some(format!("land #{} first", d.number)),
                    )
                } else if matches!(
                    review,
                    PullRequestReview::ChangesRequested | PullRequestReview::Pending
                ) {
                    (
                        PullRequestDisposition::WaitingForReview,
                        vec![format!("review:{}", word(&review))],
                        Some("a reviewer approves it".into()),
                    )
                } else if review == PullRequestReview::Unknown {
                    (
                        PullRequestDisposition::Unknown,
                        vec!["review_policy_unread".into()],
                        Some("majordomus prs refresh".into()),
                    )
                } else if checks == RequiredCheckState::Failed {
                    let mut reasons = vec!["required_check_failed".to_string()];
                    if let RelationToMaster::Behind { behind, .. } = relation {
                        reasons.push(format!("behind_master:{behind}"));
                    }
                    (
                        PullRequestDisposition::NeedsRepair,
                        reasons,
                        Some("the author fixes the failing required check".into()),
                    )
                } else if let (RelationToMaster::Behind { behind, .. }, true) =
                    (relation, pr.cross_repository)
                {
                    (
                        PullRequestDisposition::NeedsRepair,
                        vec![format!("behind_master:{behind}"), "fork_head".into()],
                        Some(format!(
                            "the author merges {} into the fork's branch",
                            policy.base
                        )),
                    )
                } else if let RelationToMaster::Behind { behind, .. } = relation {
                    (
                            PullRequestDisposition::NeedsRefresh,
                            vec![format!("behind_master:{behind}")],
                            Some(format!(
                                "bring {} in with the derived merge driver, derive, push; CI runs again",
                                policy.base
                            )),
                        )
                } else {
                    match checks {
                        RequiredCheckState::Passed => (
                            PullRequestDisposition::Ready,
                            vec!["contains_master".into(), "required_checks_passed".into()],
                            Some("merge it".into()),
                        ),
                        RequiredCheckState::Pending | RequiredCheckState::Missing => (
                            PullRequestDisposition::WaitingForChecks,
                            vec![format!("required_checks:{}", word(&checks))],
                            Some("wait for the required checks on this head".into()),
                        ),
                        RequiredCheckState::Unknown => (
                            PullRequestDisposition::Unknown,
                            vec!["required_checks_unread".into()],
                            Some("majordomus prs refresh".into()),
                        ),
                        RequiredCheckState::Failed => unreachable!("handled above"),
                    }
                }
            }
        }
    };
    if disposition == PullRequestDisposition::Draft
        || disposition == PullRequestDisposition::Blocked
    {
        for l in &blocking {
            evidence.push(ev("label", "blocking", (*l).clone()));
        }
    }

    PullRequestAssessment {
        number: pr.number,
        title: pr.title.clone(),
        author: pr.author.clone(),
        head_ref: pr.head_ref.clone(),
        base_ref: pr.base_ref.clone(),
        evaluated_against: EvaluatedAgainst {
            master_sha: master_sha.to_string(),
            head_sha: pr.head_sha.clone(),
        },
        lane: disposition.lane(),
        disposition,
        reasons,
        next_action: next,
        required_checks: checks,
        review,
        relation: relation.clone(),
        dependencies,
        overlaps: overlaps(pr.number, &queue.authored),
        authored_paths: authored,
        risk,
        risk_factors,
        evidence,
        created_at: pr.created_at.clone(),
        // the audit trail's to say, not the observation's: queue_of adds it
        wait: None,
    }
}

/// The serialised word of any unit enum here.
pub fn word<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        _ => "unknown".into(),
    }
}

fn relation_word(r: &RelationToMaster) -> &'static str {
    match r {
        RelationToMaster::Contained => "contained",
        RelationToMaster::Superseded => "superseded",
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
