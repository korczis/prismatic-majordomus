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
    ChangeShape, CheckKind, CheckRunState, CrossReferenceRead, DependencyCertainty,
    DependencyState, EvaluatedAgainst, EvidenceKind, EvidenceSource, GateResult,
    IntegrationEvidence, IntegrationGate, IntegrationRisk, OverlapKind, PathOverlap,
    PullRequestAssessment, PullRequestDependency, PullRequestDisposition, PullRequestObservation,
    PullRequestReview, ReasonCode, RelationToMaster, RequiredCheck, RequiredCheckState,
    ReviewPolicy,
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

/// What to do about a check run of a context bound to an app that names no app. A refresh
/// clears only one of the ways it comes about, and the text says which.
pub const UNREAD_WRITER_REMEDY: &str = "majordomus prs refresh: a check run of a context \
bound to an app names no app. A refresh clears a head that moved during the read; a check \
suite whose app the forge does not name, or a head carrying more than 1000 checks, stays \
unknown until a person looks";

/// The queue's word on check runs whose writer was not read: one line for each requirement
/// bound to an app that some open pull request carries an unattributed check run of, naming
/// the pull requests, in the order the base requires them. Nothing when every writer was
/// read. Without it a queue holding only such pull requests would say nothing is ready and
/// not why: an observation recorded before writers were read turns every one of them
/// `unknown`.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::classify::unread_writers;
/// use majordomus_cli::integration::forge::pull_request_of;
/// use majordomus_cli::integration::RequiredCheck;
/// let listed = serde_json::json!({"number": 7, "statusCheckRollup": [
///     {"__typename": "CheckRun", "name": "ci", "status": "COMPLETED", "conclusion": "SUCCESS"}]});
/// let bound = [RequiredCheck { context: "ci".into(), app_id: Some(15368) }];
/// let said = unread_writers(&[pull_request_of(&listed).unwrap()], &bound);
/// assert!(said[0].starts_with("1 pull request(s) carry a check run of ci (app 15368)"));
/// assert!(said[0].contains("(#7)"));
/// ```
pub fn unread_writers(prs: &[PullRequestObservation], required: &[RequiredCheck]) -> Vec<String> {
    required
        .iter()
        .filter(|req| req.app_id.is_some())
        .filter_map(|req| {
            let unread = |c: &super::model::CheckObservation| {
                c.name == req.context && c.kind == CheckKind::CheckRun && c.app_id.is_none()
            };
            let carrying: Vec<String> = prs
                .iter()
                .filter(|p| p.checks.iter().any(&unread))
                .map(|p| format!("#{}", p.number))
                .collect();
            (!carrying.is_empty()).then(|| {
                format!(
                    "{} pull request(s) carry a check run of {req} whose app was not read ({}): \
                     that check is unknown on them, never passed. {UNREAD_WRITER_REMEDY}",
                    carrying.len(),
                    carrying.join(" ")
                )
            })
        })
        .collect()
}

/// The state of each required check on one head, in the order the base requires them.
///
/// For each requirement, only the reports that may stand for it are read. When the base binds
/// the context to an app, those are that app's check runs and nothing else: a status context
/// of the name, or another app's check run of it, neither passes the check nor fails it nor
/// holds it pending. Of the reports that stand, one still running makes the check pending;
/// otherwise the newest completed report is the verdict, so a failure followed by a passing
/// re-run has passed. A skip is a pass only for a context in `skipped_permitted`; otherwise
/// it is `missing`.
///
/// A check run of a bound context that names no app is one whose writer was not read. It is
/// never taken for the bound app's, and never ignored either: the check is then `unknown`,
/// unless the bound app's own verdict is a failure, which stands. A context two apps are
/// bound to is two requirements, each answered by its own app.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::{CheckObservation, CheckRunState, RequiredCheck, RequiredCheckState};
/// use majordomus_cli::integration::classify::required_check_states;
/// let run = |app_id| CheckObservation { name: "ci".into(), state: CheckRunState::Passed, app_id, ..Default::default() };
/// let bound = [RequiredCheck { context: "ci".into(), app_id: Some(15368) }];
/// let state = |checks: &[CheckObservation]| required_check_states(checks, &bound, &[])[0].1;
/// assert_eq!(state(&[run(Some(15368))]), RequiredCheckState::Passed);
/// assert_eq!(state(&[run(Some(99))]), RequiredCheckState::Missing, "another app's run is not it");
/// assert_eq!(state(&[run(None)]), RequiredCheckState::Unknown, "an unread writer is never a pass");
/// ```
pub fn required_check_states(
    checks: &[super::model::CheckObservation],
    required: &[RequiredCheck],
    skipped_permitted: &[String],
) -> Vec<(String, RequiredCheckState)> {
    required
        .iter()
        .map(|req| {
            let named: Vec<&super::model::CheckObservation> =
                checks.iter().filter(|c| c.name == req.context).collect();
            // bound to an app: only that app's check runs stand for the context, and a check
            // run of the name whose writer was not read leaves the verdict unproved
            let (reports, unattributed) = match req.app_id {
                None => (named, false),
                Some(app) => {
                    let runs: Vec<&super::model::CheckObservation> = named
                        .into_iter()
                        .filter(|c| c.kind == CheckKind::CheckRun)
                        .collect();
                    let unattributed = runs.iter().any(|c| c.app_id.is_none());
                    let its: Vec<_> = runs.into_iter().filter(|c| c.app_id == Some(app)).collect();
                    (its, unattributed)
                }
            };
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
            let state = if unattributed {
                worse(state, RequiredCheckState::Unknown)
            } else {
                state
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
/// Who wrote the body decides whether a declaration counts, so only what its author states
/// counts as one ([`stated_lines()`]): a quoted line is someone else's words, and a line in a
/// fenced code block or an HTML comment is an example or a template's hint. A dependency
/// keeps the lenient reading: it can only make a pull request wait.
///
/// ```text
/// use crate::integration::classify::declared_supersessions;
/// let s = declared_supersessions("Supersedes #12 and #14.\nsuperseded by #20 in spirit");
/// assert_eq!((s.supersedes, s.superseded_by), (vec![12, 14], vec![20]));
/// assert_eq!(declared_supersessions("this supersedes #3").supersedes, Vec::<u64>::new());
/// assert_eq!(declared_supersessions("> Supersedes #3").supersedes, Vec::<u64>::new());
/// ```text
pub fn declared_supersessions(body: &str) -> Supersessions {
    let stated = stated_lines(body);
    Supersessions {
        superseded_by: marked_numbers(&stated, SUPERSEDED_BY_MARKERS),
        supersedes: marked_numbers(&stated, SUPERSEDES_MARKERS),
    }
}

/// The lines of a body that are its author's own statement, each on its line: none that is
/// quoted (it opens with `>`), none inside a fenced code block (between two lines that open
/// with the same fence, three backticks or three tildes) and none inside an HTML comment
/// (after a line holding `<!--` and no `-->`, up to and with the next line holding `-->`; the
/// line that opens the comment is kept, for what it states before it). A fence or a comment
/// nobody closes takes the rest of the body with it, as it does where the body is rendered. A body pasted or prefilled from a commit message is still its author's: what
/// they submit under their name is what they state.
///
/// ```text
/// use crate::integration::classify::stated_lines;
/// assert_eq!(stated_lines("a\n> quoted\n```\nfenced\n```\nb"), "a\nb");
/// assert_eq!(stated_lines("a <!--\nhint\n-->\nb"), "a <!--\nb");
/// ```text
pub fn stated_lines(body: &str) -> String {
    let fence_of = |line: &str| ["```", "~~~"].into_iter().find(|f| line.starts_with(f));
    let mut fence: Option<&str> = None;
    let mut comment = false;
    let mut stated: Vec<&str> = Vec::new();
    for line in body.lines() {
        let opening = line.trim_start();
        let opens = fence_of(opening);
        if let Some(open) = fence {
            // only the fence that opened the block closes it
            fence = Some(open).filter(|open| !opening.starts_with(open));
        } else if comment {
            comment = !line.contains("-->");
        } else if opens.is_some() {
            fence = opens;
        } else if !opening.starts_with('>') {
            // a quoted line is someone else's words
            comment = line
                .rsplit_once("<!--")
                .is_some_and(|(_, after)| !after.contains("-->"));
            stated.push(line);
        }
    }
    stated.join("\n")
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
    /// (`Supersedes #N`). Authorised successors only ([`declaration_is_authorised()`]): these
    /// hold and close.
    pub superseded_by: BTreeMap<u64, Vec<Successor>>,
    /// The declarations nobody entitled made, by the number of the pull request each claims
    /// to replace: evidence, and nothing else.
    pub possible_supersessions: BTreeMap<u64, Vec<PossibleSupersession>>,
    /// The open pull requests whose cross-references were not read whole, each with how far
    /// the read got: each is held, whatever else is known of it.
    pub references_unread: BTreeMap<u64, CrossReferenceRead>,
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

/// The forge associations whose pull requests may declare a supersession.
pub const DECLARING_ASSOCIATIONS: &[&str] = &["OWNER", "MEMBER", "COLLABORATOR"];

/// Whether a pull request with this author association, from this repository or a fork,
/// may declare that one pull request replaces another: an owner, a member or a collaborator,
/// with a branch in this repository. The word is the forge's, matched exactly; an empty or
/// unknown one authorises nothing, and neither does anything from a fork.
///
/// The module is private, so the example is text; the unit tests run the same assertions.
///
/// ```text
/// use majordomus_cli::integration::classify::declaration_is_authorised;
/// assert!(declaration_is_authorised("COLLABORATOR", false));
/// assert!(!declaration_is_authorised("CONTRIBUTOR", false), "anyone can open a pull request");
/// assert!(!declaration_is_authorised("OWNER", true), "a fork declares nothing");
/// assert!(!declaration_is_authorised("", false), "unread is unauthorised");
/// ```
pub fn declaration_is_authorised(author_association: &str, cross_repository: bool) -> bool {
    !cross_repository && DECLARING_ASSOCIATIONS.contains(&author_association)
}

/// A declaration by someone the repository does not let declare one: evidence, never a
/// decision. It neither holds the pull request it names nor closes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PossibleSupersession {
    /// The pull request said to replace this one.
    pub successor: u64,
    /// Whose body said so: the replaced one's own, or the successor's.
    pub declared_in: u64,
    /// The declarer's login; empty when unread.
    pub declared_by: String,
    /// The declarer's association, verbatim; empty when unread.
    pub association: String,
    /// Whether the declarer's head lives in a fork.
    pub cross_repository: bool,
}

/// What the evidence says of a declaration nobody entitled made about `replaced`: who said
/// it, what they are to the repository, and what a declaration that counts looks like.
fn possible_supersession_detail(replaced: u64, said: &PossibleSupersession) -> String {
    let or = |text: &str, unread: &str| {
        if text.is_empty() {
            unread.to_string()
        } else {
            text.to_string()
        }
    };
    let who = or(&said.declared_by, "an unread author");
    let association = or(&said.association, "association unread");
    let fork = if said.cross_repository {
        ", from a fork"
    } else {
        ""
    };
    let n = said.successor;
    if said.declared_in == replaced {
        format!(
            "its body says superseded by #{n}, but its author {who} ({association}{fork}) is not \
             an owner, member or collaborator with a branch in this repository: it is neither \
             held nor closed by that. To replace it, one of them says `Supersedes #{replaced}` \
             in a pull request of this repository"
        )
    } else {
        format!(
            "#{n} by {who} ({association}{fork}) says it supersedes #{replaced}: not a \
             declaration this repository acts on, so #{replaced} is neither held nor closed by \
             it. To replace #{replaced}, an owner, member or collaborator says \
             `Supersedes #{replaced}` in a pull request of this repository"
        )
    }
}

/// One pull request declared to replace another, by someone the repository lets declare it.
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
    /// No longer open, and git finds its head or its merge commit in master: it landed,
    /// whatever the forge calls it.
    Landed {
        /// The commit git found in master: its head, or its merge commit.
        head_sha: String,
        /// Whether the forge calls it merged. It words the evidence and decides nothing.
        merged: bool,
        /// Whether that commit is its merge commit (a squash leaves no commit of its head on
        /// master), not its head.
        by_merge_commit: bool,
    },
    /// No longer open, and master contains neither its head nor its merge commit.
    /// `merged: false` is closed unmerged and releases the hold; `merged: true` is merged
    /// somewhere master does not contain.
    NotLanded {
        /// Whether the forge calls it merged.
        merged: bool,
    },
    /// No longer open, and the forge says it changes no file, or did not say how many: it
    /// brought nothing to master wherever its head now points — a branch reset onto master
    /// has its head there — so git is not asked and it never landed. `merged` as for
    /// [`SuccessorState::NotLanded`].
    Empty {
        /// Whether the forge calls it merged.
        merged: bool,
    },
    /// Closed unmerged with its head in a fork, or somewhere the forge did not name: whoever
    /// owns that fork can point the head at any commit of master before closing, so where it
    /// points proves nothing, git is not asked, and it releases its hold.
    ForkClosed,
    /// Open, declared in its own body, and nothing could be read of who its author is to the
    /// repository: the declaration may be one that counts, so the one it names is held until
    /// a refresh reads it.
    DeclarerUnread,
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
    // held on its own read state, whoever declared what: a declaration that was not read is
    // never taken for none
    let references_unread: Option<CrossReferenceRead> = queue
        .references_unread
        .get(&pr.number)
        .copied()
        .filter(|read| *read != CrossReferenceRead::Whole);
    evidence.extend(references_unread.map(|read| {
        let n = pr.number;
        if read == CrossReferenceRead::Truncated {
            ev(
                EvidenceKind::Supersession,
                "references_truncated",
                format!(
                    "more pull requests mention #{n} than were read ({}; a mention by an \
                     issue or from another repository counts toward that limit): one of them \
                     may say it supersedes #{n}, so #{n} is held until a person decides it",
                    super::forge::REFERENCE_PAGES * 100
                ),
                &forge,
            )
        } else {
            ev(
                EvidenceKind::Supersession,
                "references_unread",
                format!(
                    "the pull requests that mention #{n} were not read: one of them may say it \
                     supersedes #{n}, so #{n} is held"
                ),
                &forge,
            )
        }
    }));
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
            SuccessorState::Landed {
                head_sha,
                merged,
                by_merge_commit,
            } => ev(
                EvidenceKind::Supersession,
                "landed",
                format!(
                    "#{} ({declared}) is {}, and master contains its {} {}",
                    s.number,
                    if *merged { "merged" } else { "closed" },
                    if *by_merge_commit {
                        "merge commit"
                    } else {
                        "head"
                    },
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
                if *merged {
                    format!(
                        "#{} ({declared}) is merged, but master contains neither its head nor \
                         its merge commit: merged into another branch, or this clone's master \
                         is behind",
                        s.number
                    )
                } else {
                    format!(
                        "#{} ({declared}) was closed unmerged, and master contains neither its \
                         head nor a merge commit of it: it replaces nothing, and #{} is decided \
                         on its own",
                        s.number, pr.number
                    )
                },
                &forge,
            ),
            SuccessorState::Empty { merged } => ev(
                EvidenceKind::Supersession,
                "not_landed",
                format!(
                    "#{} ({declared}) is {}, and the forge names no file it changes: it \
                     brought nothing to master, wherever its head points, so it replaces \
                     nothing{}",
                    s.number,
                    if *merged { "merged" } else { "closed" },
                    if *merged {
                        String::new()
                    } else {
                        format!(", and #{} is decided on its own", pr.number)
                    }
                ),
                &forge,
            ),
            SuccessorState::ForkClosed => ev(
                EvidenceKind::Supersession,
                "not_landed",
                format!(
                    "#{} ({declared}) was closed unmerged and its head lives in a fork: where \
                     that head points proves nothing, so it replaces nothing, and #{} is \
                     decided on its own",
                    s.number, pr.number
                ),
                &forge,
            ),
            SuccessorState::DeclarerUnread => ev(
                EvidenceKind::Supersession,
                "unread",
                format!(
                    "#{} ({declared}) is open, and who its author is to the repository could \
                     not be read: the declaration may count, so #{} is held until it is read",
                    s.number, pr.number
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
    let possible: &[PossibleSupersession] = queue
        .possible_supersessions
        .get(&pr.number)
        .map_or(&[], Vec::as_slice);
    evidence.extend(possible.iter().map(|said| {
        ev(
            EvidenceKind::Supersession,
            "possible_supersession",
            possible_supersession_detail(pr.number, said),
            &forge,
        )
    }));
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
        (
            IntegrationGate::Supersession,
            supersession(successors, references_unread),
        ),
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
                // the requirement was read, and a check run of a context bound to an app was
                // not read with its writer: unknown, never waiting, since waiting attributes
                // nothing
                RequiredCheckState::Unknown if policy.required_checks.is_some() => fails(
                    PullRequestDisposition::Unknown,
                    vec![ReasonCode::RequiredChecks { state: checks }],
                    Some(UNREAD_WRITER_REMEDY.into()),
                ),
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

/// What the supersession gate answers for these authorised successors: nothing when there is
/// none and the pull requests that mention it were all read. A partial read of those
/// (`references`) decides before anything else: it is `unknown`, held, whatever is known of
/// its successors. Then, of several successors, the strongest decides — one that landed makes
/// it `superseded`, else one still open makes it wait for that one, else one merged somewhere
/// master does not contain leaves it to a person (`possibly_redundant`), else one that could
/// not be read, or an open declarer nothing was read of, makes it `unknown` — and every
/// successor gives its reason, the deciding kind first. A successor closed unmerged is in no
/// group, with its head on master or not when that head lives in a fork or it changes no
/// file: it releases its hold, and gives none.
fn supersession(
    successors: &[Successor],
    references: Option<CrossReferenceRead>,
) -> Option<Failure> {
    let of = |want: fn(&SuccessorState) -> bool| -> Vec<u64> {
        successors
            .iter()
            .filter(|s| want(&s.state))
            .map(|s| s.number)
            .collect()
    };
    let landed = of(|s| matches!(s, SuccessorState::Landed { .. }));
    let open = of(|s| matches!(s, SuccessorState::Open));
    let not_landed = of(|s| {
        matches!(
            s,
            SuccessorState::NotLanded { merged: true } | SuccessorState::Empty { merged: true }
        )
    });
    let unread = of(|s| matches!(s, SuccessorState::Unread | SuccessorState::DeclarerUnread));
    let mut reasons: Vec<ReasonCode> = Vec::new();
    reasons.extend(references.map(|_| ReasonCode::DeclarationsUnread));
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
    let (disposition, next) = if let Some(read) = references {
        let remedy = if read == CrossReferenceRead::Truncated {
            "a person decides it: more pull requests and issues mention it than are read, one \
             may supersede it, and no refresh clears that (docs/INTEGRATION.md, held by its \
             mentions)"
        } else {
            "majordomus prs refresh; the pull requests that mention it were not all read, and \
             one may supersede it"
        };
        (PullRequestDisposition::Unknown, remedy.to_string())
    } else if let Some(n) = landed.first() {
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
                "#{n}, which was to supersede it, is merged but brought nothing git finds on \
                 master: a person decides whether this one is still wanted, and removes the \
                 declaration if so"
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

    /// Under D3 this is the batch row: the forge calls it closed, git finds its head in
    /// master, and it landed.
    #[test]
    fn a_successor_closed_with_its_head_on_master_landed() {
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
                    by_merge_commit: false,
                },
            }],
        );
        let a = classify(&p, &up_to_date(), "m", "t", &policy(None, &[]), &ctx);
        assert_eq!(a.disposition, PullRequestDisposition::Superseded);
        assert!(
            a.evidence.iter().any(|e| e.detail.contains(
                "#2 (its body says superseded by #2) is closed, and master contains its head h2"
            )),
            "{:?}",
            a.evidence
        );
    }

    /// #1 with its required check passed and no review asked for, against `ctx`: ready
    /// unless the supersession gate says otherwise.
    fn decided(ctx: &QueueContext) -> PullRequestAssessment {
        let p = pr(serde_json::json!({
            "statusCheckRollup": [{"__typename": "CheckRun", "name": "ci", "status": "COMPLETED", "conclusion": "SUCCESS"}]
        }));
        let no_review = ReviewPolicy {
            approvals: 0,
            code_owners: false,
            dismiss_stale: false,
        };
        classify(
            &p,
            &up_to_date(),
            "m",
            "t",
            &policy(Some(no_review), &[]),
            ctx,
        )
    }

    /// A context in which #1 has these authorised successors.
    fn succeeded_by(states: Vec<(u64, SuccessorState)>) -> QueueContext {
        let mut ctx = context();
        ctx.superseded_by.insert(
            1,
            states
                .into_iter()
                .map(|(number, state)| Successor {
                    number,
                    declared_in: 1,
                    state,
                })
                .collect(),
        );
        ctx
    }

    fn supersession_evidence(a: &PullRequestAssessment) -> Vec<(&str, &str)> {
        a.evidence
            .iter()
            .filter(|e| e.kind == EvidenceKind::Supersession)
            .map(|e| (e.status.as_str(), e.detail.as_str()))
            .collect()
    }

    fn gate_passed(a: &PullRequestAssessment) -> bool {
        a.gates
            .iter()
            .any(|g| g.gate == IntegrationGate::Supersession && g.passed)
    }

    #[test]
    fn only_an_owner_member_or_collaborator_of_this_repository_declares() {
        for word in ["OWNER", "MEMBER", "COLLABORATOR"] {
            assert!(declaration_is_authorised(word, false), "{word}");
            assert!(!declaration_is_authorised(word, true), "{word} from a fork");
        }
        for word in [
            "CONTRIBUTOR",
            "FIRST_TIME_CONTRIBUTOR",
            "FIRST_TIMER",
            "MANNEQUIN",
            "NONE",
            "",
            "owner",
            " OWNER",
        ] {
            assert!(!declaration_is_authorised(word, false), "{word:?}");
            assert!(
                !declaration_is_authorised(word, true),
                "{word:?} from a fork"
            );
        }
        assert_eq!(DECLARING_ASSOCIATIONS.len(), 3);
    }

    #[test]
    fn a_successor_that_landed_by_its_merge_commit_says_so() {
        let ctx = succeeded_by(vec![(
            2,
            SuccessorState::Landed {
                head_sha: "squash2".into(),
                merged: true,
                by_merge_commit: true,
            },
        )]);
        let a = decided(&ctx);
        assert_eq!(a.disposition, PullRequestDisposition::Superseded);
        assert_eq!(a.superseded_by, Some(2));
        let landed = a
            .evidence
            .iter()
            .find(|e| e.status == "landed")
            .expect("landed");
        assert!(
            landed
                .detail
                .contains("is merged, and master contains its merge commit squash2"),
            "{}",
            landed.detail
        );
        assert_eq!(
            landed.source,
            Some(EvidenceSource::Git {
                master_sha: "m".into(),
                head_sha: "squash2".into()
            }),
            "the commit git found is the one named"
        );
    }

    #[test]
    fn a_successor_closed_unmerged_releases_the_hold() {
        let a = decided(&succeeded_by(vec![(
            2,
            SuccessorState::NotLanded { merged: false },
        )]));
        assert_eq!(
            a.disposition,
            PullRequestDisposition::Ready,
            "{:?}",
            a.reasons
        );
        assert!(gate_passed(&a), "{:?}", a.gates);
        assert!(
            !a.reasons
                .iter()
                .any(|r| r.code().starts_with("successor_") || r.code() == "superseded_by"),
            "{:?}",
            a.reasons
        );
        assert_eq!(a.superseded_by, None);
        let said = supersession_evidence(&a);
        assert_eq!(said.len(), 1, "{said:?}");
        assert_eq!(said[0].0, "not_landed");
        assert!(
            said[0]
                .1
                .contains("#2 (its body says superseded by #2) was closed unmerged")
                && said[0].1.contains("#1 is decided on its own"),
            "{}",
            said[0].1
        );
    }

    #[test]
    fn a_successor_merged_elsewhere_is_a_persons() {
        let a = decided(&succeeded_by(vec![(
            2,
            SuccessorState::NotLanded { merged: true },
        )]));
        assert_eq!(a.disposition, PullRequestDisposition::PossiblyRedundant);
        assert_eq!(a.reasons[0], "successor_not_landed:#2");
        assert!(!gate_passed(&a));
        assert!(
            a.next_action
                .as_deref()
                .is_some_and(|n| n
                    .contains("#2, which was to supersede it, is merged but brought nothing git")),
            "{:?}",
            a.next_action
        );
        let said = supersession_evidence(&a);
        assert_eq!(said[0].0, "not_landed");
        assert!(
            said[0]
                .1
                .contains("is merged, but master contains neither its head nor its merge commit"),
            "{}",
            said[0].1
        );
        assert_eq!(
            crate::integration::drain::cleanup_action(a.disposition),
            Some(crate::integration::drain::LEFT_FOR_A_PERSON)
        );
    }

    #[test]
    fn a_released_hold_does_not_release_anothers() {
        let released = (2, SuccessorState::NotLanded { merged: false });
        let a = decided(&succeeded_by(vec![
            released.clone(),
            (3, SuccessorState::Open),
        ]));
        assert_eq!(a.disposition, PullRequestDisposition::WaitingForDependency);
        assert_eq!(a.reasons[0], "successor_open:#3");
        assert!(
            !a.reasons.iter().any(|r| *r == "successor_not_landed:#2"),
            "{:?}",
            a.reasons
        );
        let a = decided(&succeeded_by(vec![released, (3, SuccessorState::Unread)]));
        assert_eq!(a.disposition, PullRequestDisposition::Unknown);
        assert_eq!(a.reasons[0], "successor_unread:#3");
    }

    #[test]
    fn an_unauthorised_declaration_is_evidence_and_nothing_else() {
        let said = |successor: u64, declared_in: u64, by: &str, association: &str, fork: bool| {
            PossibleSupersession {
                successor,
                declared_in,
                declared_by: by.into(),
                association: association.into(),
                cross_repository: fork,
            }
        };
        let mut ctx = context();
        ctx.possible_supersessions.insert(
            1,
            vec![
                said(2, 2, "mallory", "COLLABORATOR", true),
                said(3, 1, "a", "CONTRIBUTOR", false),
                said(4, 4, "", "NONE", false),
                said(5, 5, "eve", "", false),
            ],
        );
        // another pull request's possible supersessions are not this one's
        ctx.possible_supersessions
            .insert(9, vec![said(8, 8, "x", "NONE", false)]);
        let a = decided(&ctx);
        assert_eq!(
            a.disposition,
            PullRequestDisposition::Ready,
            "{:?}",
            a.reasons
        );
        assert!(gate_passed(&a));
        assert_eq!(a.superseded_by, None);
        let evidence = supersession_evidence(&a);
        assert_eq!(evidence.len(), 4, "{evidence:?}");
        assert!(evidence
            .iter()
            .all(|(status, _)| *status == "possible_supersession"));
        assert!(
            evidence[0].1.starts_with(
                "#2 by mallory (COLLABORATOR, from a fork) says it supersedes #1: not a \
                 declaration this repository acts on, so #1 is neither held nor closed by it."
            ),
            "{}",
            evidence[0].1
        );
        assert!(evidence[0]
            .1
            .contains("says `Supersedes #1` in a pull request of this repository"));
        assert!(
            evidence[1].1.starts_with(
                "its body says superseded by #3, but its author a (CONTRIBUTOR) is not an owner, \
                 member or collaborator with a branch in this repository: it is neither held nor \
                 closed by that."
            ),
            "{}",
            evidence[1].1
        );
        assert!(
            evidence[2]
                .1
                .starts_with("#4 by an unread author (NONE) says"),
            "{}",
            evidence[2].1
        );
        assert!(
            evidence[3]
                .1
                .starts_with("#5 by eve (association unread) says"),
            "{}",
            evidence[3].1
        );
        assert_eq!(
            crate::integration::drain::cleanup_action(a.disposition),
            None,
            "nothing closes it"
        );
    }

    #[test]
    fn references_not_read_whole_hold_whatever_else_is_known() {
        let landed = || {
            succeeded_by(vec![(
                2,
                SuccessorState::Landed {
                    head_sha: "h2".into(),
                    merged: true,
                    by_merge_commit: false,
                },
            )])
        };
        let mut ctx = landed();
        ctx.references_unread
            .insert(1, CrossReferenceRead::Truncated);
        let a = decided(&ctx);
        assert_eq!(a.disposition, PullRequestDisposition::Unknown);
        assert_eq!(a.reasons[0], "declarations_unread");
        assert!(
            a.reasons.iter().any(|r| *r == "superseded_by:#2"),
            "what is known is still said: {:?}",
            a.reasons
        );
        assert_eq!(a.superseded_by, None, "never closable on a partial read");
        assert!(!gate_passed(&a));
        assert!(
            a.next_action.as_deref().is_some_and(|n| n
                .starts_with("a person decides it: more pull requests and issues")
                && n.contains("no refresh clears that")),
            "{:?}",
            a.next_action
        );
        let said = supersession_evidence(&a);
        assert_eq!(
            said[0].0, "references_truncated",
            "the read state comes first"
        );
        assert!(
            said[0]
                .1
                .contains("more pull requests mention #1 than were read (5000; a mention by")
                && said[0].1.contains("#1 is held"),
            "{}",
            said[0].1
        );
        assert_eq!(said[1].0, "landed");
        assert_eq!(
            crate::integration::drain::cleanup_action(a.disposition),
            None
        );

        let mut ctx = context();
        ctx.references_unread.insert(1, CrossReferenceRead::Unread);
        let a = decided(&ctx);
        assert_eq!(a.disposition, PullRequestDisposition::Unknown);
        assert_eq!(a.reasons[0], "declarations_unread");
        let said = supersession_evidence(&a);
        assert_eq!(said.len(), 1);
        assert_eq!(said[0].0, "references_unread");
        assert!(
            said[0]
                .1
                .contains("the pull requests that mention #1 were not read"),
            "{}",
            said[0].1
        );

        // read whole, or another pull request's read: nothing is held
        let mut ctx = landed();
        ctx.references_unread.insert(1, CrossReferenceRead::Whole);
        ctx.references_unread.insert(7, CrossReferenceRead::Unread);
        let a = decided(&ctx);
        assert_eq!(a.disposition, PullRequestDisposition::Superseded);
        assert_eq!(a.superseded_by, Some(2));
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
