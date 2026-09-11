//! The classifier: measurement in, typed lifecycle out.
//!
//! This is the only place in the repository that decides what state a piece of work is in.
//! Every decision below names the measurement that produced it and records it as
//! [`Evidence`] on the record, so that a reader can disagree with the classifier by
//! checking the number rather than by arguing about the word.
//!
//! The precedence is deliberate and is the reason this module exists. A previous sweep left
//! forty-two branches carrying a single unpushed checkpoint commit, eleven of them with a
//! pull request already merged. A classifier that reads "the pull request is merged" first
//! calls those disposable and deletes them. This one reads the residue first: a landing
//! that does not contain everything the branch carries is [`WorkState::Superseded`], never
//! [`WorkState::Landed`], and nothing superseded is ever provably safe to remove.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::model::Severity;
use crate::worktree::{DirtyState, RepositoryTopology, Standing, WorktreeError, WorktreeService};

use super::model::{
    Blocker, CleanupPlan, CleanupStep, ClosureBlocker, Evidence, Finding, Landing,
    LifecycleReport, LifecycleTallies, PullRequestState, PullRequestView,
    StateCount, WorkRecord, WorkState, SCHEMA,
};
use super::policy::AgingPolicy;
use super::{forge, measure, Clock};

/// How many merges of the trunk are read when looking for landings. Far enough back to
/// cover every branch anybody still has a worktree for, and bounded so the cost does not
/// grow with the repository's whole history.
const LANDING_DEPTH: usize = 2_000;

/// How many of a record's unique commits are shown.
const SAMPLE: usize = 5;

/// What to measure.
#[derive(Debug, Clone, Default)]
pub struct Request {
    /// Where pull request facts may come from.
    pub source: forge::Source,
    /// Also report local branches that have no worktree. On by default: a branch with
    /// unique commits and no worktree is exactly the kind of work that goes unnoticed.
    pub include_branches: bool,
    /// Peers of the shared board, as `(id, the text they announced)`. A peer that names a
    /// branch owns its worktree, and an owned worktree is never stale and never pruned.
    pub peers: Vec<(String, String)>,
}

/// The lifecycle of a repository's work.
pub struct LifecycleService {
    root: PathBuf,
    clock: Clock,
    policy: AgingPolicy,
}

impl LifecycleService {
    /// Open the lifecycle over a repository, with a clock and the resolved thresholds.
    pub fn new(root: impl Into<PathBuf>, clock: Clock, policy: AgingPolicy) -> Self {
        LifecycleService {
            root: root.into(),
            clock,
            policy,
        }
    }

    /// The whole lifecycle: every worktree, every branch, every pull request.
    pub fn report(&self, request: &Request) -> Result<LifecycleReport, WorktreeError> {
        let service = WorktreeService::open(&self.root)?;
        let topology = service.topology(crate::worktree::Detail::Full)?;
        let root = PathBuf::from(&topology.repository.primary_worktree);

        let mut limitations = Vec::new();
        let trunk = topology.trunk.branch.clone();
        // Landings are read from the *integration* ref: what reached origin, not what this
        // clone happens to hold. A local trunk can be ahead, behind or rewritten, and a
        // branch judged landed against a local merge nobody pushed is judged against
        // nothing.
        let integration = match &trunk {
            Some(branch) => {
                let remote = format!("origin/{branch}");
                if measure::committed_at(&root, &remote).is_some() {
                    Some(remote)
                } else {
                    limitations.push(format!(
                        "`origin/{branch}` is not in this clone, so landings were read from \
                         the local `{branch}`: a branch judged landed here may not have \
                         landed anywhere else"
                    ));
                    Some(branch.clone())
                }
            }
            None => {
                limitations.push(
                    "the trunk could not be determined, so nothing could be judged landed"
                        .into(),
                );
                None
            }
        };

        let landings = match &integration {
            Some(reference) => measure::landings(&root, reference, LANDING_DEPTH),
            None => BTreeMap::new(),
        };
        let forge = forge::read(&root, integration.as_deref(), request.source, self.clock);
        limitations.extend(forge.limitations.iter().cloned());
        if request.peers.is_empty() {
            limitations.push(
                "no peer of the shared board was visible from this process, so a worktree \
                 someone is working in right now can only be recognised by how recent its \
                 newest commit is"
                    .into(),
            );
        }

        let mut records = Vec::new();
        let mut with_worktree: BTreeSet<String> = BTreeSet::new();

        for worktree in &topology.worktrees {
            if let Some(branch) = &worktree.branch {
                with_worktree.insert(branch.clone());
            }
            records.push(self.record_for_worktree(
                &root,
                worktree,
                &topology,
                &landings,
                &forge.pull_requests,
                integration.as_deref(),
                request,
            ));
        }

        if request.include_branches {
            for branch in &topology.branches {
                if with_worktree.contains(&branch.name) || branch.trunk {
                    continue;
                }
                records.push(self.record_for_branch(
                    &root,
                    &branch.name,
                    &branch.head,
                    branch.issue.clone(),
                    &landings,
                    &forge.pull_requests,
                    integration.as_deref(),
                    request,
                ));
            }
        }

        records.sort_by(|a, b| {
            a.state
                .cmp(&b.state)
                .then_with(|| b.unique_commits.cmp(&a.unique_commits))
                .then_with(|| a.label.cmp(&b.label))
        });

        let findings = self.findings(&records, &forge.pull_requests);
        let tallies = tallies(&records, &forge.pull_requests, &findings);

        Ok(LifecycleReport {
            schema: SCHEMA.into(),
            measured_at: self.clock.rfc3339(),
            trunk,
            pull_request_source: forge.source,
            complete: limitations.is_empty(),
            limitations,
            policy: self.policy.clone(),
            records,
            pull_requests: forge.pull_requests,
            findings,
            tallies,
        })
    }

    /// What a prune would do. Producing this changes nothing.
    pub fn cleanup_plan(&self, request: &Request) -> Result<CleanupPlan, WorktreeError> {
        let report = self.report(request)?;
        let mut removable = Vec::new();
        let mut refused = Vec::new();
        for record in report.records {
            let Some(path) = record.worktree.clone() else {
                continue;
            };
            let step = CleanupStep {
                command: format!("majordomus worktree remove {path}"),
                path,
                branch: record.branch.clone(),
                state: record.state,
                reason: if record.cleanup_safe {
                    format!(
                        "{}: nothing is reachable from no origin ref, nothing is \
                         uncommitted, nobody is working here",
                        record.state.as_str()
                    )
                } else {
                    record
                        .blockers
                        .iter()
                        .map(|b| b.message.clone())
                        .collect::<Vec<_>>()
                        .join("; ")
                },
                blockers: record.blockers.clone(),
            };
            if record.cleanup_safe {
                removable.push(step);
            } else {
                refused.push(step);
            }
        }
        Ok(CleanupPlan {
            schema: SCHEMA.into(),
            measured_at: self.clock.rfc3339(),
            removable,
            refused,
            applied: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn record_for_worktree(
        &self,
        root: &Path,
        worktree: &crate::worktree::WorktreeState,
        topology: &RepositoryTopology,
        landings: &BTreeMap<String, Landing>,
        pull_requests: &[PullRequestView],
        integration: Option<&str>,
        request: &Request,
    ) -> WorkRecord {
        let primary = worktree.kind == crate::worktree::WorktreeKind::Primary;
        let trunk_here = topology
            .trunk
            .branch
            .as_deref()
            .zip(worktree.branch.as_deref())
            .map(|(t, b)| t == b)
            .unwrap_or(false);

        let rev = worktree
            .branch
            .clone()
            .or_else(|| worktree.head.clone())
            .unwrap_or_else(|| "HEAD".into());

        let mut record = self.measure_record(
            root,
            &worktree.label,
            worktree.branch.clone(),
            &rev,
            worktree.head.clone(),
            worktree.issue.clone(),
            landings,
            pull_requests,
            integration,
            request,
        );
        record.worktree = Some(worktree.path.clone());
        record.dirty = worktree.dirty.clone();

        let unreachable = matches!(worktree.standing, Standing::Missing)
            || worktree.prunable.is_some()
            || !worktree.exists;
        let locked = worktree.locked.is_some();
        self.classify(
            &mut record,
            Classification {
                unreachable,
                locked,
                detached: worktree.detached,
                primary,
                trunk_here,
            },
        );
        record
    }

    #[allow(clippy::too_many_arguments)]
    fn record_for_branch(
        &self,
        root: &Path,
        branch: &str,
        head: &str,
        issue: Option<String>,
        landings: &BTreeMap<String, Landing>,
        pull_requests: &[PullRequestView],
        integration: Option<&str>,
        request: &Request,
    ) -> WorkRecord {
        let mut record = self.measure_record(
            root,
            branch,
            Some(branch.to_string()),
            branch,
            Some(head.to_string()),
            issue,
            landings,
            pull_requests,
            integration,
            request,
        );
        self.classify(
            &mut record,
            Classification {
                unreachable: false,
                locked: false,
                detached: false,
                primary: false,
                trunk_here: false,
            },
        );
        record
    }

    #[allow(clippy::too_many_arguments)]
    fn measure_record(
        &self,
        root: &Path,
        label: &str,
        branch: Option<String>,
        rev: &str,
        head: Option<String>,
        issue: Option<String>,
        landings: &BTreeMap<String, Landing>,
        pull_requests: &[PullRequestView],
        integration: Option<&str>,
        request: &Request,
    ) -> WorkRecord {
        let unique = measure::unique_commits(root, rev);
        let unique_sample = if unique.unwrap_or(0) > 0 {
            measure::unique_sample(root, rev, SAMPLE)
        } else {
            Vec::new()
        };
        let age_days = measure::committed_at(root, rev).map(|t| self.clock.days_since(t));
        let merged_into_trunk = integration
            .and_then(|trunk| measure::is_ancestor(root, rev, trunk))
            .unwrap_or(false);
        let archived_as = measure::archive_tags(root, rev);

        let mut landing = branch.as_deref().and_then(|b| landings.get(b).cloned());
        if let Some(landing) = landing.as_mut() {
            // What the branch carries that the merge does not: the residue. This is the
            // measurement the eleven merged-pull-request branches were misread on.
            landing.residue = residue(root, rev, &landing.merge_commit, SAMPLE);
        }

        let mine: Vec<PullRequestView> = branch
            .as_deref()
            .map(|b| {
                pull_requests
                    .iter()
                    .filter(|pr| pr.branch.as_deref() == Some(b))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        let owner = branch.as_deref().and_then(|b| {
            request
                .peers
                .iter()
                .find(|(_, text)| text.contains(b))
                .map(|(id, _)| id.clone())
        });

        WorkRecord {
            label: label.to_string(),
            branch,
            worktree: None,
            head,
            state: WorkState::Idle,
            evidence: Vec::new(),
            unique_commits: unique.unwrap_or(0),
            unique_sample,
            dirty: None,
            landing,
            pull_requests: mine,
            issue: issue.clone(),
            task: issue,
            milestone: None,
            merged_into_trunk,
            archived_as,
            age_days,
            owner,
            cleanup_safe: false,
            blockers: Vec::new(),
        }
    }

    /// Put a measured record in its state, and record why.
    fn classify(&self, record: &mut WorkRecord, facts: Classification) {
        let dirty = record.dirty.clone().unwrap_or_default();
        let clean = record.dirty.as_ref().map(|d| d.clean).unwrap_or(true);
        let landed = record.landing.is_some() || record.merged_into_trunk;
        let residue = record
            .landing
            .as_ref()
            .map(|l| !l.residue.is_empty())
            .unwrap_or(false);
        let open_pr = record.pull_requests.iter().any(|pr| pr.state.is_open());
        let age = record.age_days.map(|d| d * 86_400);
        let mut evidence = Vec::new();

        evidence.push(Evidence::new(
            "unique_commits",
            format!(
                "{} reachable from no ref on origin (`git rev-list --count {} --not \
                 --remotes=origin`)",
                record.unique_commits,
                record.branch.as_deref().unwrap_or("HEAD")
            ),
        ));
        if record.dirty.is_some() {
            evidence.push(Evidence::new("dirty", dirty.summary()));
        }
        if let Some(days) = record.age_days {
            evidence.push(Evidence::new(
                "age",
                format!("the newest commit here is {days} day(s) old"),
            ));
        }

        let state = if facts.unreachable {
            evidence.push(Evidence::new(
                "registration",
                "the directory git registered is gone or the registration is prunable",
            ));
            WorkState::Orphaned
        } else if let Some(owner) = record.owner.clone() {
            evidence.push(Evidence::new("live_owner", format!("peer {owner} named it")));
            WorkState::Active
        } else if facts.locked {
            evidence.push(Evidence::new("lock", "git holds this worktree locked"));
            WorkState::Active
        } else if age.map(|a| a < self.policy.active_within_seconds).unwrap_or(false) {
            evidence.push(Evidence::new(
                "age",
                "the newest commit is inside the policy's `active_within`",
            ));
            WorkState::Active
        } else if dirty.in_progress.is_some() || dirty.conflicted > 0 {
            evidence.push(Evidence::new(
                "in_progress",
                dirty
                    .in_progress
                    .clone()
                    .unwrap_or_else(|| format!("{} path(s) in conflict", dirty.conflicted)),
            ));
            WorkState::Blocked
        } else if landed && (record.unique_commits > 0 || residue || !clean) {
            evidence.push(Evidence::new(
                "residue_after_landing",
                match &record.landing {
                    Some(l) => format!(
                        "merge {} landed this branch{} and it still carries {} commit(s) \
                         reachable from no origin ref{}",
                        &l.merge_commit[..12.min(l.merge_commit.len())],
                        l.pull_request
                            .map(|n| format!(" as pull request #{n}"))
                            .unwrap_or_default(),
                        record.unique_commits,
                        if clean {
                            String::new()
                        } else {
                            format!(" and {}", dirty.summary())
                        }
                    ),
                    None => "the tip is reachable from the trunk and work remains here".into(),
                },
            ));
            WorkState::Superseded
        } else if record.unique_commits > 0 {
            if !record.archived_as.is_empty() {
                evidence.push(Evidence::new(
                    "archived",
                    format!("preserved by {}", record.archived_as.join(", ")),
                ));
                WorkState::Archived
            } else {
                evidence.push(Evidence::new(
                    "stranded",
                    "these commits exist on this disk and nowhere else",
                ));
                WorkState::Stranded
            }
        } else if facts.detached {
            evidence.push(Evidence::new(
                "detached",
                "no branch holds what is checked out here",
            ));
            WorkState::Orphaned
        } else if open_pr {
            evidence.push(Evidence::new(
                "pull_request",
                record
                    .pull_requests
                    .iter()
                    .filter(|pr| pr.state.is_open())
                    .map(|pr| format!("#{} {}", pr.number, pr.state.as_str()))
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
            WorkState::Landing
        } else if landed {
            evidence.push(Evidence::new(
                "landed",
                match &record.landing {
                    Some(l) => format!(
                        "merge {} contains everything here",
                        &l.merge_commit[..12.min(l.merge_commit.len())]
                    ),
                    None => "the tip is reachable from the trunk".into(),
                },
            ));
            WorkState::Landed
        } else if !clean {
            WorkState::Dirty
        } else if !record.merged_into_trunk && record.branch.is_some() && !facts.trunk_here {
            evidence.push(Evidence::new(
                "ready",
                "every commit here is on origin and no pull request carries it",
            ));
            WorkState::Ready
        } else if age
            .map(|a| a > self.policy.worktree_inactive_after_seconds)
            .unwrap_or(false)
            && record.task.is_none()
        {
            WorkState::Stale
        } else {
            WorkState::Idle
        };

        record.state = state;
        record.evidence = evidence;
        record.blockers = self.blockers(record, &facts, &dirty, clean, open_pr, residue);
        record.cleanup_safe = record.blockers.is_empty()
            && matches!(
                state,
                WorkState::Landed | WorkState::Stale | WorkState::Idle | WorkState::Ready
            );
    }

    fn blockers(
        &self,
        record: &WorkRecord,
        facts: &Classification,
        dirty: &DirtyState,
        clean: bool,
        open_pr: bool,
        residue: bool,
    ) -> Vec<ClosureBlocker> {
        let mut blockers = Vec::new();
        let mut add = |code: Blocker, message: String, remedy: &str| {
            blockers.push(ClosureBlocker {
                code,
                message,
                remedy: remedy.to_string(),
            })
        };
        if facts.primary || facts.trunk_here {
            add(
                Blocker::PrimaryOrTrunk,
                "this is the primary checkout, or it holds the trunk".into(),
                "nothing: the primary checkout is never removed",
            );
        }
        if facts.locked {
            add(
                Blocker::Locked,
                "git holds this worktree locked".into(),
                "git worktree unlock <path>, once you know why it was locked",
            );
        }
        if let Some(owner) = &record.owner {
            add(
                Blocker::LiveOwner,
                format!("peer {owner} is working here"),
                "ask them, or wait: an announced worktree is somebody's open work",
            );
        }
        if !clean {
            add(
                Blocker::UncommittedWork,
                format!(
                    "{} — removing the worktree destroys this, and nothing else holds it",
                    dirty.summary()
                ),
                "commit it on its branch and push, or record why it is being discarded",
            );
        }
        if dirty.in_progress.is_some() {
            add(
                Blocker::OperationInProgress,
                format!(
                    "{} is in progress here",
                    dirty.in_progress.clone().unwrap_or_default()
                ),
                "finish or abort it before deciding anything about this worktree",
            );
        }
        if record.unique_commits > 0 {
            add(
                Blocker::UniqueCommits,
                format!(
                    "{} commit(s) are reachable from no ref on origin{}",
                    record.unique_commits,
                    if record.branch.is_some() {
                        " (the branch would survive a worktree removal, but nothing off \
                         this disk holds them)"
                    } else {
                        " and no branch holds them: removing this worktree loses them"
                    }
                ),
                "push the branch: git push -u origin <branch>",
            );
        }
        if residue {
            add(
                Blocker::ResidueAfterLanding,
                "the merge that landed this branch does not contain everything it carries"
                    .into(),
                "open a pull request for the remainder, or record why it is not wanted",
            );
        }
        if facts.detached {
            add(
                Blocker::DetachedHead,
                "HEAD is detached: no branch preserves what is checked out here".into(),
                "git branch <name> to give it a ref before deciding anything",
            );
        }
        if open_pr {
            add(
                Blocker::OpenPullRequest,
                record
                    .pull_requests
                    .iter()
                    .filter(|pr| pr.state.is_open())
                    .map(|pr| format!("pull request #{} is open", pr.number))
                    .collect::<Vec<_>>()
                    .join(", "),
                "land it or close it first",
            );
        }
        if record.age_days.is_none() && !facts.unreachable {
            add(
                Blocker::Unproven,
                "git could not date what is here, so nothing about it is proved".into(),
                "look at it by hand: a check that cannot see its subject does not decide it",
            );
        }
        blockers
    }

    fn findings(&self, records: &[WorkRecord], pull_requests: &[PullRequestView]) -> Vec<Finding> {
        let mut findings = Vec::new();
        for record in records {
            let age = record.age_days.map(|d| d * 86_400).unwrap_or(0);
            match record.state {
                WorkState::Stranded if age >= self.policy.stranded_after_seconds => {
                    findings.push(Finding {
                        code: "lifecycle.stranded_work".into(),
                        severity: Severity::Error,
                        subject: record.label.clone(),
                        message: format!(
                            "{} commit(s) here are reachable from no ref on origin and have \
                             been for {} day(s)",
                            record.unique_commits,
                            record.age_days.unwrap_or(0)
                        ),
                        remedy: format!(
                            "git -C <worktree> push -u origin {}",
                            record.branch.as_deref().unwrap_or("<branch>")
                        ),
                    });
                }
                WorkState::Superseded => {
                    findings.push(Finding {
                        code: "lifecycle.residue_after_landing".into(),
                        severity: Severity::Error,
                        subject: record.label.clone(),
                        message: format!(
                            "this branch landed{} and still carries {} commit(s) the landing \
                             does not contain; it is not disposable",
                            record
                                .landing
                                .as_ref()
                                .and_then(|l| l.pull_request)
                                .map(|n| format!(" as pull request #{n}"))
                                .unwrap_or_default(),
                            record.unique_commits.max(
                                record.landing.as_ref().map(|l| l.residue.len()).unwrap_or(0)
                            )
                        ),
                        remedy: "open a pull request for the remainder, or archive it with a \
                                 recorded reason"
                            .into(),
                    });
                }
                WorkState::Orphaned => findings.push(Finding {
                    code: "lifecycle.orphaned_worktree".into(),
                    severity: Severity::Warning,
                    subject: record.label.clone(),
                    message: "this registration has no usable subject: the directory is gone, \
                              git calls it prunable, or no branch holds what is here"
                        .into(),
                    remedy: "majordomus worktree repair, after checking what the commits here \
                             are"
                        .into(),
                }),
                WorkState::Stale => findings.push(Finding {
                    code: "lifecycle.worktree_stale".into(),
                    severity: Severity::Info,
                    subject: record.label.clone(),
                    message: format!(
                        "nothing unique, nothing uncommitted, and no commit for {} day(s)",
                        record.age_days.unwrap_or(0)
                    ),
                    remedy: "majordomus lifecycle prune --apply removes it, having proved it \
                             holds nothing"
                        .into(),
                }),
                WorkState::Landed if age >= self.policy.branch_merged_after_seconds => {
                    findings.push(Finding {
                        code: "lifecycle.branch_merged".into(),
                        severity: Severity::Info,
                        subject: record.label.clone(),
                        message: format!(
                            "landed {} day(s) ago and nothing here is unique",
                            record.age_days.unwrap_or(0)
                        ),
                        remedy: "majordomus lifecycle prune --apply".into(),
                    });
                }
                _ => {}
            }
        }
        for pr in pull_requests {
            let age = pr.age_days.unwrap_or(0) * 86_400;
            match pr.state {
                PullRequestState::Mergeable
                    if age >= self.policy.pull_request_mergeable_after_seconds =>
                {
                    findings.push(Finding {
                        code: "lifecycle.pull_request_aged".into(),
                        severity: Severity::Warning,
                        subject: format!("#{}", pr.number),
                        message: format!(
                            "mergeable for {} day(s): nothing technical is stopping it",
                            pr.age_days.unwrap_or(0)
                        ),
                        remedy: format!("gh pr merge {}", pr.number),
                    });
                }
                PullRequestState::Failing
                    if age >= self.policy.pull_request_failing_after_seconds =>
                {
                    findings.push(Finding {
                        code: "lifecycle.pull_request_failing".into(),
                        severity: Severity::Warning,
                        subject: format!("#{}", pr.number),
                        message: format!(
                            "failing for {} day(s)",
                            pr.age_days.unwrap_or(0)
                        ),
                        remedy: format!("gh pr checks {}", pr.number),
                    });
                }
                PullRequestState::Superseded => findings.push(Finding {
                    code: "lifecycle.pull_request_superseded".into(),
                    severity: Severity::Warning,
                    subject: format!("#{}", pr.number),
                    message: "its head is already reachable from the trunk: the content landed \
                              by another route and nobody closed this"
                        .into(),
                    remedy: format!("gh pr close {}", pr.number),
                }),
                _ => {}
            }
        }
        findings
    }
}

/// The facts about a worktree that are not measurements of its commits.
#[derive(Debug, Clone, Copy)]
struct Classification {
    unreachable: bool,
    locked: bool,
    detached: bool,
    primary: bool,
    trunk_here: bool,
}

/// Commits on `rev` that `merge` does not contain: what a landing left behind.
fn residue(root: &Path, rev: &str, merge: &str, limit: usize) -> Vec<String> {
    let max = format!("--max-count={limit}");
    let Ok(out) = crate::worktree::git::try_run(
        root,
        &[
            "log",
            &max,
            "--format=%h %s",
            "--end-of-options",
            rev,
            &format!("^{merge}"),
        ],
    ) else {
        return Vec::new();
    };
    if out.status != Some(0) {
        return Vec::new();
    }
    out.text()
        .map(|t| {
            t.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn tallies(
    records: &[WorkRecord],
    pull_requests: &[PullRequestView],
    findings: &[Finding],
) -> LifecycleTallies {
    let by_state = WorkState::ALL
        .iter()
        .map(|state| StateCount {
            state: *state,
            count: records.iter().filter(|r| r.state == *state).count(),
        })
        .filter(|c| c.count > 0)
        .collect();
    LifecycleTallies {
        records: records.len(),
        by_state,
        unconverged: records
            .iter()
            .filter(|r| r.state.holds_unconverged_work())
            .count(),
        unique_commits: records.iter().map(|r| r.unique_commits).sum(),
        cleanup_safe: records.iter().filter(|r| r.cleanup_safe).count(),
        pull_requests: pull_requests.len(),
        findings: findings.len(),
    }
}
