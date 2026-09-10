//! The worktree service: every decision about where a worktree belongs, whether it may be
//! created, moved or removed, and what the topology currently is.
//!
//! This is the only implementation. The command line renders what it answers, the capability
//! registry projects three of its read-only questions to MCP, HTTP, OpenAPI and the cockpit,
//! and `majordomus doctor` reads the same report. None of them decides anything; a second
//! decision somewhere else is the defect this file exists to prevent.
//!
//! Nothing here touches the network. `git fetch` is never run, a ref that does not resolve
//! locally does not resolve, and every operation works with the machine offline.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::error::{Result, WorktreeError};
use super::git;
use super::identity::{RepositoryIdentity, ResolvedPath};
use super::lock::WorktreeLock;
use super::naming::WorktreeName;
use super::policy::{Disposition, Level, WorktreePolicy};
use super::topology::{self, WorktreeRecord};

/// The file written inside the container the first time one is created. It is evidence, not
/// authority: the container's location is always derived from the repository's identity and
/// the policy, and this file is never read to find it. It is read to *confirm*, before a
/// destructive operation, that the directory being operated inside is the container this
/// repository made and not a directory that happens to have the same name.
pub const ROOT_MARKER: &str = ".majordomus-worktree-root";

// ---------------------------------------------------------------- output types

/// Whether a work tree is the repository's own checkout or one linked to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WorktreeKind {
    /// The main work tree. Never moved, never removed, exempt from the layout rule.
    Primary,
    /// A linked work tree. Belongs under the canonical root.
    Linked,
}

impl WorktreeKind {
    /// The word this kind is reported under.
    pub fn as_str(self) -> &'static str {
        match self {
            WorktreeKind::Primary => "primary",
            WorktreeKind::Linked => "linked",
        }
    }
}

/// What the layout rule says about one work tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PolicyStatus {
    /// Where it belongs.
    Pass,
    /// Not where it belongs.
    Fail,
    /// The rule does not apply: the primary checkout.
    Exempt,
}

impl PolicyStatus {
    /// The word this status is reported under.
    pub fn as_str(self) -> &'static str {
        match self {
            PolicyStatus::Pass => "pass",
            PolicyStatus::Fail => "fail",
            PolicyStatus::Exempt => "exempt",
        }
    }
}

/// One work tree, as the topology and the policy see it together.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeView {
    /// Absolute, as git holds it.
    pub path: String,
    /// The directory name, which is also this worktree's short selector.
    pub name: String,
    /// Primary or linked.
    pub kind: WorktreeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch checked out here, short. Absent when detached.
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commit checked out here.
    pub head: Option<String>,
    /// HEAD is detached here.
    pub detached: bool,
    /// The command was run inside this work tree.
    pub current: bool,
    /// The directory git registered still exists on disk.
    pub exists: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Locked, with git's reason; an empty string when it recorded none.
    pub locked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Prunable, with git's reason.
    pub prunable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Uncommitted or untracked content. Absent when the status was not asked for — it
    /// costs one subprocess per work tree, and the layout rule does not need it.
    pub dirty: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// How many entries `git status --porcelain` printed, when it was asked.
    pub changes: Option<usize>,
    /// Whether the layout rule holds here.
    pub policy: PolicyStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Why the layout rule does not hold, in one line.
    pub policy_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where it would go, when it is in the wrong place and a safe destination exists.
    pub proposed_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The issue this worktree's name refers to, when the name carries one.
    pub issue: Option<String>,
}

/// Where the container is, and how that was decided.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RootReport {
    /// The primary checkout: what the container is named after.
    pub repository_root: String,
    /// The canonical container every linked worktree belongs under.
    pub worktree_root: String,
    /// The policy's suffix.
    pub suffix: String,
    /// The policy's placement strategy.
    pub strategy: String,
    /// The container exists on disk. It is created when the first worktree is created.
    pub exists: bool,
    /// The work tree the command ran in.
    pub current_worktree: String,
    /// Whether that was the primary checkout or a linked one.
    pub current_kind: WorktreeKind,
    /// The common git directory: this repository's identity.
    pub git_common_dir: String,
}

/// How many work trees are in each state.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeTallies {
    /// Every registered work tree, primary included.
    pub worktrees: usize,
    /// Linked work trees.
    pub linked: usize,
    /// Linked work trees where the layout rule holds.
    pub compliant: usize,
    /// Linked work trees outside the canonical root.
    pub violations: usize,
    /// Work trees git reports as prunable.
    pub prunable: usize,
    /// Work trees git reports as locked.
    pub locked: usize,
}

/// The whole topology with the policy applied: what `list`, `doctor` and the MCP resource
/// all read.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeReport {
    /// Where the container is and how that was decided.
    pub root: RootReport,
    /// The state of each state.
    pub tallies: WorktreeTallies,
    /// Every registered work tree, primary first.
    pub worktrees: Vec<WorktreeView>,
    /// What is wrong with the topology, in the order the work trees are listed. Empty is the
    /// healthy answer.
    pub violations: Vec<WorktreeViolation>,
    /// Whether a violation fails a check, only warns, or is not evaluated.
    pub enforcement: Level,
}

impl WorktreeReport {
    /// Does this report fail a gate? A violation under `warn` or `off` does not.
    pub fn fails(&self) -> bool {
        self.enforcement == Level::Error && !self.violations.is_empty()
    }
}

/// One thing wrong with the topology.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeViolation {
    /// The stable machine name, which is the error variant's name.
    pub code: String,
    /// The work tree involved.
    pub path: String,
    /// Its directory name.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch it holds.
    pub branch: Option<String>,
    /// Where linked work trees belong.
    pub expected_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where it would go, when a safe destination exists.
    pub proposed_path: Option<String>,
    /// What is wrong, in one line.
    pub reason: String,
    /// The command that repairs it.
    pub remedy: String,
}

/// The answer to "where am I and is that all right".
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StatusReport {
    /// The primary checkout.
    pub repository: String,
    /// The work tree the command ran in.
    pub current: String,
    /// Primary or linked.
    pub kind: WorktreeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch checked out here.
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commit checked out here.
    pub head: Option<String>,
    /// The canonical container.
    pub canonical_root: String,
    /// Whether this work tree is where it belongs.
    pub policy: PolicyStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Why not, when it is not.
    pub policy_reason: Option<String>,
    /// Uncommitted or untracked content here.
    pub dirty: bool,
    /// How many entries `git status --porcelain` printed.
    pub changes: usize,
    /// How many linked work trees of this repository are outside the container.
    pub repository_violations: usize,
}

/// What a create did.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateReport {
    /// Where it is.
    pub path: String,
    /// Its directory name.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch checked out in it.
    pub branch: Option<String>,
    /// Whether that branch was created by this command.
    pub branch_created: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commit it started from.
    pub base: Option<String>,
    /// The container it went into.
    pub worktree_root: String,
    /// Whether the container was created by this command.
    pub root_created: bool,
}

/// What a remove did.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RemoveReport {
    /// What was removed.
    pub path: String,
    /// Its directory name.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch it held. It still exists: removing a worktree never deletes a branch.
    pub branch: Option<String>,
    /// Whether `--force` was needed.
    pub forced: bool,
}

/// One move a migration would make, or made.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MigrationStep {
    /// Where it is now.
    pub from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where it would go. Absent when the move is blocked.
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch it holds.
    pub branch: Option<String>,
    /// Whether this step can be carried out as things stand.
    pub safe: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Why not, when it cannot.
    pub blocked_by: Option<String>,
    /// Whether this step was actually applied.
    pub applied: bool,
}

/// A migration, planned or applied.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MigrationPlan {
    /// The container everything moves into.
    pub worktree_root: String,
    /// One step per linked work tree outside the container.
    pub steps: Vec<MigrationStep>,
    /// Steps that can be carried out.
    pub safe: usize,
    /// Steps that cannot, with their reasons on the steps.
    pub blocked: usize,
    /// Whether anything was changed. `--plan` never changes anything.
    pub applied: bool,
}

/// What a prune did.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PruneReport {
    /// The work trees whose metadata git considered stale.
    pub pruned: Vec<String>,
    /// Whether anything was changed.
    pub applied: bool,
}

// ---------------------------------------------------------------- requests

/// What to create. Every field is optional except a way to name the thing.
#[derive(Debug, Clone, Default)]
pub struct CreateRequest {
    /// The label the directory name is derived from.
    pub name: Option<String>,
    /// The branch to check out or create. Defaults to the label.
    pub branch: Option<String>,
    /// The commit or ref the branch starts from. Defaults to HEAD.
    pub base: Option<String>,
    /// Check out a commit with no branch.
    pub detach: bool,
}

// ---------------------------------------------------------------- the service

#[derive(Debug)]
/// The repository's worktrees, decided once.
///
/// Built from a directory — any directory inside any work tree — and the canonical policy.
/// Everything it answers is derived from three git subprocesses taken at construction, plus
/// one `git status` per work tree when a caller asks for dirtiness.
pub struct WorktreeService {
    identity: RepositoryIdentity,
    policy: WorktreePolicy,
    root: ResolvedPath,
    policy_path: String,
}

impl WorktreeService {
    /// Resolve the repository from `start` and apply `policy`.
    pub fn open(start: &Path, policy: WorktreePolicy, policy_path: &str) -> Result<Self> {
        let identity = RepositoryIdentity::discover(start)?;
        policy.check_suffix(policy_path)?;
        let root = policy.canonical_root(&identity)?;
        Ok(WorktreeService {
            identity,
            policy,
            root,
            policy_path: policy_path.to_string(),
        })
    }

    /// The repository this service speaks for.
    pub fn identity(&self) -> &RepositoryIdentity {
        &self.identity
    }

    /// The policy it applies.
    pub fn policy(&self) -> &WorktreePolicy {
        &self.policy
    }

    /// The canonical container.
    pub fn canonical_root(&self) -> &ResolvedPath {
        &self.root
    }

    /// Where the container is and how that was decided.
    pub fn root_report(&self) -> RootReport {
        let current = self.identity.current_worktree();
        RootReport {
            repository_root: display(&self.identity.primary_worktree().path),
            worktree_root: display(&self.root.path),
            suffix: self.policy.root.suffix.clone(),
            strategy: "sibling".to_string(),
            exists: self.root.path.is_dir(),
            current_worktree: display(&current.path),
            current_kind: if current.same_as(self.identity.primary_worktree()) {
                WorktreeKind::Primary
            } else {
                WorktreeKind::Linked
            },
            git_common_dir: display(&self.identity.git_common_dir().path),
        }
    }

    /// The whole topology with the policy applied. `with_status` costs one `git status` per
    /// existing work tree; the layout rule never needs it, so doctor does not pay for it.
    pub fn report(&self, with_status: bool) -> Result<WorktreeReport> {
        let mut worktrees = Vec::new();
        let mut violations = Vec::new();
        let mut tallies = WorktreeTallies {
            worktrees: 0,
            linked: 0,
            compliant: 0,
            violations: 0,
            prunable: 0,
            locked: 0,
        };
        // Names already claimed inside the container, so a migration plan never proposes a
        // destination that is taken.
        let claimed: BTreeMap<String, PathBuf> = self
            .identity
            .registered_worktrees()
            .iter()
            .filter(|r| ResolvedPath::of(&r.path).is_directly_inside(&self.root))
            .map(|r| (r.name(), r.path.clone()))
            .collect();

        for record in self.identity.registered_worktrees() {
            let resolved = ResolvedPath::of(&record.path);
            let is_primary = self.identity.is_primary(record);
            let kind = if is_primary {
                WorktreeKind::Primary
            } else {
                WorktreeKind::Linked
            };
            let exists = record.path.is_dir();
            let (dirty, changes) = if with_status && exists {
                match self.dirtiness(&record.path) {
                    Ok((d, c)) => (Some(d), Some(c)),
                    Err(_) => (None, None),
                }
            } else {
                (None, None)
            };

            let (policy, reason, proposed) = self.judge(record, &resolved, is_primary, &claimed);

            tallies.worktrees += 1;
            if !is_primary {
                tallies.linked += 1;
            }
            match policy {
                PolicyStatus::Pass => tallies.compliant += 1,
                PolicyStatus::Fail => tallies.violations += 1,
                PolicyStatus::Exempt => {}
            }
            if record.prunable.is_some() {
                tallies.prunable += 1;
            }
            if record.locked.is_some() {
                tallies.locked += 1;
            }

            if policy == PolicyStatus::Fail {
                let reason = reason.clone().unwrap_or_default();
                violations.push(WorktreeViolation {
                    code: self.violation_code(record, &resolved),
                    path: display(&record.path),
                    name: record.name(),
                    branch: record.branch.clone(),
                    expected_root: display(&self.root.path),
                    proposed_path: proposed.clone(),
                    reason,
                    remedy: "majordomus worktree migrate --plan".to_string(),
                });
            }

            worktrees.push(WorktreeView {
                path: display(&record.path),
                name: record.name(),
                kind,
                branch: record.branch.clone(),
                head: record.head.clone(),
                detached: record.detached,
                current: self.identity.is_current(record),
                exists,
                locked: record.locked.clone(),
                prunable: record.prunable.clone(),
                dirty,
                changes,
                policy,
                policy_reason: reason,
                proposed_path: proposed,
                issue: issue_of(&record.name()),
            });
        }

        Ok(WorktreeReport {
            root: self.root_report(),
            tallies,
            worktrees,
            violations,
            enforcement: self.policy.enforcement.outside_root,
        })
    }

    /// Is this work tree where it belongs, and if not, where would it go?
    fn judge(
        &self,
        record: &WorktreeRecord,
        resolved: &ResolvedPath,
        is_primary: bool,
        claimed: &BTreeMap<String, PathBuf>,
    ) -> (PolicyStatus, Option<String>, Option<String>) {
        // The primary checkout is the thing the container is named after. It is exempt by
        // definition, and counting it as a violation would be a bug that fires everywhere.
        if is_primary {
            return (PolicyStatus::Exempt, None, None);
        }
        if self.policy.enforcement.outside_root == Level::Off {
            return (PolicyStatus::Exempt, None, None);
        }
        if resolved.same_as(&self.root) {
            return (
                PolicyStatus::Fail,
                Some(format!(
                    "it is the canonical root {} itself; the container holds worktrees and cannot be one",
                    display(&self.root.path)
                )),
                None,
            );
        }
        if resolved.is_directly_inside(&self.root) {
            return (PolicyStatus::Pass, None, None);
        }
        let reason = if resolved.is_inside(&self.root) {
            format!(
                "it is nested inside {} rather than being one of its directories",
                display(&self.root.path)
            )
        } else {
            format!(
                "it is outside the canonical root {}",
                display(&self.root.path)
            )
        };
        // A destination is only proposed when it is free: a plan that proposes a collision
        // is a plan that fails halfway through.
        let proposed = WorktreeName::derive(&record.name()).ok().and_then(|n| {
            if claimed.contains_key(n.as_str()) {
                None
            } else {
                Some(display(&self.root.path.join(n.as_str())))
            }
        });
        (PolicyStatus::Fail, Some(reason), proposed)
    }

    fn violation_code(&self, _record: &WorktreeRecord, resolved: &ResolvedPath) -> String {
        if resolved.same_as(&self.root) {
            "RootIsAWorktree".to_string()
        } else {
            "OutsideCanonicalRoot".to_string()
        }
    }

    /// The current context, answered in one line each.
    pub fn status(&self) -> Result<StatusReport> {
        let report = self.report(false)?;
        let current = self
            .identity
            .registered_worktrees()
            .iter()
            .find(|r| self.identity.is_current(r));
        let view = report.worktrees.iter().find(|v| v.current);
        let (dirty, changes) = self
            .dirtiness(&self.identity.current_worktree().path)
            .unwrap_or((false, 0));
        Ok(StatusReport {
            repository: display(&self.identity.primary_worktree().path),
            current: display(&self.identity.current_worktree().path),
            kind: view.map(|v| v.kind).unwrap_or(WorktreeKind::Linked),
            branch: current.and_then(|r| r.branch.clone()),
            head: current.and_then(|r| r.head.clone()),
            canonical_root: display(&self.root.path),
            policy: view.map(|v| v.policy).unwrap_or(PolicyStatus::Exempt),
            policy_reason: view.and_then(|v| v.policy_reason.clone()),
            dirty,
            changes,
            repository_violations: report.violations.len(),
        })
    }

    /// Uncommitted or untracked content, and how many entries said so.
    fn dirtiness(&self, worktree: &Path) -> Result<(bool, usize)> {
        let text = git::status_porcelain(worktree)?;
        let n = text.lines().filter(|l| !l.trim().is_empty()).count();
        Ok((n > 0, n))
    }

    // ------------------------------------------------------------ selectors

    /// Resolve a selector to exactly one registered work tree.
    ///
    /// Three exact forms, in order: a path, a directory name, a branch name. Nothing is
    /// matched by prefix, substring or similarity — these selectors are given to commands
    /// that delete things, and a command that deletes must not guess. Two matches is an
    /// error naming both, never a choice made on the caller's behalf.
    pub fn resolve(&self, selector: &str) -> Result<WorktreeRecord> {
        let records = self.identity.registered_worktrees();
        let wanted = ResolvedPath::of(PathBuf::from(selector));
        let by_path: Vec<&WorktreeRecord> = records
            .iter()
            .filter(|r| ResolvedPath::of(&r.path).same_as(&wanted))
            .collect();
        if !by_path.is_empty() {
            return one(selector, by_path);
        }
        let by_name: Vec<&WorktreeRecord> =
            records.iter().filter(|r| r.name() == selector).collect();
        if !by_name.is_empty() {
            return one(selector, by_name);
        }
        let by_branch: Vec<&WorktreeRecord> = records
            .iter()
            .filter(|r| r.branch.as_deref() == Some(selector))
            .collect();
        if !by_branch.is_empty() {
            return one(selector, by_branch);
        }
        Err(WorktreeError::NoSuchWorktree {
            selector: selector.to_string(),
        })
    }

    /// The path of the worktree a selector names: what `cd "$(...)"` consumes.
    pub fn path_of(&self, selector: &str) -> Result<PathBuf> {
        Ok(self.resolve(selector)?.path)
    }

    // ------------------------------------------------------------ create

    /// Create a worktree under the canonical root.
    ///
    /// The destination is derived and never given: a caller says what to work on, and the
    /// policy says where it goes. Everything that could go wrong is checked before git is
    /// asked to do anything, so a refusal leaves the repository exactly as it was.
    pub fn create(&self, request: &CreateRequest) -> Result<CreateReport> {
        let label = request
            .name
            .clone()
            .or_else(|| request.branch.clone())
            .ok_or_else(|| WorktreeError::InvalidWorktreeName {
                given: String::new(),
                reason: "no name and no branch was given, so there is nothing to name it after"
                    .into(),
            })?;
        let name = WorktreeName::derive(&label)?;
        let target = self.root.path.join(name.as_str());

        let branch = if request.detach {
            None
        } else {
            Some(
                request
                    .branch
                    .clone()
                    .unwrap_or_else(|| super::naming::default_branch_for(&label)),
            )
        };

        // One lock around the whole check-then-act. Two agents creating the same worktree at
        // once is the case this exists for: without it, both pass the collision check and
        // one of them half-registers.
        let _lock = WorktreeLock::acquire(&self.identity.git_common_dir().path)?;

        // The topology may have changed between `open` and here, so the collision checks read
        // it again — under the lock, which is what makes them meaningful.
        let records = topology::read(&self.identity.primary_worktree().path)?;
        let resolved_target = ResolvedPath::of(&target);
        if let Some(existing) = records
            .iter()
            .find(|r| ResolvedPath::of(&r.path).same_as(&resolved_target))
        {
            return Err(WorktreeError::WorktreeAlreadyExists {
                path: existing.path.clone(),
                name: existing.name(),
                branch: existing.branch.clone(),
            });
        }
        if target.exists() || target.symlink_metadata().is_ok() {
            return Err(WorktreeError::TargetCollision { path: target });
        }
        if let Some(b) = branch.as_deref() {
            if !git::branch_name_is_valid(&self.identity.primary_worktree().path, b)? {
                return Err(WorktreeError::InvalidBranchName {
                    given: b.to_string(),
                    reason:
                        "git does not accept it as a reference name (git check-ref-format refs/heads/<name>)"
                            .into(),
                });
            }
            if let Some(other) = records.iter().find(|r| r.branch.as_deref() == Some(b)) {
                return Err(WorktreeError::BranchAlreadyCheckedOut {
                    branch: b.to_string(),
                    path: other.path.clone(),
                });
            }
        }
        let primary = self.identity.primary_worktree().path.clone();
        if let Some(base) = request.base.as_deref() {
            if !git::rev_exists(&primary, base)? {
                return Err(WorktreeError::BaseDoesNotExist {
                    base: base.to_string(),
                });
            }
        }

        let root_created = self.ensure_root()?;

        let branch_exists = match branch.as_deref() {
            Some(b) => git::branch_exists(&primary, b)?,
            None => false,
        };
        let mut args: Vec<String> = vec!["worktree".into(), "add".into()];
        let mut branch_created = false;
        match (&branch, branch_exists) {
            (None, _) => {
                args.push("--detach".into());
                args.push("--".into());
                args.push(target.to_string_lossy().to_string());
                if let Some(base) = &request.base {
                    args.push(base.clone());
                }
            }
            (Some(b), true) => {
                // The branch is there: check it out, do not move it.
                args.push("--".into());
                args.push(target.to_string_lossy().to_string());
                args.push(b.clone());
            }
            (Some(b), false) => {
                args.push("-b".into());
                args.push(b.clone());
                args.push("--".into());
                args.push(target.to_string_lossy().to_string());
                if let Some(base) = &request.base {
                    args.push(base.clone());
                }
                branch_created = true;
            }
        }
        git::run(&primary, &args)?;

        // Verify rather than assume: git said it worked, and the answer this command gives
        // is what the topology says, not what was intended.
        let after = topology::read(&primary)?;
        let registered = after
            .iter()
            .find(|r| ResolvedPath::of(&r.path).same_as(&resolved_target));
        let Some(registered) = registered else {
            return Err(WorktreeError::MigrationUnsafe {
                path: target,
                reason: "git reported success and `git worktree list` does not show it; \
                         the repository metadata is inconsistent, and nothing was cleaned up \
                         automatically because that would risk deleting real work"
                    .into(),
            });
        };
        if !ResolvedPath::of(&registered.path).is_directly_inside(&self.root) {
            return Err(WorktreeError::OutsideCanonicalRoot {
                path: registered.path.clone(),
                root: self.root.path.clone(),
            });
        }

        Ok(CreateReport {
            path: display(&registered.path),
            name: registered.name(),
            branch: registered.branch.clone(),
            branch_created,
            base: request.base.clone(),
            worktree_root: display(&self.root.path),
            root_created,
        })
    }

    /// Make sure the container exists, and answer whether this call created it.
    ///
    /// The container is outside the repository and cannot be committed, so it is created
    /// lazily — the first time a worktree needs it — rather than by `init`. The marker
    /// written beside it records which repository made it.
    fn ensure_root(&self) -> Result<bool> {
        let path = &self.root.path;
        if path.is_dir() {
            self.write_marker();
            return Ok(false);
        }
        if path.exists() || path.symlink_metadata().is_ok() {
            return Err(WorktreeError::RootNotADirectory { path: path.clone() });
        }
        std::fs::create_dir_all(path).map_err(|e| WorktreeError::io(path, &e))?;
        self.write_marker();
        Ok(true)
    }

    /// Best effort: the marker is evidence, and a container on a read-only filesystem is
    /// still a container.
    fn write_marker(&self) {
        let marker = self.root.path.join(ROOT_MARKER);
        let body = format!(
            "# Majordomus worktree container. Derived from the repository identity and \
             .ai/repo/policy.yaml; this file is evidence, never the source of truth.\n\
             repository: {}\ngit_common_dir: {}\nsuffix: {}\n",
            display(&self.identity.primary_worktree().path),
            display(&self.identity.git_common_dir().path),
            self.policy.root.suffix,
        );
        let _ = std::fs::write(marker, body);
    }

    /// Does the marker in the container name this repository? Evidence for a destructive
    /// operation, never a requirement: a container made before the marker existed, or by
    /// hand, is still the container the policy derives.
    pub fn marker_confirms(&self) -> Option<bool> {
        let text = std::fs::read_to_string(self.root.path.join(ROOT_MARKER)).ok()?;
        Some(text.lines().any(|l| {
            l.strip_prefix("git_common_dir:")
                .map(|v| {
                    ResolvedPath::of(PathBuf::from(v.trim()))
                        .same_as(self.identity.git_common_dir())
                })
                .unwrap_or(false)
        }))
    }

    // ------------------------------------------------------------ remove

    /// Remove one linked worktree. Never a branch, never the primary checkout, never a
    /// directory this repository does not own.
    pub fn remove(&self, selector: &str, force: bool) -> Result<RemoveReport> {
        let _lock = WorktreeLock::acquire(&self.identity.git_common_dir().path)?;
        let record = self.resolve(selector)?;
        let resolved = ResolvedPath::of(&record.path);

        // Three proofs before anything is deleted: it is ours, it is not the primary
        // checkout, and losing it is not losing work.
        self.assert_ours(&record)?;
        if resolved.same_as(self.identity.primary_worktree()) {
            return Err(WorktreeError::PrimaryWorktreeProtected {
                path: record.path.clone(),
                operation: "remove".into(),
            });
        }
        self.assert_not_locked(&record, force)?;
        self.assert_clean(&record, "remove", force)?;

        let mut args: Vec<String> = vec!["worktree".into(), "remove".into()];
        if force {
            args.push("--force".into());
        }
        args.push("--".into());
        args.push(record.path.to_string_lossy().to_string());
        git::run(&self.identity.primary_worktree().path, &args)?;

        Ok(RemoveReport {
            path: display(&record.path),
            name: record.name(),
            branch: record.branch.clone(),
            forced: force,
        })
    }

    // ------------------------------------------------------------ migrate

    /// What it would take to bring every linked worktree under the canonical root.
    ///
    /// Planning changes nothing, ever, and neither `doctor`, `init` nor `update` calls the
    /// applying form. A blocked step says why and is not proposed a destination.
    pub fn migration_plan(&self) -> Result<MigrationPlan> {
        let report = self.report(true)?;
        let mut steps = Vec::new();
        for view in report
            .worktrees
            .iter()
            .filter(|v| v.policy == PolicyStatus::Fail)
        {
            let record = self.resolve(&view.path)?;
            // Two independent reasons a step is not safe, and each is worth its own
            // sentence: the state of the work tree, and the state of the destination.
            let blocked = self.migration_blocker(&record, view).or_else(|| {
                if view.proposed_path.is_none() {
                    Some(format!(
                        "no free destination: '{}' is already taken inside {}; rename one of them first",
                        WorktreeName::derive(&view.name)
                            .map(|n| n.to_string())
                            .unwrap_or_else(|_| view.name.clone()),
                        display(&self.root.path)
                    ))
                } else {
                    None
                }
            });
            steps.push(MigrationStep {
                from: view.path.clone(),
                to: blocked
                    .is_none()
                    .then(|| view.proposed_path.clone())
                    .flatten(),
                branch: view.branch.clone(),
                safe: blocked.is_none(),
                blocked_by: blocked,
                applied: false,
            });
        }
        let safe = steps.iter().filter(|s| s.safe).count();
        Ok(MigrationPlan {
            worktree_root: display(&self.root.path),
            blocked: steps.len() - safe,
            safe,
            steps,
            applied: false,
        })
    }

    /// Why this worktree cannot be moved as things stand, if it cannot.
    fn migration_blocker(&self, record: &WorktreeRecord, view: &WorktreeView) -> Option<String> {
        if !view.exists {
            return Some(
                "the directory git registered does not exist; run `majordomus worktree prune` first"
                    .into(),
            );
        }
        if record.locked.is_some() {
            return Some(format!(
                "it is locked{}",
                record
                    .locked
                    .as_deref()
                    .filter(|r| !r.is_empty())
                    .map(|r| format!(" ({r})"))
                    .unwrap_or_default()
            ));
        }
        if self.policy.cleanup.dirty == Disposition::Refuse && view.dirty == Some(true) {
            return Some(format!(
                "it has {} uncommitted or untracked change(s); commit or stash them there first",
                view.changes.unwrap_or(0)
            ));
        }
        if view.kind == WorktreeKind::Primary {
            return Some("it is the primary checkout, which is never moved".into());
        }
        None
    }

    /// Carry out the safe steps of the plan. Recomputed under the lock, never replayed from
    /// a plan the caller is holding: the tree may have changed since it was printed.
    pub fn migrate(&self) -> Result<MigrationPlan> {
        let _lock = WorktreeLock::acquire(&self.identity.git_common_dir().path)?;
        let mut plan = self.migration_plan()?;
        self.ensure_root()?;
        let primary = self.identity.primary_worktree().path.clone();
        for step in plan.steps.iter_mut() {
            let (Some(to), true) = (step.to.clone(), step.safe) else {
                continue;
            };
            // The destination is derived, inside the container, and free. Prove the last
            // part again immediately before moving: planning and applying are not atomic
            // with respect to anything outside this lock.
            let to_path = PathBuf::from(&to);
            if to_path.exists() {
                step.safe = false;
                step.blocked_by = Some(format!("{to} appeared before the move could be made"));
                continue;
            }
            if !ResolvedPath::of(&to_path).is_directly_inside(&self.root) {
                step.safe = false;
                step.blocked_by = Some(format!(
                    "{to} is not directly inside {}",
                    display(&self.root.path)
                ));
                continue;
            }
            match git::run(
                &primary,
                &[
                    "worktree".to_string(),
                    "move".to_string(),
                    "--".to_string(),
                    step.from.clone(),
                    to,
                ],
            ) {
                Ok(_) => step.applied = true,
                Err(e) => {
                    step.safe = false;
                    step.blocked_by = Some(format!("git refused the move: {e}"));
                }
            }
        }
        plan.applied = true;
        plan.safe = plan.steps.iter().filter(|s| s.applied).count();
        plan.blocked = plan.steps.len() - plan.safe;
        Ok(plan)
    }

    // ------------------------------------------------------------ prune

    /// Ask git to drop the metadata of work trees whose directories are gone.
    ///
    /// This deletes no directory: `git worktree prune` removes administrative files under
    /// the common git directory for work trees that are already absent. `dry_run` reports
    /// what it would drop.
    pub fn prune(&self, dry_run: bool) -> Result<PruneReport> {
        let _lock = WorktreeLock::acquire(&self.identity.git_common_dir().path)?;
        let mut args: Vec<String> = vec!["worktree".into(), "prune".into(), "--verbose".into()];
        if dry_run {
            args.push("--dry-run".into());
        }
        let out = git::run(&self.identity.primary_worktree().path, &args)?;
        let text = String::from_utf8_lossy(&out.stdout);
        Ok(PruneReport {
            pruned: text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect(),
            applied: !dry_run,
        })
    }

    // ------------------------------------------------------------ guards

    /// Prove that a path is a work tree of *this* repository before anything touches it.
    fn assert_ours(&self, record: &WorktreeRecord) -> Result<()> {
        let known = self
            .identity
            .registered_worktrees()
            .iter()
            .any(|r| ResolvedPath::of(&r.path).same_as(&ResolvedPath::of(&record.path)));
        if known {
            Ok(())
        } else {
            Err(WorktreeError::RepositoryIdentityMismatch {
                path: record.path.clone(),
                git_common_dir: self.identity.git_common_dir().path.clone(),
            })
        }
    }

    fn assert_not_locked(&self, record: &WorktreeRecord, force: bool) -> Result<()> {
        match &record.locked {
            Some(reason) if self.policy.cleanup.locked == Disposition::Refuse && !force => {
                Err(WorktreeError::LockedWorktree {
                    path: record.path.clone(),
                    reason: Some(reason.clone()),
                })
            }
            _ => Ok(()),
        }
    }

    fn assert_clean(&self, record: &WorktreeRecord, operation: &str, force: bool) -> Result<()> {
        if self.policy.cleanup.dirty != Disposition::Refuse || force {
            return Ok(());
        }
        if !record.path.is_dir() {
            return Ok(());
        }
        let (dirty, n) = self.dirtiness(&record.path)?;
        if !dirty {
            return Ok(());
        }
        Err(WorktreeError::DirtyWorktree {
            path: record.path.clone(),
            summary: format!("{n} uncommitted or untracked change(s)"),
            operation: operation.to_string(),
        })
    }

    /// The policy file this service was configured from, for a message that says where to edit.
    pub fn policy_path(&self) -> &str {
        &self.policy_path
    }
}

fn one(selector: &str, matches: Vec<&WorktreeRecord>) -> Result<WorktreeRecord> {
    if matches.len() == 1 {
        return Ok(matches[0].clone());
    }
    Err(WorktreeError::AmbiguousSelector {
        selector: selector.to_string(),
        candidates: matches.iter().map(|r| r.path.clone()).collect(),
    })
}

/// A path as text. Absolute, as git holds it; nothing is shortened or made relative, because
/// these strings are copied into commands.
fn display(p: &Path) -> String {
    p.display().to_string()
}

/// The issue a worktree's name refers to, when the name follows the `issue-<id>-...` form
/// this repository's branches use. Derived from the name and from nothing else: no record is
/// read, no number is invented, and a name that carries none answers nothing.
///
/// ```
/// use majordomus_cli::worktree::issue_of;
/// assert_eq!(issue_of("issue-123-worktrees").as_deref(), Some("123"));
/// assert_eq!(issue_of("issue-I0042-graph").as_deref(), Some("I0042"));
/// assert_eq!(issue_of("feature-foo"), None);
/// ```
pub fn issue_of(name: &str) -> Option<String> {
    let rest = name.strip_prefix("issue-")?;
    let id = rest.split('-').next()?;
    if id.is_empty() || !id.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(id.to_string())
}
