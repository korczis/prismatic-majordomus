//! `prs compose`: build one pull request out of several that are each proved on their own
//! head, on a person's request (ADR 0114).
//!
//! # Why it exists
//!
//! One pull request lands per CI run of its refreshed head, and every landing puts every other
//! open pull request behind master again. A batch buys that back: several members' heads are
//! merged, in order, onto the current master, and the result is one more pull request, which
//! the executor classifies, ranks, repairs, merges and verifies like any other — the
//! one-merge-at-a-time invariant of ADR 0101 is untouched, and several members' work lands in
//! that one merge. Batches were built by hand before this module; what the hand-built form
//! lacked is what it adds: a rule for who may ride, a record of who did, and a branch whose
//! shape lets a bisection land on one member.
//!
//! # What decides
//!
//! Nothing here classifies. [`decide`] reads the assessments the queue built — every gate of
//! the policy answers whatever the others said ([`super::IntegrationGate`]), so "its required
//! checks passed on its own head" is the `required_checks` and `no_failing_check` gates, read
//! apart from the `freshness` gate that says whether the head contains master — and maps each
//! pull request onto one of two answers:
//!
//! - a member: open, on the integration base, not a draft, no holding label, its head in this
//!   repository, no auto-merge armed, no declared successor, a relation to master that is
//!   `up_to_date` or `behind` (a member need not be refreshed and run alone again: that is the
//!   throughput a batch buys), the review policy satisfied, every required check passed on
//!   the head, and every declared dependency landed or a member placed before it;
//! - left out, with the one typed reason that decided it ([`LeftOutReason`]).
//!
//! Members are taken in the planner's rank order ([`super::rank`]), a dependent after what it
//! waits for, up to the `max_members` the caller names. Fewer than two is not a batch
//! ([`ComposeRefusal::TooFew`]): one pull request is `prs repair`.
//!
//! # What it reads, what it does, and what it never does
//!
//! The dry run is the default and it is a read: [`plan`] decides from the queue
//! [`super::queue_of`] builds out of the last recorded observation, so it reaches no network,
//! takes no lease and writes nothing to the trail. [`apply`] is the act, and it follows the
//! drain's rules: the caller holds the base branch's integration lease for the whole of it; it
//! observes the forge afresh ([`Integrator::observe`]) and decides again on that, so a plan a
//! person read a moment ago is never what is composed; `compose_selected` and then
//! `compose_attempted` are on the trail before anything reaches the remote, and `composed` or
//! `compose_refused` after. The act is the executor's own ([`Integrator::compose_branch`]): in
//! a scratch worktree, one `--no-ff` merge per member onto the decided master, one `release
//! bump` to what the public contract of the composed tree requires (D6), the manifest
//! ([`BatchManifest`]), a fresh derive, one composition commit, a push as a *new* branch
//! `int/batch-<id>` and a pull request whose body supersedes each member
//! ([`pull_request_body`]). It never pushes to a branch that exists, never merges anything
//! into master, and never touches a person's checkout.
//!
//! # The rollout
//!
//! The act is refused until the trail holds a verified merge since the last one that could
//! not be verified ([`rollout_refused`], ADR 0114 D7): composing is not the first thing an
//! executor does to a repository. There is no continuous composition. It is refused as well
//! while the layer does not hold ADR 0114 as accepted ([`decision_refused`]): accepting a
//! decision is a person's act, and until it is taken only the dry run answers.
//!
//! # The size
//!
//! How many members a batch may carry is the policy's `integration.batch.max_members`
//! ([`max_members`]); `--max` lowers it for one composition. The code has no default.

use std::collections::BTreeSet;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::drain::{
    event, events, record, verified_merges_since_failure, FailureClass, IntegrationAction,
    IntegrationEvent, Integrator,
};
use super::{
    DependencyCertainty, IntegrationGate, IntegrationQueue, PullRequestAssessment,
    PullRequestReview, ReasonCode, RelationToMaster, RequiredCheckState,
};

/// The schema word a batch manifest declares.
pub const BATCH_SCHEMA: &str = "integration-batch/v1";

/// What a batch's head branch is called before its id.
pub const BATCH_BRANCH_PREFIX: &str = "int/batch-";

/// Where a batch's manifest is tracked, on the batch's own branch.
pub const BATCH_DIR: &str = ".ai/repo/integration/batches";

/// The fewest members a batch has: one pull request is `prs repair`, not a batch.
pub const MIN_MEMBERS: usize = 2;

/// How many verified merges the trail must hold, since the last merge that could not be
/// verified, before a batch may be composed (ADR 0114 D7).
///
/// ADR 0101 §13 stages the rollout — a dry run, one bounded merge, small bounded drains,
/// continuous mode — and gives a number only to the last
/// ([`super::drain::ROLLOUT_MERGES_BEFORE_CONTINUOUS`]). Composition is asked for, never
/// continuous, so it is unlocked by the first stage that acts: one bounded merge, proved
/// where it landed.
pub const ROLLOUT_MERGES_BEFORE_COMPOSE: usize = 1;

/// The policy key that says how many members a batch may carry. There is no default in code.
pub const MAX_MEMBERS_KEY: &str = "integration.batch.max_members";

/// The decision that makes composition a capability of the subsystem: `--apply` is refused
/// until this layer holds it as accepted.
pub const DECISION: &str = "adr-0114";

/// The status a decision must have for `--apply` to act.
pub const DECISION_ACCEPTED: &str = "accepted";

/// `integration.batch.max_members` of the policy of the repository at `root`: `None` when the
/// key is absent, or when `root` holds no `.ai/` layer at all — a repository that declares no
/// policy declares no cap. `Err` when there is a policy and it cannot be read: an unreadable
/// policy is never "no key".
pub fn policy_max(root: &Path) -> Result<Option<usize>, String> {
    if !root.join(crate::repository::MANIFEST).is_file() {
        return Ok(None);
    }
    let repository = crate::repository::Repository::open(root).map_err(|e| e.to_string())?;
    crate::policy::LoadedPolicy::load(&repository)
        .map(|loaded| loaded.policy.integration.batch.max_members)
        .map_err(|e| e.to_string())
}

/// How many members a batch may carry: the policy's cap, lowered by `--max` when one is
/// given. `--max` above the cap is refused, never clamped — a person who asked for nine and
/// got eight was not answered. With no cap declared, `--max` alone is the size; with neither,
/// nothing decides and the refusal names the key.
///
/// ```text
/// use crate::integration::compose::{resolve_max, ComposeRefusal};
/// assert_eq!(resolve_max(Some(8), None), Ok(8));
/// assert_eq!(resolve_max(Some(8), Some(3)), Ok(3));
/// assert_eq!(resolve_max(None, Some(3)), Ok(3));
/// assert_eq!(resolve_max(None, None), Err(ComposeRefusal::NoMaxMembers));
/// assert!(matches!(resolve_max(Some(8), Some(9)), Err(ComposeRefusal::MaxAbovePolicy { .. })));
/// ```
pub fn resolve_max(policy: Option<usize>, asked: Option<usize>) -> Result<usize, ComposeRefusal> {
    match (policy, asked) {
        (None, None) => Err(ComposeRefusal::NoMaxMembers),
        (Some(policy), Some(max)) if max > policy => {
            Err(ComposeRefusal::MaxAbovePolicy { max, policy })
        }
        (_, Some(max)) | (Some(max), None) => Ok(max),
    }
}

/// [`resolve_max`] for the repository at `root` ([`policy_max`]).
pub fn max_members(root: &Path, asked: Option<usize>) -> Result<usize, ComposeRefusal> {
    let policy = policy_max(root).map_err(|reason| ComposeRefusal::ActFailed {
        reason: format!("the policy could not be read for {MAX_MEMBERS_KEY}: {reason}"),
        class: FailureClass::Unreadable,
    })?;
    resolve_max(policy, asked)
}

/// The status the layer at `root` gives the decision [`DECISION`], with the file that states
/// it: read from the front matter of the record under the manifest's `adrs` section whose
/// `id` is the decision's, with the layer's own front-matter reader
/// ([`crate::metadata::frontmatter`]) — the reader the index is built with, asked for one
/// file, because `prs` never loads the index and must not need the whole layer valid to
/// refuse. `Ok(None)` when the layer holds no such record, or `root` holds no layer.
pub fn decision_status(root: &Path) -> Result<Option<(String, String)>, String> {
    if !root.join(crate::repository::MANIFEST).is_file() {
        return Ok(None);
    }
    let repository = crate::repository::Repository::open(root).map_err(|e| e.to_string())?;
    let Some(section) = repository.section_path("adrs") else {
        return Ok(None);
    };
    let Ok(entries) = std::fs::read_dir(root.join(&section)) else {
        return Ok(None);
    };
    // by name, whatever order the directory is listed in
    let files: BTreeSet<std::path::PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .collect();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Some(front) = crate::metadata::frontmatter::split(&text)
            .ok()
            .and_then(|s| s.front)
        else {
            continue;
        };
        // a record whose front matter does not parse is some other record's defect unless it
        // is this one, and then its status is unknown, which is not accepted
        let Ok(meta) = crate::metadata::frontmatter::parse(front) else {
            continue;
        };
        if meta.get("id").and_then(|v| v.as_str()) != Some(DECISION) {
            continue;
        }
        let name = file
            .file_name()
            .map(|n| format!("{section}/{}", n.to_string_lossy()))
            .unwrap_or_default();
        let status = meta
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unstated")
            .to_string();
        return Ok(Some((status, name)));
    }
    Ok(None)
}

/// Why `--apply` may not act in the repository at `root`, or nothing when the layer holds the
/// decision as accepted. In addition to the rollout record ([`rollout_refused`]), and asked
/// before it. Nothing here can accept the decision: that is a person's act.
pub fn decision_refused(root: &Path) -> Option<ComposeRefusal> {
    let status = match decision_status(root) {
        Ok(Some((status, _))) if status == DECISION_ACCEPTED => return None,
        Ok(Some((status, file))) => format!("its status is `{status}` ({file})"),
        Ok(None) => "this repository's layer holds no such decision".to_string(),
        Err(reason) => format!("its status could not be read: {reason}"),
    };
    Some(ComposeRefusal::DecisionNotAccepted { status })
}

/// The manifest a batch's head carries, read with git: `text_at(path)` gives the file at the
/// head. A batch's branch names its manifest ([`branch_of`], [`manifest_path`]).
fn manifest_named(
    head_ref: &str,
    text_at: impl Fn(&str) -> Option<String>,
) -> Option<BatchManifest> {
    let id = head_ref.strip_prefix(BATCH_BRANCH_PREFIX)?;
    BatchManifest::parse(&text_at(&manifest_path(id))?).ok()
}

/// The manifest the open pull request `a` carries on its head in the clone at `root`, when it
/// is a batch: the one its branch `int/batch-<id>` names, or — for a branch named otherwise —
/// the one manifest its head adds to `master`. `None` when it carries none that reads.
pub fn manifest_on(root: &Path, master: &str, a: &PullRequestAssessment) -> Option<BatchManifest> {
    let head = a.evaluated_against.head_sha.as_str();
    let git = |args: &[&str]| -> Option<String> {
        std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .arg("--no-pager")
            .args(args)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let text_at = |path: &str| git(&["show", &format!("{head}:{path}")]);
    manifest_named(&a.head_ref, text_at).or_else(|| {
        let added = git(&[
            "diff",
            "--name-only",
            "--diff-filter=AM",
            &format!("{master}...{head}"),
            "--",
            BATCH_DIR,
        ])?;
        let mut manifests = added.lines().filter(|p| p.ends_with(".yaml"));
        match (manifests.next(), manifests.next()) {
            (Some(path), None) => BatchManifest::parse(&text_at(path)?).ok(),
            _ => None,
        }
    })
}

/// Say, on every assessment of `queue`, what a batch has to do with it: an open batch carries
/// its manifest ([`PullRequestAssessment::batch`]) and each open member names the batch that
/// carries it ([`PullRequestAssessment::carried_by`]). The one place either is set.
/// `manifest_of` reads a candidate's manifest ([`manifest_on`]); it is asked only about the
/// pull requests that can be batches ([`batches`]), so a queue with none costs nothing. A
/// member two batches name is carried by the one ranked first. Neither field decides a
/// disposition: the supersession gate already holds a member while its batch is open.
pub fn carry(
    queue: &mut IntegrationQueue,
    manifest_of: impl Fn(&PullRequestAssessment) -> Option<BatchManifest>,
) {
    let candidates = batches(queue);
    let mut carried: std::collections::BTreeMap<u64, u64> = std::collections::BTreeMap::new();
    for a in queue
        .assessments
        .iter_mut()
        .filter(|a| candidates.contains(&a.number))
    {
        a.batch = manifest_of(a);
        for n in a.batch.iter().flat_map(BatchManifest::numbers) {
            carried.entry(n).or_insert(a.number);
        }
    }
    for a in &mut queue.assessments {
        a.carried_by = carried.get(&a.number).copied().filter(|b| *b != a.number);
    }
}

/// The first ten characters of a commit id: what a batch's id and a person's line carry.
///
/// ```text
/// use crate::integration::compose::short;
/// assert_eq!(short("4090b2b8ae17c0ffee"), "4090b2b8ae");
/// assert_eq!(short("m1"), "m1");
/// ```
pub fn short(sha: &str) -> &str {
    &sha[..sha.len().min(10)]
}

/// A batch's id: the master it was composed on, short, then its members' numbers in
/// composition order. Derived from what the batch is, never chosen: the same members on the
/// same master are the same batch, so composing them twice names one branch and the second
/// push is refused.
///
/// ```text
/// use crate::integration::compose::batch_id;
/// assert_eq!(batch_id("4090b2b8ae17c0ffee", &[806, 815]), "4090b2b8ae-806-815");
/// ```
pub fn batch_id(master_sha: &str, members: &[u64]) -> String {
    members
        .iter()
        .fold(short(master_sha).to_string(), |id, n| format!("{id}-{n}"))
}

/// The head branch of the batch `id`.
pub fn branch_of(id: &str) -> String {
    format!("{BATCH_BRANCH_PREFIX}{id}")
}

/// The repository path of the batch `id`'s manifest.
pub fn manifest_path(id: &str) -> String {
    format!("{BATCH_DIR}/{id}.yaml")
}

/// One member of a planned batch: a pull request at the head it was decided on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BatchMember {
    /// The pull request.
    pub number: u64,
    /// The head commit its eligibility was decided on, and the one that is merged.
    pub head: String,
    /// Its title, as observed.
    pub title: String,
}

/// Why a pull request is not a member of the batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum LeftOutReason {
    /// It targets another branch than the integration base — another pull request's head
    /// (it is stacked) or a branch of its own.
    OtherBase {
        /// The branch it targets.
        base: String,
    },
    /// It is itself a batch: its head branch is a batch's, or it is the declared successor of
    /// two or more open pull requests. A batch is one candidate, never a member.
    IsBatch,
    /// It is a draft.
    Draft,
    /// A label whose policy is to hold, or to call it obsolete, is on it.
    Label {
        /// The labels, as the forge spells them.
        names: Vec<String>,
    },
    /// The forge has auto-merge armed on it and would merge it on its own.
    AutoMergeArmed,
    /// Its head lives in a fork, not in this repository.
    ForkHead,
    /// An open pull request — a batch, or any declared successor — already names it as what
    /// it supersedes.
    NamedByOpen {
        /// The open pull request that names it.
        number: u64,
    },
    /// Its supersession is otherwise undecided or decided against it: a successor landed, one
    /// could not be read, or the pull requests that mention it were not all read.
    Supersession {
        /// What the classifier said.
        reasons: Vec<ReasonCode>,
    },
    /// Merging it into master conflicts on authored paths: the owner's to settle first.
    AuthoredConflict {
        /// The conflicting authored paths.
        paths: Vec<String>,
    },
    /// Its work is on master already, or only derived files would change: cleanup's lane.
    NothingToLand {
        /// What git decided the head is to master.
        relation: String,
    },
    /// Git could not decide what the head is to master.
    Undecidable {
        /// Why.
        reason: String,
    },
    /// The repository allows no merge commit, the only way a batch — or anything — lands.
    MergeCommitNotAllowed,
    /// The review policy is not satisfied on its head.
    Review {
        /// The review state.
        state: PullRequestReview,
    },
    /// Its required checks have not all passed on its head: failed, pending, missing,
    /// skipped without permission, or unread.
    RequiredChecks {
        /// Their verdict.
        state: RequiredCheckState,
    },
    /// A dependency it declares has not landed and is not a member placed before it.
    Dependency {
        /// The dependencies it still waits for.
        waits_for: Vec<u64>,
    },
    /// It is eligible, and the batch already holds as many members as were asked for.
    OverMax {
        /// The most members the batch may have.
        max: usize,
    },
    /// During the composition: its merge onto the members before it conflicts on authored
    /// paths. It was dropped and the composition went on without it.
    ConflictsInBatch {
        /// The conflicting authored paths.
        paths: Vec<String>,
    },
    /// During the composition: its head was already contained in what was composed before
    /// it, so its merge would be no merge commit and it would carry nothing.
    AlreadyInBatch,
}

impl std::fmt::Display for LeftOutReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let numbers = |ns: &[u64]| {
            ns.iter()
                .map(|n| format!("#{n}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        match self {
            LeftOutReason::OtherBase { base } => {
                write!(f, "it targets {base}, not the integration base")
            }
            LeftOutReason::IsBatch => {
                f.write_str("it is itself a batch: one candidate, never a member")
            }
            LeftOutReason::Draft => f.write_str("it is a draft"),
            LeftOutReason::Label { names } => {
                write!(f, "it carries a holding label: {}", names.join(", "))
            }
            LeftOutReason::AutoMergeArmed => f.write_str(
                "auto-merge is armed on it: the forge would merge it on its own; a person disarms it first",
            ),
            LeftOutReason::ForkHead => {
                f.write_str("its head lives in a fork, not in this repository")
            }
            LeftOutReason::NamedByOpen { number } => write!(
                f,
                "open pull request #{number} already says it supersedes it"
            ),
            LeftOutReason::Supersession { reasons } => write!(
                f,
                "its supersession is not settled: {}",
                super::reason_list(reasons, ", ")
            ),
            LeftOutReason::AuthoredConflict { paths } => write!(
                f,
                "merging it into master conflicts on {} authored file(s), which are the owner's to settle: {}",
                paths.len(),
                paths.join(", ")
            ),
            LeftOutReason::NothingToLand { relation } => write!(
                f,
                "it has nothing to land ({relation}); cleanup is the lane for it"
            ),
            LeftOutReason::Undecidable { reason } => {
                write!(f, "git could not decide its relation to master: {reason}")
            }
            LeftOutReason::MergeCommitNotAllowed => {
                f.write_str("the repository allows no merge commit, so nothing can land")
            }
            LeftOutReason::Review { state } => write!(
                f,
                "the review policy is not satisfied on its head ({})",
                super::classify::word(state)
            ),
            LeftOutReason::RequiredChecks { state } => write!(
                f,
                "its required checks have not passed on its head ({})",
                super::classify::word(state)
            ),
            LeftOutReason::Dependency { waits_for } => write!(
                f,
                "it waits for {}, which has not landed and is not a member before it",
                numbers(waits_for)
            ),
            LeftOutReason::OverMax { max } => {
                write!(f, "eligible, and the batch already holds its {max} member(s)")
            }
            LeftOutReason::ConflictsInBatch { paths } => write!(
                f,
                "its merge onto the members before it conflicts on {} authored file(s): {}",
                paths.len(),
                paths.join(", ")
            ),
            LeftOutReason::AlreadyInBatch => {
                f.write_str("its head is already contained in the members before it")
            }
        }
    }
}

/// One pull request the batch does not carry, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LeftOut {
    /// The pull request.
    pub number: u64,
    /// The head the decision was taken on.
    pub head: String,
    /// Its title, as observed.
    pub title: String,
    /// Why it is not a member.
    pub reason: LeftOutReason,
}

impl LeftOut {
    /// `member`, left out of the composition after all, for `reason`.
    pub fn of(member: &BatchMember, reason: LeftOutReason) -> Self {
        LeftOut {
            number: member.number,
            head: member.head.clone(),
            title: member.title.clone(),
            reason,
        }
    }
}

/// The plan of one batch: who would be merged onto which master, in what order, and who is
/// left out. Deterministic: the same queue, in whatever order its pull requests were listed,
/// gives the same plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BatchPlan {
    /// The integration base.
    pub base: String,
    /// The master commit every member was decided against, and the one the batch starts from.
    pub master_sha: String,
    /// When the forge was observed, for the observation the plan rests on.
    pub observed_at: String,
    /// The most members that were asked for.
    pub max_members: usize,
    /// The members, in composition order.
    pub members: Vec<BatchMember>,
    /// Every other open pull request, in rank order, with why it is not a member.
    pub left_out: Vec<LeftOut>,
}

impl BatchPlan {
    /// The members' numbers, in composition order: what a test compares a plan by.
    #[cfg(test)]
    pub fn numbers(&self) -> Vec<u64> {
        self.members.iter().map(|m| m.number).collect()
    }
}

/// Whether `gate` passed for `a`. A gate the assessment does not carry did not pass: an
/// assessment nobody asked the question of proves nothing.
fn passed(a: &PullRequestAssessment, gate: IntegrationGate) -> bool {
    a.gates.iter().any(|g| g.gate == gate && g.passed)
}

/// The open pull requests that are batches: a head branch under [`BATCH_BRANCH_PREFIX`], or
/// the declared open successor of two or more open pull requests (one is a replacement, not
/// a composition).
fn batches(queue: &IntegrationQueue) -> BTreeSet<u64> {
    let mut named: std::collections::BTreeMap<u64, usize> = std::collections::BTreeMap::new();
    for a in &queue.assessments {
        for r in &a.reasons {
            if let ReasonCode::SuccessorOpen { number } = r {
                *named.entry(*number).or_default() += 1;
            }
        }
    }
    queue
        .assessments
        .iter()
        .filter(|a| {
            a.head_ref.starts_with(BATCH_BRANCH_PREFIX)
                || named.get(&a.number).is_some_and(|n| *n >= MIN_MEMBERS)
        })
        .map(|a| a.number)
        .collect()
}

/// What one assessment says about riding a batch, apart from the batch around it: the open
/// dependencies it would have to be placed after, or the reason it cannot ride at all.
fn standing(
    a: &PullRequestAssessment,
    queue: &IntegrationQueue,
    batches: &BTreeSet<u64>,
) -> Result<Vec<u64>, LeftOutReason> {
    if a.base_ref != queue.base || !passed(a, IntegrationGate::Base) {
        return Err(LeftOutReason::OtherBase {
            base: a.base_ref.clone(),
        });
    }
    if batches.contains(&a.number) {
        return Err(LeftOutReason::IsBatch);
    }
    if !passed(a, IntegrationGate::Draft) {
        return Err(LeftOutReason::Draft);
    }
    if !passed(a, IntegrationGate::Label) {
        return Err(LeftOutReason::Label {
            names: a
                .reasons
                .iter()
                .filter_map(|r| match r {
                    ReasonCode::Label { name } | ReasonCode::LabelObsolete { name } => {
                        Some(name.clone())
                    }
                    _ => None,
                })
                .collect(),
        });
    }
    if !passed(a, IntegrationGate::AutoMerge) {
        return Err(LeftOutReason::AutoMergeArmed);
    }
    if a.fork_head {
        return Err(LeftOutReason::ForkHead);
    }
    if !passed(a, IntegrationGate::Supersession) {
        let open = a.reasons.iter().find_map(|r| match r {
            ReasonCode::SuccessorOpen { number } => Some(*number),
            _ => None,
        });
        return Err(match open {
            Some(number) => LeftOutReason::NamedByOpen { number },
            None => LeftOutReason::Supersession {
                reasons: a
                    .reasons
                    .iter()
                    .filter(|r| {
                        matches!(
                            r,
                            ReasonCode::SupersededBy { .. }
                                | ReasonCode::SuccessorNotLanded { .. }
                                | ReasonCode::SuccessorUnread { .. }
                                | ReasonCode::DeclarationsUnread
                        )
                    })
                    .cloned()
                    .collect(),
            },
        });
    }
    match &a.relation {
        // behind is what a batch is for; up to date rides as well
        RelationToMaster::Behind { .. } | RelationToMaster::UpToDate { .. }
            if passed(a, IntegrationGate::RelationToMaster) => {}
        RelationToMaster::Conflicting { paths } => {
            return Err(LeftOutReason::AuthoredConflict {
                paths: paths.clone(),
            })
        }
        RelationToMaster::Unknown { reason } => {
            return Err(LeftOutReason::Undecidable {
                reason: reason.clone(),
            })
        }
        other => {
            return Err(LeftOutReason::NothingToLand {
                relation: super::classify::relation_word(other).to_string(),
            })
        }
    }
    if !passed(a, IntegrationGate::MergeMethod) {
        return Err(LeftOutReason::MergeCommitNotAllowed);
    }
    if !passed(a, IntegrationGate::Review) {
        return Err(LeftOutReason::Review { state: a.review });
    }
    // on its own head, whatever that head is to master: the freshness gate is not asked
    if !passed(a, IntegrationGate::NoFailingCheck) || !passed(a, IntegrationGate::RequiredChecks) {
        return Err(LeftOutReason::RequiredChecks {
            state: a.required_checks,
        });
    }
    let unmet: Vec<_> = a
        .dependencies
        .iter()
        .filter(|d| d.certainty == DependencyCertainty::Confirmed && !d.satisfied)
        .collect();
    // closed unmerged, or unread: no place in the batch can satisfy it
    let hopeless: Vec<u64> = unmet
        .iter()
        .filter(|d| d.state != super::DependencyState::Open)
        .map(|d| d.number)
        .collect();
    if !hopeless.is_empty() {
        return Err(LeftOutReason::Dependency {
            waits_for: hopeless,
        });
    }
    if unmet.is_empty() && !passed(a, IntegrationGate::Dependency) {
        // the gate failed on something the dependencies do not name: what it said stands
        return Err(LeftOutReason::Dependency {
            waits_for: a
                .reasons
                .iter()
                .filter_map(|r| match r {
                    ReasonCode::DependsOn { number }
                    | ReasonCode::DependencyCycle { number }
                    | ReasonCode::DependencyClosedUnmerged { number }
                    | ReasonCode::DependencyUnread { number } => Some(*number),
                    _ => None,
                })
                .collect(),
        });
    }
    Ok(unmet.iter().map(|d| d.number).collect())
}

/// Decide the batch from the queue alone: the gates, the relation and the reasons the
/// classifier gave. Pure: it reads no file, runs no git and asks no forge.
///
/// The pull requests are ranked again ([`super::rank`]) rather than taken in the order the
/// queue lists them, so the plan does not depend on that order. A member is the first, in
/// rank order, whose open dependencies are all members already; that repeats until
/// `max_members` are placed or nobody else can be.
///
/// ```text
/// use crate::integration::compose::{decide, LeftOutReason};
/// // #1 and #2 behind master with a passed check, #3 a draft
/// let plan = decide(&queue, 5);
/// assert_eq!(plan.numbers(), [1, 2]);
/// assert_eq!(plan.left_out[0].reason, LeftOutReason::Draft);
/// ```
pub fn decide(queue: &IntegrationQueue, max_members: usize) -> BatchPlan {
    let batches = batches(queue);
    let ranked = super::rank(queue.assessments.clone());
    let member_of = |a: &PullRequestAssessment| BatchMember {
        number: a.number,
        head: a.evaluated_against.head_sha.clone(),
        title: a.title.clone(),
    };
    let mut reasons: std::collections::BTreeMap<u64, LeftOutReason> =
        std::collections::BTreeMap::new();
    // the eligible, in rank order, each with the open dependencies it must follow
    let mut waiting: Vec<(&PullRequestAssessment, Vec<u64>)> = Vec::new();
    for a in &ranked {
        match standing(a, queue, &batches) {
            Ok(after) => waiting.push((a, after)),
            Err(reason) => {
                reasons.insert(a.number, reason);
            }
        }
    }
    let mut members: Vec<BatchMember> = Vec::new();
    let mut placed: BTreeSet<u64> = BTreeSet::new();
    while members.len() < max_members {
        let Some(next) = waiting
            .iter()
            .position(|(_, after)| after.iter().all(|n| placed.contains(n)))
        else {
            break;
        };
        let (a, _) = waiting.remove(next);
        placed.insert(a.number);
        members.push(member_of(a));
    }
    for (a, after) in waiting {
        let waits_for: Vec<u64> = after.into_iter().filter(|n| !placed.contains(n)).collect();
        let reason = if waits_for.is_empty() {
            LeftOutReason::OverMax { max: max_members }
        } else {
            LeftOutReason::Dependency { waits_for }
        };
        reasons.insert(a.number, reason);
    }
    BatchPlan {
        base: queue.base.clone(),
        master_sha: queue.master_sha.clone(),
        observed_at: queue.observed_at.clone(),
        max_members,
        members,
        left_out: ranked
            .iter()
            .filter_map(|a| {
                reasons.remove(&a.number).map(|reason| LeftOut {
                    number: a.number,
                    head: a.evaluated_against.head_sha.clone(),
                    title: a.title.clone(),
                    reason,
                })
            })
            .collect(),
    }
}

/// A string as a YAML scalar the layer's reader gives back unchanged: bare when it opens with
/// a letter and holds only letters, digits and `. _ / -` — never a word or a number the
/// reader would type — and single-quoted otherwise, a quote inside it doubled.
///
/// ```text
/// use crate::integration::compose::yaml_scalar;
/// assert_eq!(yaml_scalar("int/batch-1"), "int/batch-1");
/// assert_eq!(yaml_scalar("1234"), "'1234'");
/// assert_eq!(yaml_scalar("it's \"x\": y"), "'it''s \"x\": y'");
/// ```
pub fn yaml_scalar(s: &str) -> String {
    let bare = s.starts_with(|c: char| c.is_ascii_alphabetic())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '/' | '-'))
        && !matches!(
            s.to_ascii_lowercase().as_str(),
            "true" | "false" | "null" | "yes" | "no" | "on" | "off"
        );
    if bare {
        s.to_string()
    } else {
        yaml_quoted(s)
    }
}

/// A string as a single-quoted YAML scalar, a quote inside it doubled: how a manifest writes
/// every commit id, moment, id and title, so that one that happens to open with a digit is
/// written like one that does not.
fn yaml_quoted(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// One member as the batch carries it: the planned member and the merge commit that brought
/// its head in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ComposedMember {
    /// The pull request.
    pub number: u64,
    /// The head that was merged: the merge commit's second parent.
    pub head: String,
    /// Its title, as observed.
    pub title: String,
    /// The merge commit on the batch's first-parent line that carries it.
    pub merge_commit: String,
}

/// A batch's manifest, of kind `integration-batch/v1`: the batch's identity, tracked on its
/// branch at [`manifest_path`] and landing with it. A regression found after a batch landed
/// is located by `git bisect --first-parent` over the batch's range, and this names the
/// member each merge commit carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BatchManifest {
    /// [`BATCH_SCHEMA`].
    pub schema: String,
    /// The batch's id ([`batch_id`]).
    pub id: String,
    /// The integration base.
    pub base: String,
    /// The master commit the batch was composed on.
    pub base_master: String,
    /// When it was composed, RFC 3339.
    pub composed_at: String,
    /// The version the composed tree declared after the member merges and before the one
    /// `release bump` of the composition (ADR 0114 D6). Absent when the tree declares none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_before: Option<String>,
    /// The version the composition commit declares: equal to `version_before` when the
    /// public contract of the composed tree required nothing above it and no bump was
    /// written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_after: Option<String>,
    /// The members, in composition order.
    pub members: Vec<ComposedMember>,
}

impl BatchManifest {
    /// The manifest of `members`, composed in that order on `base_master` at `composed_at`.
    pub fn of(
        base: &str,
        base_master: &str,
        composed_at: &str,
        members: Vec<ComposedMember>,
    ) -> Self {
        let numbers: Vec<u64> = members.iter().map(|m| m.number).collect();
        BatchManifest {
            schema: BATCH_SCHEMA.to_string(),
            id: batch_id(base_master, &numbers),
            base: base.to_string(),
            base_master: base_master.to_string(),
            composed_at: composed_at.to_string(),
            version_before: None,
            version_after: None,
            members,
        }
    }

    /// This manifest, recording what the composition's one bump did: the version the composed
    /// tree declared before it and the one it declares after ([`VersionStep`]).
    pub fn versioned(mut self, step: &VersionStep) -> Self {
        self.version_before = Some(step.before.clone());
        self.version_after = Some(step.after.clone());
        self
    }

    /// Whether the composition raised the version: `None` when the tree declares none.
    pub fn bumped(&self) -> Option<bool> {
        self.version_before
            .as_ref()
            .zip(self.version_after.as_ref())
            .map(|(before, after)| before != after)
    }

    /// The manifest as the layer's YAML, its keys in declaration order: `schema`, `id`,
    /// `base`, `base_master`, `composed_at`, `version_before` and `version_after` when the
    /// tree declares a version, then `members`, each `number`, `head`, `title`,
    /// `merge_commit`. Byte-deterministic for a given value.
    ///
    /// Written here rather than by [`crate::metadata::yaml::render`]: a title is the forge's
    /// text, and the layer's reader keeps a double-quoted scalar verbatim, backslashes and
    /// all, so a title holding a quote would not read back. A single-quoted scalar has one
    /// escape, which the reader and every YAML parser undo; the schema word and the base are
    /// bare where they can be ([`yaml_scalar`]), and everything else is quoted.
    ///
    /// ```text
    /// use crate::integration::compose::BatchManifest;
    /// let text = manifest.to_yaml();
    /// assert!(text.starts_with("schema: integration-batch/v1\nid: "));
    /// assert_eq!(BatchManifest::parse(&text).unwrap(), manifest);
    /// ```
    pub fn to_yaml(&self) -> String {
        let mut lines = vec![
            format!("schema: {}", yaml_scalar(&self.schema)),
            format!("id: {}", yaml_quoted(&self.id)),
            format!("base: {}", yaml_scalar(&self.base)),
            format!("base_master: {}", yaml_quoted(&self.base_master)),
            format!("composed_at: {}", yaml_quoted(&self.composed_at)),
        ];
        for (key, value) in [
            ("version_before", &self.version_before),
            ("version_after", &self.version_after),
        ] {
            lines.extend(value.iter().map(|v| format!("{key}: {}", yaml_quoted(v))));
        }
        if self.members.is_empty() {
            lines.push("members: []".to_string());
        } else {
            lines.push("members:".to_string());
        }
        for m in &self.members {
            lines.push(format!("  - number: {}", m.number));
            lines.push(format!("    head: {}", yaml_quoted(&m.head)));
            lines.push(format!("    title: {}", yaml_quoted(&m.title)));
            lines.push(format!(
                "    merge_commit: {}",
                yaml_quoted(&m.merge_commit)
            ));
        }
        lines.join("\n") + "\n"
    }

    /// The manifest a YAML document holds, or why it is none: a document outside the layer's
    /// subset, a key this version does not know, or another schema than [`BATCH_SCHEMA`].
    pub fn parse(text: &str) -> Result<Self, String> {
        let manifest: BatchManifest = crate::metadata::yaml::parse_into(text)?;
        if manifest.schema != BATCH_SCHEMA {
            return Err(format!(
                "schema {:?} is not {BATCH_SCHEMA}",
                manifest.schema
            ));
        }
        Ok(manifest)
    }

    /// The members' numbers, in composition order.
    pub fn numbers(&self) -> Vec<u64> {
        self.members.iter().map(|m| m.number).collect()
    }
}

/// The message of the merge commit that carries `member` into a batch on `base`.
pub fn merge_message(member: &BatchMember, base: &str) -> String {
    format!(
        "Merge pull request #{} into a batch on {base}\n\n{}\n\nMerged at {} by the integrator (majordomus prs compose, ADR 0114): this commit's\nsecond parent is the member's head, so a bisection over the batch lands on one member.",
        member.number, member.title, member.head
    )
}

/// The message of a batch's one composition commit: the manifest, the one bump when the
/// public contract of the composed tree required one, and what the derive wrote.
pub fn composition_message(manifest: &BatchManifest) -> String {
    let version = match (&manifest.version_before, &manifest.version_after) {
        (Some(before), Some(after)) if before != after => format!(
            "\n\nThe version is raised {before} -> {after}: what the public contract of the composed\ntree requires (majordomus release bump, ADR 0114 D6)."
        ),
        (Some(before), Some(_)) => format!(
            "\n\nThe version stays {before}: the public contract of the composed tree requires nothing\nabove it (majordomus release bump, ADR 0114 D6)."
        ),
        _ => String::new(),
    };
    format!(
        "chore(batch): batch {} is composed and derived\n\nThe manifest of {} member(s) on {} {} and the derived artifacts of the composed\ntree, written by the integrator (majordomus prs compose, ADR 0114). No person wrote\nthis commit, and it changes nothing a member's own branch could.{version}",
        manifest.id,
        manifest.members.len(),
        manifest.base,
        short(&manifest.base_master)
    )
}

/// What the composition's one `release bump` did to the composed tree (ADR 0114 D6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionStep {
    /// The version the tree declared after the member merges.
    pub before: String,
    /// The version it declares after the bump: `before` when the public contract required
    /// nothing above it, and nothing was written.
    pub after: String,
}

/// The title of a batch's pull request.
pub fn pull_request_title(manifest: &BatchManifest) -> String {
    format!(
        "Batch {}: {}",
        manifest.id,
        manifest
            .numbers()
            .iter()
            .map(|n| format!("#{n}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// The body of a batch's pull request: one `Supersedes #N` line per member, each on its own
/// line and nothing else on it, which is the declaration
/// [`super::classify::declared_supersessions`] reads — so each member is held while the batch
/// is open, released if it closes unmerged, and closable by cleanup once it lands — then the
/// members as the manifest records them. A member line opens with its number, never with a
/// marker, so a title cannot declare anything.
///
/// ```text
/// use crate::integration::{classify::declared_supersessions, compose::pull_request_body};
/// let body = pull_request_body(&manifest);
/// assert_eq!(declared_supersessions(&body).supersedes, manifest.numbers());
/// ```
pub fn pull_request_body(manifest: &BatchManifest) -> String {
    let mut lines = vec![format!(
        "A batch composed by the integrator (`majordomus prs compose`, ADR 0114) on {} {}. Its manifest is `{}`.",
        manifest.base,
        short(&manifest.base_master),
        manifest_path(&manifest.id)
    )];
    lines.push(String::new());
    lines.extend(
        manifest
            .members
            .iter()
            .map(|m| format!("Supersedes #{}", m.number)),
    );
    lines.push(String::new());
    lines.push("Members, in composition order:".to_string());
    lines.push(String::new());
    lines.extend(manifest.members.iter().map(|m| {
        format!(
            "- #{} at {}, merged as {}: {}",
            m.number,
            short(&m.head),
            short(&m.merge_commit),
            m.title
        )
    }));
    lines.push(String::new());
    lines.push(
        "A fix belongs on the member's own branch; the batch is then composed again.".to_string(),
    );
    lines.join("\n")
}

/// A batch that was composed and pushed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComposedBatch {
    /// The batch's head branch, new on the remote.
    pub branch: String,
    /// The batch's head: the composition commit.
    pub head: String,
    /// The pull request that was opened for it, when the forge's answer named one.
    pub pull_request: Option<u64>,
    /// The manifest that was committed.
    pub manifest: BatchManifest,
    /// The planned members the composition dropped, and why.
    pub dropped: Vec<LeftOut>,
}

/// Why a planned batch was not composed. Nothing reached the remote unless the reason says
/// so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotComposed {
    /// After the members that conflicted were dropped, fewer than [`MIN_MEMBERS`] were left.
    TooFew {
        /// The members whose merges succeeded.
        remaining: Vec<u64>,
        /// The members that were dropped, and why.
        dropped: Vec<LeftOut>,
    },
    /// A step of the act failed, in its own words.
    Failed {
        /// What failed.
        reason: String,
    },
}

/// Why a batch is not composed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ComposeRefusal {
    /// Fewer than two members were asked for: that is not a batch.
    MaxTooSmall {
        /// What was asked for.
        max: usize,
    },
    /// Neither the policy nor the caller says how many members a batch may carry: the code
    /// has no default (ADR 0114 D2).
    NoMaxMembers,
    /// The caller asked for more members than the policy allows: `--max` only lowers.
    MaxAbovePolicy {
        /// What was asked for.
        max: usize,
        /// What the policy allows.
        policy: usize,
    },
    /// The decision that makes composition a capability is not accepted in this layer, so
    /// the act is not taken; the dry run is unaffected.
    DecisionNotAccepted {
        /// The decision's status, in the layer's words, or why it could not be read.
        status: String,
    },
    /// Fewer than two pull requests are eligible: one is `prs repair`, none is nothing.
    TooFew {
        /// How many are eligible.
        eligible: usize,
    },
    /// The trail does not yet hold the verified merge that unlocks composition (ADR 0114 D7).
    RolloutIncomplete {
        /// What the trail holds, and what it needs.
        reason: String,
    },
    /// The composition was taken and dropped the members that conflicted, and fewer than two
    /// were left. Nothing was pushed.
    TooFewComposed {
        /// The members whose merges succeeded.
        remaining: Vec<u64>,
        /// The members that were dropped, and why.
        dropped: Vec<LeftOut>,
    },
    /// The act was taken and failed — a merge, a derive, a commit the hooks refused, a branch
    /// that already exists, a push or a pull request the forge refused.
    ActFailed {
        /// What failed, in the words of the step that failed.
        reason: String,
        /// Why, as a class.
        class: FailureClass,
    },
    /// The trail could not record the act first, so it was not taken.
    TrailUnwritable {
        /// Why the trail refused it.
        reason: String,
    },
}

impl ComposeRefusal {
    /// The class a refusal is recorded with on the trail.
    pub fn class(&self) -> FailureClass {
        match self {
            ComposeRefusal::TooFewComposed { .. } => FailureClass::Conflict,
            ComposeRefusal::ActFailed { class, .. } => *class,
            ComposeRefusal::TrailUnwritable { .. } => FailureClass::Unreadable,
            ComposeRefusal::MaxTooSmall { .. }
            | ComposeRefusal::NoMaxMembers
            | ComposeRefusal::MaxAbovePolicy { .. }
            | ComposeRefusal::DecisionNotAccepted { .. }
            | ComposeRefusal::TooFew { .. }
            | ComposeRefusal::RolloutIncomplete { .. } => FailureClass::PolicyViolation,
        }
    }
}

impl std::fmt::Display for ComposeRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComposeRefusal::MaxTooSmall { max } => write!(
                f,
                "a batch has at least {MIN_MEMBERS} members and at most {max} was asked for; one pull request is prs repair"
            ),
            ComposeRefusal::NoMaxMembers => write!(
                f,
                "nothing says how many members a batch may carry: the policy declares no \
                 {MAX_MEMBERS_KEY} and no --max was given; declare the key in the policy \
                 (.ai/repo/policy.yaml), or name a size for this one composition with --max"
            ),
            ComposeRefusal::MaxAbovePolicy { max, policy } => write!(
                f,
                "--max {max} is above the {policy} the policy allows ({MAX_MEMBERS_KEY}): \
                 --max may lower the cap for one composition and never raise it"
            ),
            ComposeRefusal::DecisionNotAccepted { status } => write!(
                f,
                "prs compose --apply is refused while ADR 0114 ({DECISION}) is not accepted: \
                 {status}. Accepting a decision is a person's act — the owner sets `status: \
                 accepted` in its front matter; nothing here does. The dry run (`prs compose`) \
                 is unaffected"
            ),
            ComposeRefusal::TooFew { eligible } => write!(
                f,
                "{eligible} pull request(s) are eligible and a batch has at least {MIN_MEMBERS}; one pull request is prs repair"
            ),
            ComposeRefusal::RolloutIncomplete { reason } => f.write_str(reason),
            ComposeRefusal::TooFewComposed { remaining, dropped } => write!(
                f,
                "nothing was pushed: {} member(s) were dropped during the composition ({}) and {} were left, fewer than {MIN_MEMBERS}",
                dropped.len(),
                dropped
                    .iter()
                    .map(|d| format!("#{}: {}", d.number, d.reason))
                    .collect::<Vec<_>>()
                    .join("; "),
                remaining.len()
            ),
            ComposeRefusal::ActFailed { reason, .. } => {
                write!(f, "the batch was not composed: {reason}")
            }
            ComposeRefusal::TrailUnwritable { reason } => write!(
                f,
                "the act was not taken: the trail could not record it first: {reason}"
            ),
        }
    }
}

/// What a composition did, or would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum ComposeOutcome {
    /// Dry run: the plan's members would be merged in order, derived, committed and pushed as
    /// a new branch.
    WouldCompose,
    /// The batch was composed, pushed as a new branch and its pull request opened.
    Composed {
        /// What was pushed. Boxed: it carries the whole manifest, and a refusal carries a
        /// sentence.
        composed: Box<ComposedBatch>,
    },
    /// Refused, and why.
    Refused {
        /// Why.
        refusal: ComposeRefusal,
    },
}

/// The answer of one `prs compose`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ComposeReport {
    /// Whether nothing was allowed to change.
    pub dry_run: bool,
    /// The plan the outcome was decided on; `None` when the refusal came before any queue was
    /// read.
    pub plan: Option<BatchPlan>,
    /// What happened.
    pub outcome: ComposeOutcome,
    /// What the observation the decision rests on cannot vouch for — the queue's own
    /// diagnostics, such as an observation older than an hour. A dry run with any exits 10, as
    /// every other reading of a diagnosed queue does.
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

/// Why `--max` is no batch, or nothing when it is one.
fn max_refused(max_members: usize) -> Option<ComposeRefusal> {
    (max_members < MIN_MEMBERS).then_some(ComposeRefusal::MaxTooSmall { max: max_members })
}

/// The report of `plan`, taken on `queue`, before anything is done.
fn report_of(queue: &IntegrationQueue, plan: BatchPlan, dry_run: bool) -> ComposeReport {
    let outcome = match max_refused(plan.max_members) {
        Some(refusal) => ComposeOutcome::Refused { refusal },
        None if plan.members.len() < MIN_MEMBERS => ComposeOutcome::Refused {
            refusal: ComposeRefusal::TooFew {
                eligible: plan.members.len(),
            },
        },
        None => ComposeOutcome::WouldCompose,
    };
    ComposeReport {
        dry_run,
        plan: Some(plan),
        outcome,
        diagnostics: queue.diagnostics.clone(),
    }
}

/// The dry run over a queue already built: what [`apply`] would do on it. Pure.
pub fn dry_run(queue: &IntegrationQueue, max_members: usize) -> ComposeReport {
    report_of(queue, decide(queue, max_members), true)
}

/// The dry run of this checkout: decided on the queue of the last recorded observation
/// ([`super::queue_of`]), the read every other non-acting `prs` command makes. It reaches no
/// network, takes no lease and records nothing; an observation that is absent is the error,
/// as it is for `prs status`.
pub fn plan(root: &Path, max_members: usize) -> Result<ComposeReport, String> {
    Ok(dry_run(&super::queue_of(root)?, max_members))
}

/// Why a batch may not be composed yet, or nothing when the trail's record allows it: the
/// trail must hold [`ROLLOUT_MERGES_BEFORE_COMPOSE`] verified merge(s) since the last merge
/// that could not be verified ([`verified_merges_since_failure`], the record continuous mode
/// counts too).
pub fn rollout_refused(trail: &[IntegrationEvent]) -> Option<String> {
    let record = verified_merges_since_failure(trail);
    (record < ROLLOUT_MERGES_BEFORE_COMPOSE).then(|| {
        format!(
            "composing a batch needs {ROLLOUT_MERGES_BEFORE_COMPOSE} verified merge(s) on the \
             trail since the last one that could not be verified, and it holds {record}: land \
             one with a bounded drain first (`prs drain --max 1`) (ADR 0114 D7, ADR 0101 §13)"
        )
    })
}

/// `#1 at aaaa, #2 at bbbb`: the members a trail line names, each at its head.
fn named(members: &[BatchMember]) -> String {
    members
        .iter()
        .map(|m| format!("#{} at {}", m.number, m.head))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A trail event of the composition of `plan`: no one pull request is its subject, so the
/// members are in the detail and the master is the plan's.
fn composing(action: IntegrationAction, plan: &BatchPlan, detail: String) -> IntegrationEvent {
    let mut e = event(action, None, detail);
    e.master_before = Some(plan.master_sha.clone());
    e
}

/// The act. The caller holds the base branch's integration lease for the whole call: the act
/// is a push to the forge's remote, and a drain must not race it. The rollout record is asked
/// first; then the forge is observed afresh, as a drain step observes it, and the batch is
/// decided again on that — never on a plan read earlier; every act is on the trail before it
/// reaches the remote. `Err` when the forge could not be observed, or the trail refused a
/// line recording a refusal or an outcome; an act the trail could not announce first is not
/// taken and is reported as [`ComposeRefusal::TrailUnwritable`].
pub fn apply(
    root: &Path,
    integrator: &mut dyn Integrator,
    max_members: usize,
) -> Result<ComposeReport, String> {
    let before = |refusal| ComposeReport {
        dry_run: false,
        plan: None,
        outcome: ComposeOutcome::Refused { refusal },
        diagnostics: Vec::new(),
    };
    // asked before the forge, and recorded nowhere: nothing was decided, so nothing is refused
    // about any pull request
    if let Some(refusal) = max_refused(max_members) {
        return Ok(before(refusal));
    }
    if let Some(reason) = rollout_refused(&events(root)) {
        return Ok(before(ComposeRefusal::RolloutIncomplete { reason }));
    }
    let queue = integrator.observe()?;
    let mut report = report_of(&queue, decide(&queue, max_members), false);
    let plan = match (&report.outcome, &report.plan) {
        (ComposeOutcome::WouldCompose, Some(plan)) => plan.clone(),
        (ComposeOutcome::Refused { refusal }, Some(plan)) => {
            let mut e = composing(IntegrationAction::ComposeRefused, plan, refusal.to_string());
            e.class = Some(refusal.class());
            record(root, e)?;
            return Ok(report);
        }
        _ => return Ok(report),
    };
    let unwritable = |reason| ComposeOutcome::Refused {
        refusal: ComposeRefusal::TrailUnwritable { reason },
    };
    let selected = composing(
        IntegrationAction::ComposeSelected,
        &plan,
        format!(
            "asked for by a person, at most {max_members}: {}; {} left out",
            named(&plan.members),
            plan.left_out.len()
        ),
    );
    if let Err(reason) = record(root, selected) {
        report.outcome = unwritable(reason);
        return Ok(report);
    }
    // on the trail before anything is pushed: a push the trail cannot name is not made
    let attempted = composing(
        IntegrationAction::ComposeAttempted,
        &plan,
        format!(
            "{} merged in order onto {} {}, derived, and pushed as a new branch {}<id>",
            named(&plan.members),
            plan.base,
            plan.master_sha,
            BATCH_BRANCH_PREFIX
        ),
    );
    if let Err(reason) = record(root, attempted) {
        report.outcome = unwritable(reason);
        return Ok(report);
    }
    let refusal = match integrator.compose_branch(&plan) {
        Ok(composed) => {
            let mut e = composing(
                IntegrationAction::Composed,
                &plan,
                format!(
                    "{} at {}{}: {}{}",
                    composed.branch,
                    composed.head,
                    composed
                        .pull_request
                        .map(|n| format!(", pull request #{n}"))
                        .unwrap_or_default(),
                    composed
                        .manifest
                        .members
                        .iter()
                        .map(|m| format!("#{} at {} as {}", m.number, m.head, m.merge_commit))
                        .collect::<Vec<_>>()
                        .join(", "),
                    if composed.dropped.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "; dropped {}",
                            composed
                                .dropped
                                .iter()
                                .map(|d| format!("#{} ({})", d.number, d.reason))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                ),
            );
            e.pr = composed.pull_request;
            e.master_after = Some(plan.master_sha.clone());
            e.head_after = Some(composed.head.clone());
            record(root, e)?;
            report.outcome = ComposeOutcome::Composed {
                composed: Box::new(composed),
            };
            return Ok(report);
        }
        Err(NotComposed::TooFew { remaining, dropped }) => {
            ComposeRefusal::TooFewComposed { remaining, dropped }
        }
        Err(NotComposed::Failed { reason }) => ComposeRefusal::ActFailed {
            class: FailureClass::of_refresh_failure(&reason),
            reason,
        },
    };
    let mut e = composing(
        IntegrationAction::ComposeRefused,
        &plan,
        refusal.to_string(),
    );
    e.class = Some(refusal.class());
    record(root, e)?;
    report.outcome = ComposeOutcome::Refused { refusal };
    Ok(report)
}
