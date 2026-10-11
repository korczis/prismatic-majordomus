//! `prs repair`: bring the current master into one named pull request whose only quarrel with
//! master is over derived files, on a person's request.
//!
//! # Why it exists
//!
//! `.gitattributes` marks every family of committed generated artifacts `merge=derived`, and
//! the driver that resolves them is per-clone configuration the forge never has. So the forge
//! runs its text merge over files whose content is a fingerprint of the merged tree and calls
//! nearly every pull request conflicting, over a conflict that exists on nobody's machine.
//! The remedy is rule `project.land-and-publish` clause 2: merge master in locally, where the
//! driver exists, regenerate, commit, push. The executor does exactly that for the first
//! refreshable pull request of the queue (`prs drain --refresh`); this is the same act for the
//! one a person names, whatever lane it waits in. It replaces `scripts/unblock`, the shell
//! script that performed the gesture outside the integrator.
//!
//! # What decides
//!
//! Nothing here classifies. [`decide`] reads the assessment the queue built — the relation to
//! master git decided with the derived attribute ([`super::relation`]), the disposition and
//! the reasons the classifier gave — and maps it onto three answers:
//!
//! - eligible: the relation is `behind`. The head does not contain master, and git's merge
//!   conflicts on no authored path: whatever the forge calls conflicting is a `merge=derived`
//!   path, which the regeneration settles;
//! - nothing to repair: the head already contains master, its work is on master already, or
//!   a declared successor landed — cleanup is the lane for those;
//! - refused: a conflict on an authored path (named, file by file: two people wrote two
//!   things and only they can say which is meant), a relation git could not decide, a base
//!   that is not the integration base, a fork's head this repository cannot push to, or a
//!   pull request with auto-merge armed, which the forge would merge on its own the moment a
//!   new head arrived.
//!
//! # What it reads, what it does, and what it never does
//!
//! The dry run is the default and it is a read: [`plan`] decides from the queue
//! [`super::queue_of`] builds out of the last recorded observation — the value `prs status`,
//! `prs plan` and `prs explain` render — so it reaches no network, takes no lease and writes
//! nothing to the trail. [`apply`] is the act, and it follows the drain's rules: the caller
//! holds the base branch's integration lease for the whole of it; it observes the forge
//! afresh, exactly as a drain step does ([`Integrator::observe`]), and decides again on that;
//! `repair_selected` and then `repair_attempted` are on the trail before anything reaches the
//! remote, and `repaired` or `repair_refused` after, so an act interrupted half-way leaves its
//! attempt behind. The act is the executor's own ([`Integrator::refresh_branch`]): a merge of
//! the decided master in a scratch worktree of its own, `scripts/derive` on the result, one
//! commit, and a push that is a fast-forward of the head that was observed, leased on that
//! head, so a branch somebody pushed to meanwhile is refused and never overwritten. It never
//! merges anything into master, and never touches a person's checkout.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::drain::{event, record, FailureClass, IntegrationAction, Integrator};
use super::{
    IntegrationQueue, PullRequestAssessment, PullRequestDisposition, ReasonCode, RelationToMaster,
};

/// The pull request a person names: its number, or its head branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairTarget {
    /// The number.
    Number(u64),
    /// The head branch of an open pull request of this repository.
    Branch(String),
}

impl RepairTarget {
    /// A number when the word is digits, a branch otherwise: a pull-request number is what a
    /// person has in front of them when they see the conflict.
    ///
    /// ```text
    /// use crate::integration::repair::RepairTarget;
    /// assert_eq!(RepairTarget::parse("137"), RepairTarget::Number(137));
    /// assert_eq!(RepairTarget::parse("feature/x"), RepairTarget::Branch("feature/x".into()));
    /// ```
    pub fn parse(word: &str) -> Self {
        match word.parse::<u64>() {
            Ok(n) if word.bytes().all(|b| b.is_ascii_digit()) => RepairTarget::Number(n),
            _ => RepairTarget::Branch(word.to_string()),
        }
    }

    /// The open pull request this names in `queue`. A branch name prefers this repository's
    /// own head over a fork's of the same name.
    fn find<'q>(&self, queue: &'q IntegrationQueue) -> Option<&'q PullRequestAssessment> {
        match self {
            RepairTarget::Number(n) => queue.get(*n),
            RepairTarget::Branch(b) => queue
                .assessments
                .iter()
                .filter(|a| &a.head_ref == b)
                .find(|a| !a.reasons.contains(&ReasonCode::ForkHead))
                .or_else(|| queue.assessments.iter().find(|a| &a.head_ref == b)),
        }
    }
}

impl std::fmt::Display for RepairTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepairTarget::Number(n) => write!(f, "#{n}"),
            RepairTarget::Branch(b) => f.write_str(b),
        }
    }
}

/// Why a pull request is not repaired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RepairRefusal {
    /// No open pull request is that number, or has that head branch.
    NotOpen {
        /// What was named.
        target: String,
    },
    /// It targets another branch than the integration base: the base is not what it lacks.
    OtherBase {
        /// The branch it targets.
        base: String,
    },
    /// Its head lives in a fork, which this repository cannot push to.
    ForkHead,
    /// The forge has auto-merge armed on it: a new head would be merged by the forge on its
    /// own, outside the executor.
    AutoMergeArmed,
    /// Merging master conflicts on authored paths: the owner's to settle.
    AuthoredConflict {
        /// The conflicting authored paths.
        paths: Vec<String>,
    },
    /// Git could not decide what the head is to master.
    Undecidable {
        /// Why.
        reason: String,
    },
    /// The act was taken and failed — a merge that conflicted after all, a derive that
    /// failed, a commit the hooks refused, or a push refused because the branch moved — and
    /// nothing reached the branch.
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

impl RepairRefusal {
    /// The class a refusal is recorded with on the trail.
    pub fn class(&self) -> FailureClass {
        match self {
            RepairRefusal::AuthoredConflict { .. } => FailureClass::Conflict,
            RepairRefusal::Undecidable { .. } | RepairRefusal::TrailUnwritable { .. } => {
                FailureClass::Unreadable
            }
            RepairRefusal::ActFailed { class, .. } => *class,
            RepairRefusal::NotOpen { .. }
            | RepairRefusal::OtherBase { .. }
            | RepairRefusal::ForkHead
            | RepairRefusal::AutoMergeArmed => FailureClass::PolicyViolation,
        }
    }
}

impl std::fmt::Display for RepairRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepairRefusal::NotOpen { target } => write!(
                f,
                "{target} is not an open pull request of this repository: nothing classifies it, so nothing may repair it"
            ),
            RepairRefusal::OtherBase { base } => write!(
                f,
                "it targets {base}, not the integration base: master is not what it lacks"
            ),
            RepairRefusal::ForkHead => f.write_str(
                "its head lives in a fork this repository cannot push to; the author merges the base in",
            ),
            RepairRefusal::AutoMergeArmed => f.write_str(
                "auto-merge is armed on it: a pushed head would be merged by the forge on its own; a person disarms it first",
            ),
            RepairRefusal::AuthoredConflict { paths } => write!(
                f,
                "merging master conflicts on {} authored file(s), which are the owner's to settle: {}",
                paths.len(),
                paths.join(", ")
            ),
            RepairRefusal::Undecidable { reason } => {
                write!(f, "git could not decide its relation to master: {reason}")
            }
            RepairRefusal::ActFailed { reason, .. } => {
                write!(f, "nothing reached the branch: {reason}")
            }
            RepairRefusal::TrailUnwritable { reason } => write!(
                f,
                "the act was not taken: the trail could not record it first: {reason}"
            ),
        }
    }
}

/// What a repair did, or would do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum RepairOutcome {
    /// Its head already contains master, or its work is on master already: there is nothing
    /// to bring in.
    NothingToRepair {
        /// Why, for a person.
        why: String,
    },
    /// Dry run: master would be brought in, derived, committed and pushed.
    WouldRepair,
    /// Master was brought in and the branch pushed, as a fast-forward leased on the head
    /// that was observed.
    Repaired {
        /// The head pushed.
        head_after: String,
    },
    /// Refused, and why.
    Refused {
        /// Why.
        refusal: RepairRefusal,
    },
}

/// The answer of one `prs repair`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepairReport {
    /// Whether nothing was allowed to change.
    pub dry_run: bool,
    /// What the person named.
    pub target: String,
    /// The pull request it named, when one is open.
    pub pr: Option<u64>,
    /// Its head branch.
    pub branch: Option<String>,
    /// The integration base.
    pub base: String,
    /// The master commit the decision was taken against.
    pub master_sha: String,
    /// The head the decision was taken against.
    pub head_sha: Option<String>,
    /// When the forge was observed, for the observation the decision was taken on.
    pub observed_at: String,
    /// What git decided the head is to master.
    pub relation: Option<RelationToMaster>,
    /// What happened.
    pub outcome: RepairOutcome,
    /// What the observation the decision rests on cannot vouch for — the queue's own
    /// diagnostics, such as an observation older than an hour. A dry run with any exits 10, as
    /// every other reading of a diagnosed queue does.
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

/// What the classification says about repairing one pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairDecision {
    /// Master may be brought in.
    Eligible(PullRequestAssessment),
    /// Nothing to bring in, and why.
    Nothing(PullRequestAssessment, String),
    /// Refused.
    Refused(Option<PullRequestAssessment>, RepairRefusal),
}

impl RepairDecision {
    fn assessment(&self) -> Option<&PullRequestAssessment> {
        match self {
            RepairDecision::Eligible(a) | RepairDecision::Nothing(a, _) => Some(a),
            RepairDecision::Refused(a, _) => a.as_ref(),
        }
    }
}

/// Decide from the queue alone — the relation, the disposition and the reasons the
/// classifier gave. Pure: it reads no file, runs no git and asks no forge.
pub fn decide(queue: &IntegrationQueue, target: &RepairTarget) -> RepairDecision {
    let Some(a) = target.find(queue) else {
        return RepairDecision::Refused(
            None,
            RepairRefusal::NotOpen {
                target: target.to_string(),
            },
        );
    };
    let refuse = |r| RepairDecision::Refused(Some(a.clone()), r);
    let nothing = |why: String| RepairDecision::Nothing(a.clone(), why);
    if a.base_ref != queue.base {
        return refuse(RepairRefusal::OtherBase {
            base: a.base_ref.clone(),
        });
    }
    if a.reasons.contains(&ReasonCode::AutoMergeArmed) {
        return refuse(RepairRefusal::AutoMergeArmed);
    }
    match a.disposition {
        PullRequestDisposition::Redundant => {
            return nothing(format!(
                "its work is on {} already; cleanup is the lane for it",
                queue.base
            ))
        }
        PullRequestDisposition::Superseded => {
            return nothing(format!(
                "a successor{} landed and replaces it; cleanup is the lane for it",
                a.superseded_by
                    .map(|n| format!(", #{n},"))
                    .unwrap_or_default()
            ))
        }
        _ => {}
    }
    match &a.relation {
        RelationToMaster::Conflicting { paths } => refuse(RepairRefusal::AuthoredConflict {
            paths: paths.clone(),
        }),
        RelationToMaster::Unknown { reason } => refuse(RepairRefusal::Undecidable {
            reason: reason.clone(),
        }),
        RelationToMaster::Behind { .. } if a.reasons.contains(&ReasonCode::ForkHead) => {
            refuse(RepairRefusal::ForkHead)
        }
        RelationToMaster::Behind { .. } => RepairDecision::Eligible(a.clone()),
        RelationToMaster::UpToDate { .. } => {
            nothing(format!("its head already contains {}", queue.base))
        }
        RelationToMaster::CarriesMaster { .. } => nothing(format!(
            "merging it into {} yields its own tree; its checks already judged that tree",
            queue.base
        )),
        RelationToMaster::Contained
        | RelationToMaster::Superseded
        | RelationToMaster::PatchIdsUpstream { .. } => nothing(format!(
            "its work is on {} already; cleanup is the lane for it",
            queue.base
        )),
        RelationToMaster::DerivedOnly { .. } => nothing(
            "only derived artifacts would change: the authored change appears to be on master \
             already, which a person confirms (possibly_redundant)"
                .to_string(),
        ),
    }
}

/// The report of `decision`, taken on `queue`, before anything is done.
fn report_of(
    queue: &IntegrationQueue,
    target: &RepairTarget,
    decision: &RepairDecision,
    dry_run: bool,
) -> RepairReport {
    let a = decision.assessment();
    RepairReport {
        dry_run,
        target: target.to_string(),
        pr: a.map(|a| a.number),
        branch: a.map(|a| a.head_ref.clone()),
        base: queue.base.clone(),
        master_sha: queue.master_sha.clone(),
        head_sha: a.map(|a| a.evaluated_against.head_sha.clone()),
        observed_at: queue.observed_at.clone(),
        relation: a.map(|a| a.relation.clone()),
        diagnostics: queue.diagnostics.clone(),
        outcome: match decision {
            RepairDecision::Eligible(_) => RepairOutcome::WouldRepair,
            RepairDecision::Nothing(_, why) => RepairOutcome::NothingToRepair { why: why.clone() },
            RepairDecision::Refused(_, refusal) => RepairOutcome::Refused {
                refusal: refusal.clone(),
            },
        },
    }
}

/// The dry run over a queue already built: what [`apply`] would do on it. Pure.
pub fn dry_run(queue: &IntegrationQueue, target: &RepairTarget) -> RepairReport {
    report_of(queue, target, &decide(queue, target), true)
}

/// The dry run of this checkout: decided on the queue of the last recorded observation
/// ([`super::queue_of`]), the read every other non-acting `prs` command makes. It reaches no
/// network, takes no lease and records nothing; an observation that is absent is the error,
/// as it is for `prs status`.
pub fn plan(root: &Path, target: &RepairTarget) -> Result<RepairReport, String> {
    Ok(dry_run(&super::queue_of(root)?, target))
}

/// The act. The caller holds the base branch's integration lease for the whole call: the act
/// is a push to the forge's remote, and a drain must not race it. The forge is observed
/// afresh first, as a drain step observes it, and the decision is taken again on that; every
/// act is on the trail before it reaches the remote. `Err` when the forge could not be
/// observed, or the trail refused a line recording a refusal or an outcome; an act the trail
/// could not announce first is not taken and is reported as
/// [`RepairRefusal::TrailUnwritable`].
pub fn apply(
    root: &Path,
    integrator: &mut dyn Integrator,
    target: &RepairTarget,
) -> Result<RepairReport, String> {
    let queue = integrator.observe()?;
    let decision = decide(&queue, target);
    let mut report = report_of(&queue, target, &decision, false);
    let candidate = match decision {
        RepairDecision::Nothing(..) => return Ok(report),
        RepairDecision::Refused(a, refusal) => {
            let mut e = event(
                IntegrationAction::RepairRefused,
                a.as_ref(),
                refusal.to_string(),
            );
            e.class = Some(refusal.class());
            record(root, e)?;
            return Ok(report);
        }
        RepairDecision::Eligible(a) => a,
    };
    let unwritable = |reason| RepairOutcome::Refused {
        refusal: RepairRefusal::TrailUnwritable { reason },
    };
    let selected = event(
        IntegrationAction::RepairSelected,
        Some(&candidate),
        format!("named by a person: {target}"),
    )
    .with_evidence(&candidate);
    if let Err(reason) = record(root, selected) {
        report.outcome = unwritable(reason);
        return Ok(report);
    }
    // on the trail before anything is pushed: a push the trail cannot name is not made
    let attempted = event(
        IntegrationAction::RepairAttempted,
        Some(&candidate),
        format!(
            "master {} into {}, pushed leased on {}",
            queue.master_sha, candidate.head_ref, candidate.evaluated_against.head_sha
        ),
    );
    if let Err(reason) = record(root, attempted) {
        report.outcome = unwritable(reason);
        return Ok(report);
    }
    match integrator.refresh_branch(&candidate, &queue.base) {
        Ok(head_after) => {
            let mut e = event(
                IntegrationAction::Repaired,
                Some(&candidate),
                format!("new head {head_after}"),
            );
            e.master_after = Some(queue.master_sha.clone());
            e.head_after = Some(head_after.clone());
            record(root, e)?;
            report.outcome = RepairOutcome::Repaired { head_after };
        }
        Err(reason) => {
            let refusal = RepairRefusal::ActFailed {
                class: FailureClass::of_refresh_failure(&reason),
                reason,
            };
            let mut e = event(
                IntegrationAction::RepairRefused,
                Some(&candidate),
                refusal.to_string(),
            );
            e.class = Some(refusal.class());
            record(root, e)?;
            report.outcome = RepairOutcome::Refused { refusal };
        }
    }
    Ok(report)
}
