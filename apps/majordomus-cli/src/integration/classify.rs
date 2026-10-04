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
    CheckKind, CheckRunState, DependencyCertainty, EvaluatedAgainst, IntegrationEvidence,
    IntegrationRisk, PathOverlap, PullRequestAssessment, PullRequestDependency,
    PullRequestDisposition, PullRequestObservation, PullRequestReview, RelationToMaster,
    RequiredCheck, RequiredCheckState, ReviewPolicy,
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
    let checks = required_checks(
        &pr.checks,
        policy.required_checks.as_deref(),
        &policy.skipped_permitted,
    );
    let review = review_state(pr, policy.review_policy.as_ref());
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

    let mut evidence = vec![match &policy.required_checks {
        Some(r) if r.is_empty() => ev(
            "required_checks",
            "none_required",
            "the base requires no check: a head can prove nothing to it, so nothing merges (D5)",
        ),
        Some(r) => ev(
            "required_checks",
            word(&checks),
            required_check_states(&pr.checks, r, &policy.skipped_permitted)
                .iter()
                .zip(r)
                .map(|((_, s), req)| format!("{req}: {}", word(s)))
                .collect::<Vec<_>>()
                .join(", "),
        ),
        None => ev(
            "required_checks",
            word(&checks),
            "the branch protection or rulesets could not be read",
        ),
    }];
    evidence.push(ev(
        "review",
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
    ));
    for r in &pr.latest_reviews {
        evidence.push(ev(
            "review",
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
        ));
    }
    evidence.extend([ev(
        "relation_to_master",
        relation_word(relation),
        relation_detail(relation),
    )]);
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
                    PullRequestReview::ChangesRequested
                        | PullRequestReview::Pending
                        | PullRequestReview::Stale
                        | PullRequestReview::CodeOwnersPending
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
                        RequiredCheckState::Passed | RequiredCheckState::Skipped => (
                            PullRequestDisposition::Ready,
                            vec![
                                "contains_master".into(),
                                format!("required_checks_{}", word(&checks)),
                            ],
                            Some("merge it".into()),
                        ),
                        RequiredCheckState::Pending | RequiredCheckState::Missing => (
                            PullRequestDisposition::WaitingForChecks,
                            vec![format!("required_checks:{}", word(&checks))],
                            Some("wait for the required checks on this head".into()),
                        ),
                        RequiredCheckState::Unknown
                            if policy.required_checks.as_ref().is_some_and(Vec::is_empty) =>
                        {
                            (
                                PullRequestDisposition::Unknown,
                                vec!["no_required_checks".into()],
                                Some(format!(
                                    "require a check on {} in its protection or a ruleset",
                                    policy.base
                                )),
                            )
                        }
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

/// A commit, abbreviated for a person.
fn short(sha: &str) -> &str {
    sha.get(..10).unwrap_or(sha)
}
