//! Bringing misplaced worktrees home, transactionally and losslessly.
//!
//! A plan is computed from the topology and changes nothing. Applying it recomputes the plan
//! under the repository lock, then for each movable step takes a
//! [`WorktreeFingerprint`] of the worktree, asks git to move it, takes another at the
//! destination and refuses to call the step a success unless the two are equal. A dirty
//! worktree is not an error: it is the case this exists for, and `git worktree move` moves
//! the directory — modified, staged and untracked files included — with one rename.
//!
//! Two shapes need more than a rename. A worktree that occupies the container path itself
//! is moved out to a staging path beside the container, the container is created, and the
//! worktree is moved into it. A move that crosses filesystems, which `rename` cannot do, is
//! made by copying the tree, repairing git's administrative link, verifying the copy —
//! including a manifest of every entry of the directory tree — and only then removing the
//! original. Nothing is ever reset, stashed, cleaned or checked out.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::error::{Result, WorktreeError};
use super::fingerprint::{self, WorktreeFingerprint};
use super::git;
use super::identity::ResolvedPath;
use super::lock::WorktreeLock;
use super::model::{DiagnosticCode, DirtyState, Severity, Standing, TopologyDiagnostic, SCHEMA};
use super::service::{display, Detail, WorktreeService};

/// How a step moves its worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MigrationAction {
    /// One `git worktree move`.
    Move,
    /// The worktree is the container itself: out to a staging path, then into the container.
    MoveViaStaging,
    /// The move crossed devices: copy, `git worktree repair`, verify, remove the original.
    CopyAndRepair,
    /// Nothing: the step is blocked.
    None,
}

/// What became of a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepOutcome {
    /// Planned and not applied.
    Planned,
    /// Not carried out, with the blockers on the step.
    Blocked,
    /// Moved, and the fingerprint after equals the one before.
    Moved,
    /// Attempted and not completed, or completed and not verified; the message says which.
    Failed,
}

/// One worktree to bring home.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MigrationStep {
    /// The branch it holds.
    pub branch: String,
    /// Where it is.
    pub from: String,
    /// Where it belongs.
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commit checked out.
    pub head: Option<String>,
    /// How it moves.
    pub action: MigrationAction,
    /// Its uncommitted work, which moves with it.
    pub dirty: DirtyState,
    /// Why it cannot move, when it cannot.
    pub blockers: Vec<TopologyDiagnostic>,
    /// What happened.
    pub outcome: StepOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// What happened, in words, when it was not simply moved.
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The fingerprint before the move.
    pub before: Option<WorktreeFingerprint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The fingerprint after it.
    pub after: Option<WorktreeFingerprint>,
    /// What differs between the two; empty when verified.
    pub differences: Vec<String>,
}

/// A migration, planned or applied.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MigrationPlan {
    /// [`SCHEMA`].
    pub schema: String,
    /// The container everything moves into.
    pub container: String,
    /// One step per misplaced worktree with a branch, container occupants first.
    pub steps: Vec<MigrationStep>,
    /// Worktrees the migration cannot address by design: detached, missing, the primary
    /// checkout off the trunk. Each says what a person does about it.
    pub exceptions: Vec<TopologyDiagnostic>,
    /// Steps that can be carried out as things stand.
    pub movable: usize,
    /// Steps that cannot.
    pub blocked: usize,
    /// Steps carried out and verified.
    pub moved: usize,
    /// Steps attempted and not verified.
    pub failed: usize,
    /// Whether anything was changed.
    pub applied: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The new path of the worktree the command was run from, when that one moved.
    pub moved_current: Option<String>,
}

/// What an apply may do beyond a rename.
#[derive(Debug, Clone, Default)]
pub struct MigrationOptions {
    /// Fall back to copy, repair, verify and remove when a move crosses devices.
    pub allow_copy: bool,
    /// Only these branches.
    pub only: Vec<String>,
    /// Also move the scratch checkouts of sessions (`Standing::Ephemeral`), which are
    /// otherwise reported and left alone.
    pub include_ephemeral: bool,
}

/// The plan: every misplaced worktree with a branch, and what stands in its way. Changes
/// nothing. Ephemeral worktrees are exceptions, not steps.
pub fn plan(service: &WorktreeService) -> Result<MigrationPlan> {
    plan_with(service, false)
}

/// The plan, optionally counting ephemeral worktrees as steps.
pub fn plan_with(service: &WorktreeService, include_ephemeral: bool) -> Result<MigrationPlan> {
    let topology = service.topology(Detail::Full)?;
    let container = service.container().path.clone();
    let mut steps = Vec::new();
    let mut exceptions = Vec::new();
    for w in &topology.worktrees {
        match w.standing {
            Standing::Ephemeral if !include_ephemeral => {
                exceptions.extend(w.diagnostics.iter().cloned());
            }
            Standing::Misplaced | Standing::Ephemeral => {
                let (Some(branch), Some(to)) = (&w.branch, &w.expected_path) else {
                    exceptions.extend(w.diagnostics.iter().cloned());
                    continue;
                };
                let occupies_container = w
                    .diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::ContainerOccupied);
                let blockers: Vec<TopologyDiagnostic> = w
                    .diagnostics
                    .iter()
                    .filter(|d| {
                        matches!(
                            d.code,
                            DiagnosticCode::Locked
                                | DiagnosticCode::DestinationConflict
                                | DiagnosticCode::InvalidBranchName
                                | DiagnosticCode::PathEscape
                        )
                    })
                    .cloned()
                    .collect();
                steps.push(MigrationStep {
                    branch: branch.clone(),
                    from: w.path.clone(),
                    to: to.clone(),
                    head: w.head.clone(),
                    action: if !blockers.is_empty() {
                        MigrationAction::None
                    } else if occupies_container {
                        MigrationAction::MoveViaStaging
                    } else {
                        MigrationAction::Move
                    },
                    dirty: w.dirty.clone().unwrap_or_default(),
                    outcome: if blockers.is_empty() {
                        StepOutcome::Planned
                    } else {
                        StepOutcome::Blocked
                    },
                    blockers,
                    message: None,
                    before: None,
                    after: None,
                    differences: Vec::new(),
                });
            }
            Standing::Missing | Standing::Detached => {
                exceptions.extend(w.diagnostics.iter().cloned());
            }
            Standing::Primary => {
                exceptions.extend(
                    w.diagnostics
                        .iter()
                        .filter(|d| d.code == DiagnosticCode::PrimaryOnNonTrunk)
                        .cloned(),
                );
            }
            Standing::Canonical => {}
        }
    }
    // The container's occupant moves first: every other destination is inside the
    // container, and while a worktree *is* the container those destinations would be
    // created inside that worktree. If it cannot move, nothing can.
    steps.sort_by_key(|s| s.action != MigrationAction::MoveViaStaging);
    let occupant_blocked = steps
        .iter()
        .any(|s| s.outcome == StepOutcome::Blocked && s.from == display(&container));
    if occupant_blocked {
        for s in steps.iter_mut().filter(|s| s.from != display(&container)) {
            s.blockers.push(TopologyDiagnostic {
                code: DiagnosticCode::ContainerOccupied,
                severity: Severity::Error,
                path: Some(s.from.clone()),
                branch: Some(s.branch.clone()),
                expected: Some(s.to.clone()),
                message: format!(
                    "the container {} is occupied by a worktree that cannot move yet",
                    display(&container)
                ),
                remedy: "unblock the container's occupant first".into(),
            });
            s.action = MigrationAction::None;
            s.outcome = StepOutcome::Blocked;
        }
    }
    let movable = steps
        .iter()
        .filter(|s| s.outcome == StepOutcome::Planned)
        .count();
    Ok(MigrationPlan {
        schema: SCHEMA.to_string(),
        container: display(&container),
        blocked: steps.len() - movable,
        movable,
        moved: 0,
        failed: 0,
        steps,
        exceptions,
        applied: false,
        moved_current: None,
    })
}

/// Carry out the plan. Recomputed under the lock, never replayed from a plan the caller is
/// holding: the tree may have changed since it was printed.
pub fn apply(service: &WorktreeService, options: &MigrationOptions) -> Result<MigrationPlan> {
    let _lock = WorktreeLock::acquire(&service.identity().git_common_dir().path)?;
    let mut plan = plan_with(service, options.include_ephemeral)?;
    let current = service.identity().current_worktree().clone();
    let primary = service.identity().primary_worktree().path.clone();
    let container = service.container().path.clone();

    for step in plan.steps.iter_mut() {
        if step.outcome != StepOutcome::Planned {
            continue;
        }
        if !options.only.is_empty() && !options.only.contains(&step.branch) {
            step.outcome = StepOutcome::Blocked;
            step.message = Some("not selected".into());
            continue;
        }
        // Prove the destination is still free immediately before moving: planning and
        // applying are not atomic with respect to anything outside this lock.
        let to = PathBuf::from(&step.to);
        if step.action != MigrationAction::MoveViaStaging && std::fs::symlink_metadata(&to).is_ok()
        {
            step.outcome = StepOutcome::Failed;
            step.message = Some(format!(
                "{} appeared before the move could be made",
                step.to
            ));
            continue;
        }
        if !ResolvedPath::of(&to).is_inside(service.container()) {
            step.outcome = StepOutcome::Failed;
            step.message = Some(format!("{} is not inside {}", step.to, display(&container)));
            continue;
        }
        let from = PathBuf::from(&step.from);
        let was_current = ResolvedPath::of(&from).same_as(&current);
        match execute(service, &primary, &container, step, &from, &to, options) {
            Ok(()) => {
                if was_current {
                    plan.moved_current = Some(step.to.clone());
                }
            }
            Err(e) => {
                step.outcome = StepOutcome::Failed;
                step.message = Some(e.to_string());
            }
        }
    }
    plan.applied = true;
    plan.moved = plan
        .steps
        .iter()
        .filter(|s| s.outcome == StepOutcome::Moved)
        .count();
    plan.failed = plan
        .steps
        .iter()
        .filter(|s| s.outcome == StepOutcome::Failed)
        .count();
    plan.blocked = plan
        .steps
        .iter()
        .filter(|s| s.outcome == StepOutcome::Blocked)
        .count();
    plan.movable = 0;
    Ok(plan)
}

fn execute(
    service: &WorktreeService,
    primary: &Path,
    container: &Path,
    step: &mut MigrationStep,
    from: &Path,
    to: &Path,
    options: &MigrationOptions,
) -> Result<()> {
    let before = fingerprint::capture(from, false)?;
    step.before = Some(before.clone());
    match step.action {
        MigrationAction::Move => match worktree_move(primary, from, to) {
            Ok(()) => {}
            Err(e) if is_cross_device(&e) => {
                if !options.allow_copy {
                    return Err(WorktreeError::MigrationUnsafe {
                        path: from.to_path_buf(),
                        reason: format!(
                            "{e}; the destination is on another filesystem, and a copy was not \
                             permitted (majordomus worktree migrate --allow-copy)"
                        ),
                    });
                }
                step.action = MigrationAction::CopyAndRepair;
                let before_tree = fingerprint::capture(from, true)?;
                step.before = Some(before_tree.clone());
                copy_and_repair(primary, from, to)?;
                let after_tree = fingerprint::capture(to, true)?;
                let differences = fingerprint::differences(&before_tree, &after_tree);
                step.after = Some(after_tree);
                step.differences = differences.clone();
                if !differences.is_empty() {
                    step.outcome = StepOutcome::Failed;
                    step.message = Some(format!(
                        "the copy at {} does not match the original at {}, which was kept; \
                         git points at the copy (git worktree repair {} points it back)",
                        display(to),
                        display(from),
                        display(from)
                    ));
                    return Ok(());
                }
                // verified: the original is now an unregistered duplicate, and this is the one
                // place a directory is deleted — after the proof, never before it
                let mut identity = service.identity().clone();
                identity.refresh()?;
                if identity.record_at(&ResolvedPath::of(from)).is_some() {
                    return Err(WorktreeError::MigrationUnsafe {
                        path: from.to_path_buf(),
                        reason: "git still registers the original after the repair; both copies were kept".into(),
                    });
                }
                std::fs::remove_dir_all(from).map_err(|e| WorktreeError::io(from, &e))?;
                step.outcome = StepOutcome::Moved;
                step.message =
                    Some("moved by copy across filesystems, verified, original removed".into());
                return Ok(());
            }
            Err(e) => return Err(e),
        },
        MigrationAction::MoveViaStaging => {
            let staging = staging_path(container);
            worktree_move(primary, from, &staging)?;
            if std::fs::symlink_metadata(container).is_ok() {
                return Err(WorktreeError::MigrationUnsafe {
                    path: staging,
                    reason: format!(
                        "after moving the occupant out, {} still exists; the worktree waits at the staging path and the next migration moves it home",
                        display(container)
                    ),
                });
            }
            service.ensure_container()?;
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent).map_err(|e| WorktreeError::io(parent, &e))?;
            }
            worktree_move(primary, &staging, to)?;
        }
        MigrationAction::CopyAndRepair | MigrationAction::None => {
            return Err(WorktreeError::MigrationUnsafe {
                path: from.to_path_buf(),
                reason: "the step was not planned as a move".into(),
            })
        }
    }
    let after = fingerprint::capture(to, false)?;
    let differences = fingerprint::differences(&before, &after);
    step.after = Some(after);
    step.differences = differences.clone();
    let mut identity = service.identity().clone();
    identity.refresh()?;
    let registered = identity.record_at(&ResolvedPath::of(to)).is_some();
    if !differences.is_empty() || !registered {
        step.outcome = StepOutcome::Failed;
        step.message = Some(if registered {
            format!(
                "moved to {}, and the fingerprint differs: {}. A rename cannot lose content, so something wrote to the worktree during the move; inspect it there",
                display(to),
                differences.join("; ")
            )
        } else {
            format!("git reported the move and does not list {}", display(to))
        });
        return Ok(());
    }
    step.outcome = StepOutcome::Moved;
    let dangling = dangling_relative_links(to);
    if !dangling.is_empty() {
        step.message = Some(format!(
            "relative symbolic links that no longer resolve from the new path: {}",
            dangling.join(", ")
        ));
    }
    Ok(())
}

/// Top-level symbolic links of a worktree whose relative target does not resolve any more:
/// a `node_modules -> ../sibling/node_modules` written for the old path. Reported, never
/// rewritten; the link is the person's and the move did not change it.
fn dangling_relative_links(worktree: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(worktree) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.file_type().is_symlink() {
            continue;
        }
        let Ok(target) = std::fs::read_link(&path) else {
            continue;
        };
        if target.is_relative() && !worktree.join(&target).exists() {
            out.push(format!(
                "{} -> {}",
                entry.file_name().to_string_lossy(),
                target.display()
            ));
        }
    }
    out.sort();
    out
}

/// `git worktree move`, with absolute paths, from the primary checkout. Git resolves a
/// relative path against its working directory and matches a bare name against the suffix
/// of any registered worktree; absolute paths are the only form that says what it means.
fn worktree_move(primary: &Path, from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| WorktreeError::io(parent, &e))?;
    }
    git::run(
        primary,
        &[
            "worktree".to_string(),
            "move".to_string(),
            "--".to_string(),
            absolute(from),
            absolute(to),
        ],
    )?;
    Ok(())
}

fn absolute(p: &Path) -> String {
    if p.is_absolute() {
        display(p)
    } else {
        display(
            &std::env::current_dir()
                .map(|c| c.join(p))
                .unwrap_or_else(|_| p.to_path_buf()),
        )
    }
}

/// Where the container's occupant waits between its two moves.
fn staging_path(container: &Path) -> PathBuf {
    let name = container
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "worktrees".into());
    container
        .parent()
        .unwrap_or(container)
        .join(format!("{name}.migrating-{}", std::process::id()))
}

fn is_cross_device(e: &WorktreeError) -> bool {
    match e {
        WorktreeError::GitCommandFailed { stderr, .. } => {
            let s = stderr.to_ascii_lowercase();
            s.contains("cross-device") || s.contains("exdev")
        }
        _ => false,
    }
}

/// Copy the tree, point git at the copy, and leave the original in place for the caller to
/// verify against and remove.
fn copy_and_repair(primary: &Path, from: &Path, to: &Path) -> Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| WorktreeError::io(parent, &e))?;
    }
    copy_tree(from, to)?;
    git::run(
        primary,
        &["worktree".to_string(), "repair".to_string(), absolute(to)],
    )?;
    Ok(())
}

/// A faithful copy: directories, files with their permissions, symbolic links as links.
/// Nothing is followed and nothing is skipped.
pub fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir(to).map_err(|e| WorktreeError::io(to, &e))?;
    let mut stack = vec![(from.to_path_buf(), to.to_path_buf())];
    while let Some((src, dst)) = stack.pop() {
        let read = std::fs::read_dir(&src).map_err(|e| WorktreeError::io(&src, &e))?;
        for entry in read {
            let entry = entry.map_err(|e| WorktreeError::io(&src, &e))?;
            let s = entry.path();
            let d = dst.join(entry.file_name());
            let meta = std::fs::symlink_metadata(&s).map_err(|e| WorktreeError::io(&s, &e))?;
            if meta.file_type().is_symlink() {
                let target = std::fs::read_link(&s).map_err(|e| WorktreeError::io(&s, &e))?;
                #[cfg(unix)]
                std::os::unix::fs::symlink(&target, &d).map_err(|e| WorktreeError::io(&d, &e))?;
                #[cfg(not(unix))]
                std::fs::copy(&s, &d).map_err(|e| WorktreeError::io(&d, &e))?;
            } else if meta.is_dir() {
                std::fs::create_dir(&d).map_err(|e| WorktreeError::io(&d, &e))?;
                std::fs::set_permissions(&d, meta.permissions())
                    .map_err(|e| WorktreeError::io(&d, &e))?;
                stack.push((s, d));
            } else {
                std::fs::copy(&s, &d).map_err(|e| WorktreeError::io(&d, &e))?;
                std::fs::set_permissions(&d, meta.permissions())
                    .map_err(|e| WorktreeError::io(&d, &e))?;
            }
        }
    }
    Ok(())
}

/// The copy-based move as one operation, for a caller that knows the rename cannot be used:
/// copy, repair, verify with the whole-tree manifest, remove the original only when equal.
pub fn migrate_by_copy(service: &WorktreeService, from: &Path, to: &Path) -> Result<Vec<String>> {
    let primary = service.identity().primary_worktree().path.clone();
    let before = fingerprint::capture(from, true)?;
    copy_and_repair(&primary, from, to)?;
    let after = fingerprint::capture(to, true)?;
    let differences = fingerprint::differences(&before, &after);
    if differences.is_empty() {
        let mut identity = service.identity().clone();
        identity.refresh()?;
        if identity.record_at(&ResolvedPath::of(from)).is_none() {
            std::fs::remove_dir_all(from).map_err(|e| WorktreeError::io(from, &e))?;
        }
    }
    Ok(differences)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repository with a primary checkout and whatever linked worktrees a test asks for.
    ///
    /// The fixture lives under the temporary directory on purpose: `ephemeral_root_of`
    /// disables the temporary roots for a repository that is itself inside one, so a
    /// worktree placed outside the container is `Misplaced` rather than `Ephemeral`, and
    /// `.claude/worktrees/` remains the one rule that makes a checkout ephemeral. Without
    /// that, every worktree here would be a session's scratch and there would be nothing
    /// to plan.
    struct Repo {
        _home: tempfile::TempDir,
        primary: PathBuf,
    }

    impl Repo {
        fn new() -> Self {
            let home = tempfile::tempdir().expect("tempdir");
            let primary = home.path().join("dev/repo");
            std::fs::create_dir_all(&primary).expect("mkdir");
            let r = Repo {
                _home: home,
                primary,
            };
            r.git(&["init", "-q", "-b", "master", "."]);
            r.git(&["config", "user.email", "t@example.com"]);
            r.git(&["config", "user.name", "t"]);
            r.git(&["commit", "-q", "--allow-empty", "-m", "root"]);
            r
        }

        fn git(&self, args: &[&str]) {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(&self.primary)
                .args(args)
                .output()
                .expect("git runs")
                .status
                .success();
            assert!(ok, "git {args:?}");
        }

        /// A linked worktree for `branch`, at `at`.
        fn worktree(&self, branch: &str, at: &Path) {
            if let Some(parent) = at.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            self.git(&["worktree", "add", "-q", "-b", branch, &at.to_string_lossy()]);
        }

        fn service(&self) -> WorktreeService {
            WorktreeService::open(&self.primary).expect("a service over the fixture")
        }

        /// Where the worktree of `branch` belongs.
        fn canonical(&self, branch: &str) -> PathBuf {
            self.service()
                .expected_path_of(branch)
                .expect("a canonical path")
        }
    }

    #[test]
    fn a_worktree_already_home_is_neither_a_step_nor_an_exception() {
        let repo = Repo::new();
        repo.worktree("feature/done", &repo.canonical("feature/done"));
        let plan = plan(&repo.service()).expect("a plan");
        assert!(plan.steps.is_empty(), "{:?}", plan.steps);
        assert_eq!(plan.movable, 0);
        assert_eq!(plan.blocked, 0);
    }

    #[test]
    fn a_misplaced_worktree_is_a_step_that_moves_it_home() {
        let repo = Repo::new();
        let away = repo.primary.parent().unwrap().join("elsewhere");
        repo.worktree("feature/away", &away);

        let plan = plan(&repo.service()).expect("a plan");
        assert_eq!(plan.steps.len(), 1, "{:?}", plan.steps);
        let step = &plan.steps[0];
        assert_eq!(step.branch, "feature/away");
        assert_eq!(step.action, MigrationAction::Move);
        assert_eq!(step.outcome, StepOutcome::Planned);
        assert_eq!(step.to, display(&repo.canonical("feature/away")));
        assert_eq!(plan.movable, 1);
        assert_eq!(plan.blocked, 0);
        // a plan changes nothing: the worktree is still where it was
        assert!(away.is_dir(), "the plan moved something");
    }

    #[test]
    fn a_session_scratch_checkout_is_reported_and_left_alone_unless_it_is_asked_for() {
        // The guard this exists for: a worktree under the primary's .claude/worktrees/ is
        // a harness's own, and migration reports it rather than moving it out from under
        // whoever is working in it.
        let repo = Repo::new();
        let scratch = repo.primary.join(".claude/worktrees/scratch");
        repo.worktree("feature/scratch", &scratch);

        let default = plan(&repo.service()).expect("a plan");
        assert!(
            default.steps.is_empty(),
            "an ephemeral checkout is not a step: {:?}",
            default.steps
        );
        assert_eq!(default.movable, 0);

        // and it is not silently dropped either — it is reported as an exception
        assert!(
            !default.exceptions.is_empty(),
            "an ephemeral checkout is reported"
        );

        // asked for explicitly, it becomes a step like any other
        let asked = plan_with(&repo.service(), true).expect("a plan");
        assert_eq!(asked.steps.len(), 1, "{:?}", asked.steps);
        assert_eq!(asked.steps[0].branch, "feature/scratch");
        assert!(scratch.is_dir(), "neither plan moved anything");
    }

    #[test]
    fn a_worktree_with_no_branch_has_nowhere_to_go_and_is_an_exception() {
        let repo = Repo::new();
        let away = repo.primary.parent().unwrap().join("detached");
        std::fs::create_dir_all(away.parent().unwrap()).expect("mkdir");
        repo.git(&["worktree", "add", "-q", "--detach", &away.to_string_lossy()]);

        let plan = plan(&repo.service()).expect("a plan");
        assert!(
            plan.steps.is_empty(),
            "a detached worktree has no canonical path: {:?}",
            plan.steps
        );
    }

    #[test]
    fn the_containers_occupant_moves_first_and_blocks_the_rest_when_it_cannot() {
        // A worktree that *is* the container is the one shape that needs two moves, and
        // every other destination is inside it. The plan puts it first; and when it is
        // blocked, nothing else may proceed, because their destinations would be created
        // inside a directory that is about to move.
        let repo = Repo::new();
        let container = repo.service().container().path.clone();
        repo.worktree("feature/occupant", &container);
        let away = repo.primary.parent().unwrap().join("elsewhere");
        repo.worktree("feature/away", &away);

        let plan = plan(&repo.service()).expect("a plan");
        assert_eq!(plan.steps.len(), 2, "{:?}", plan.steps);
        assert_eq!(
            plan.steps[0].branch, "feature/occupant",
            "the occupant is planned first"
        );
        assert_eq!(plan.steps[0].action, MigrationAction::MoveViaStaging);
    }

    #[test]
    fn a_plan_over_a_repository_with_nothing_to_move_is_empty_and_valid() {
        let repo = Repo::new();
        let plan = plan(&repo.service()).expect("a plan");
        assert!(plan.steps.is_empty());
        assert!(!plan.applied);
        assert_eq!(plan.moved, 0);
        assert_eq!(plan.failed, 0);
        assert_eq!(plan.schema, SCHEMA);
        assert_eq!(plan.container, display(&repo.service().container().path));
    }

    #[test]
    fn applying_moves_the_worktree_home_and_keeps_every_file_including_the_dirty_ones() {
        // The transactional guarantee: a fingerprint before, git moves the directory, a
        // fingerprint after, and the step is a success only if the two agree. A dirty
        // worktree is the case this exists for — modified, staged and untracked files all
        // travel with one rename, and nothing is reset, stashed or checked out.
        let repo = Repo::new();
        let away = repo.primary.parent().unwrap().join("elsewhere");
        repo.worktree("feature/dirty", &away);

        // leave work of all three kinds in it
        std::fs::write(away.join("tracked.txt"), "committed\n").expect("write");
        let add = std::process::Command::new("git")
            .arg("-C")
            .arg(&away)
            .args(["add", "tracked.txt"])
            .output()
            .expect("git runs");
        assert!(add.status.success());
        let commit = std::process::Command::new("git")
            .arg("-C")
            .arg(&away)
            .args([
                "-c",
                "user.email=t@e.com",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "-m",
                "w",
            ])
            .output()
            .expect("git runs");
        assert!(commit.status.success());
        std::fs::write(away.join("tracked.txt"), "modified\n").expect("write");
        std::fs::write(away.join("untracked.txt"), "never added\n").expect("write");
        let staged = away.join("staged.txt");
        std::fs::write(&staged, "staged\n").expect("write");
        let add = std::process::Command::new("git")
            .arg("-C")
            .arg(&away)
            .args(["add", "staged.txt"])
            .output()
            .expect("git runs");
        assert!(add.status.success());

        let svc = repo.service();
        let home = svc.expected_path_of("feature/dirty").expect("a path");
        let applied = apply(&svc, &MigrationOptions::default()).expect("the plan applies");

        assert!(applied.applied, "the plan says it ran");
        assert_eq!(applied.moved, 1, "{:?}", applied.steps);
        assert_eq!(applied.failed, 0, "{:?}", applied.steps);
        assert_eq!(applied.steps[0].outcome, StepOutcome::Moved);

        // the worktree is home, and the old path is gone
        assert!(home.is_dir(), "the worktree did not arrive");
        assert!(!away.exists(), "the worktree did not leave");

        // every file travelled, dirty ones included
        assert_eq!(
            std::fs::read_to_string(home.join("tracked.txt")).expect("tracked"),
            "modified\n",
            "an unstaged modification was reset"
        );
        assert_eq!(
            std::fs::read_to_string(home.join("untracked.txt")).expect("untracked"),
            "never added\n",
            "an untracked file was cleaned"
        );
        assert_eq!(
            std::fs::read_to_string(home.join("staged.txt")).expect("staged"),
            "staged\n"
        );

        // and git agrees the worktree is at its new path
        let listed = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo.primary)
            .args(["worktree", "list"])
            .output()
            .expect("git runs");
        let listed = String::from_utf8_lossy(&listed.stdout);
        assert!(
            listed.contains(&display(&home)),
            "git still points elsewhere: {listed}"
        );
    }

    #[test]
    fn only_the_selected_branch_moves_and_the_rest_say_why_not() {
        let repo = Repo::new();
        let parent = repo.primary.parent().unwrap().to_path_buf();
        repo.worktree("feature/one", &parent.join("one"));
        repo.worktree("feature/two", &parent.join("two"));

        let svc = repo.service();
        let options = MigrationOptions {
            only: vec!["feature/one".into()],
            ..MigrationOptions::default()
        };
        let applied = apply(&svc, &options).expect("the plan applies");

        assert_eq!(applied.moved, 1, "{:?}", applied.steps);
        let two = applied
            .steps
            .iter()
            .find(|s| s.branch == "feature/two")
            .expect("the unselected step is still reported");
        assert_eq!(two.outcome, StepOutcome::Blocked);
        assert_eq!(two.message.as_deref(), Some("not selected"));
        assert!(parent.join("two").is_dir(), "the unselected worktree moved");
    }

    #[test]
    fn a_session_scratch_checkout_is_not_moved_by_an_ordinary_apply() {
        // The guard, at the point where it matters: applying a migration must not relocate
        // a harness's own checkout out from under whoever is working in it.
        let repo = Repo::new();
        let scratch = repo.primary.join(".claude/worktrees/scratch");
        repo.worktree("feature/scratch", &scratch);

        let applied = apply(&repo.service(), &MigrationOptions::default()).expect("applies");
        assert_eq!(applied.moved, 0, "{:?}", applied.steps);
        assert!(scratch.is_dir(), "an ephemeral checkout was moved");
    }

    #[test]
    fn the_staging_path_sits_beside_the_container_and_names_the_process() {
        let staging = staging_path(Path::new("/dev/repo-wt"));
        assert_eq!(staging.parent(), Some(Path::new("/dev")));
        let name = staging.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("repo-wt.migrating-"), "{name}");
        assert!(
            name.ends_with(&std::process::id().to_string()),
            "two migrations at once do not share a staging path: {name}"
        );
    }

    #[test]
    fn a_cross_device_rename_is_recognised_from_what_git_said() {
        let exdev = WorktreeError::GitCommandFailed {
            command: "worktree move".into(),
            status: "exit status: 128".into(),
            stderr: "fatal: failed to rename: Invalid cross-device link".into(),
        };
        assert!(is_cross_device(&exdev));
        let other = WorktreeError::GitCommandFailed {
            command: "worktree move".into(),
            status: "exit status: 128".into(),
            stderr: "fatal: something else entirely".into(),
        };
        assert!(!is_cross_device(&other));
    }
}
