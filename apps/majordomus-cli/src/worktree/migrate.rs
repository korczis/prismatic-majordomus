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

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, content) in files {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        dir
    }

    /// A move that crosses filesystems is the one case `git worktree move` cannot do, and
    /// the only evidence of it is the wording of git's own stderr. Recognising it wrong in
    /// either direction is costly: missed, a person is told the move failed for no stated
    /// reason; claimed falsely, a rename that failed for some other reason — a permission
    /// problem, a destination that appeared — would be answered by copying the tree and
    /// deleting the original.
    #[test]
    fn only_gits_cross_device_wording_is_read_as_a_cross_device_move() {
        let git = |stderr: &str| WorktreeError::GitCommandFailed {
            command: "worktree move".into(),
            status: "128".into(),
            stderr: stderr.into(),
        };
        assert!(is_cross_device(&git(
            "fatal: failed to move: Invalid cross-device link"
        )));
        assert!(
            is_cross_device(&git("rename failed: EXDEV")),
            "the errno spelling is what some platforms print"
        );
        assert!(
            is_cross_device(&git("Cross-Device Link")),
            "the comparison is case-insensitive, because the wording is not ours"
        );
        assert!(!is_cross_device(&git("fatal: permission denied")));
        assert!(!is_cross_device(&git("")));
        assert!(
            !is_cross_device(&WorktreeError::Io {
                path: PathBuf::from("/a"),
                reason: "Invalid cross-device link".into(),
            }),
            "only git's own refusal is read this way; an IO error carrying the same words is not a failed `worktree move`"
        );
    }

    /// The staging path is where the container's own occupant waits between its two moves.
    /// It has to be *beside* the container and not under it — the container does not exist
    /// yet at that moment, and a staging path inside it would be a directory inside the
    /// worktree being moved — and it has to be unique to this process, or two migrations
    /// running at once would stage into one directory.
    #[test]
    fn the_staging_path_is_beside_the_container_and_belongs_to_this_process() {
        let container = Path::new("/a/foo-wt");
        let staging = staging_path(container);
        assert_eq!(staging.parent(), Some(Path::new("/a")));
        assert!(
            !staging.starts_with(container),
            "staging inside the container would put it inside the worktree being moved: {}",
            staging.display()
        );
        let name = staging.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("foo-wt.migrating-"), "{name}");
        assert!(
            name.ends_with(&std::process::id().to_string()),
            "two concurrent migrations must not share a staging path: {name}"
        );
        assert_eq!(
            staging_path(container),
            staging,
            "the same process derives the same path twice"
        );
    }

    /// A relative symbolic link written for the old path — `node_modules -> ../shared/node_modules`
    /// is the case this exists for — stops resolving when the worktree moves. It is reported
    /// and never rewritten. An absolute link and a link that still resolves are not findings:
    /// reporting them would bury the one that matters in noise.
    #[cfg(unix)]
    #[test]
    fn only_relative_links_that_stopped_resolving_are_reported_and_none_are_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        let wt = dir.path().join("worktree");
        std::fs::create_dir_all(wt.join("real")).unwrap();
        std::fs::write(wt.join("file"), "x").unwrap();

        std::os::unix::fs::symlink("real", wt.join("resolves")).unwrap();
        std::os::unix::fs::symlink("../gone/node_modules", wt.join("node_modules")).unwrap();
        std::os::unix::fs::symlink("../also-gone", wt.join("aaa")).unwrap();
        std::os::unix::fs::symlink("/definitely/not/here", wt.join("absolute")).unwrap();

        let found = dangling_relative_links(&wt);
        assert_eq!(
            found,
            vec![
                "aaa -> ../also-gone".to_string(),
                "node_modules -> ../gone/node_modules".to_string()
            ],
            "sorted, relative only, and only the ones that stopped resolving"
        );
        assert_eq!(
            std::fs::read_link(wt.join("node_modules")).unwrap(),
            PathBuf::from("../gone/node_modules"),
            "the link is the person's and the move did not change it"
        );
        assert!(
            dangling_relative_links(&dir.path().join("nothing-here")).is_empty(),
            "a directory that cannot be read is not a list of findings"
        );
    }

    /// The copy is the fallback a cross-device move takes, and the original is deleted after
    /// it. Anything the copy silently changed is work lost with no way back: a symbolic link
    /// followed into a duplicate of its target, an executable bit dropped from a script, a
    /// nested directory skipped. The fingerprint's tree manifest is what proves the copy,
    /// and it is asserted here against the copy this function actually makes.
    #[test]
    fn the_copy_reproduces_the_tree_exactly_enough_for_the_manifest_to_accept_it() {
        let src = tree(&[
            ("a.txt", "one"),
            ("deep/nested/b.txt", "two"),
            ("deep/c.txt", "three"),
        ]);
        std::fs::create_dir(src.path().join("empty")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::os::unix::fs::symlink("a.txt", src.path().join("link")).unwrap();
            let script = src.path().join("run.sh");
            std::fs::write(&script, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let into = tempfile::tempdir().unwrap();
        let dst = into.path().join("copy");
        copy_tree(src.path(), &dst).unwrap();

        assert_eq!(
            fingerprint::tree_manifest(&dst).unwrap(),
            fingerprint::tree_manifest(src.path()).unwrap(),
            "the manifest is what verifies a copy-based migration; if it differs the move is refused"
        );
        assert_eq!(
            std::fs::read_to_string(dst.join("deep/nested/b.txt")).unwrap(),
            "two"
        );
        assert!(
            dst.join("empty").is_dir(),
            "an empty directory is content too"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert!(
                std::fs::symlink_metadata(dst.join("link"))
                    .unwrap()
                    .file_type()
                    .is_symlink(),
                "a link copied as a file would double the bytes and lose the link"
            );
            assert_eq!(
                std::fs::read_link(dst.join("link")).unwrap(),
                PathBuf::from("a.txt")
            );
            assert_eq!(
                std::fs::metadata(dst.join("run.sh"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o755,
                "a script that lost its executable bit is a broken checkout"
            );
        }

        assert!(
            copy_tree(src.path(), &dst).is_err(),
            "copying onto a destination that already exists would merge two trees; it is refused"
        );
    }

    /// Git resolves a relative path against its own working directory and matches a bare
    /// name against the *suffix* of any registered worktree. Passing either to
    /// `git worktree move` is how a worktree ends up somewhere nobody asked for, so every
    /// path this module hands to git is made absolute first.
    #[test]
    fn every_path_handed_to_git_is_made_absolute_first() {
        assert_eq!(absolute(Path::new("/a/foo-wt/x")), "/a/foo-wt/x");
        let relative = absolute(Path::new("some/relative/path"));
        assert!(
            Path::new(&relative).is_absolute(),
            "a relative path reached git: {relative}"
        );
        assert!(relative.ends_with("some/relative/path"), "{relative}");
        assert!(
            relative.starts_with(&std::env::current_dir().unwrap().display().to_string()),
            "it is resolved against this process's working directory: {relative}"
        );
    }

    /// The action and the outcome are read by the Cockpit and by the JSON form of
    /// `worktree migrate --plan`, and `snake_case` is what those consumers match on. A
    /// renamed variant would still compile everywhere and silently change the wire form.
    #[test]
    fn the_action_and_the_outcome_serialise_under_the_names_consumers_match_on() {
        for (action, name) in [
            (MigrationAction::Move, "move"),
            (MigrationAction::MoveViaStaging, "move_via_staging"),
            (MigrationAction::CopyAndRepair, "copy_and_repair"),
            (MigrationAction::None, "none"),
        ] {
            assert_eq!(serde_json::to_value(action).unwrap(), name, "{action:?}");
        }
        for (outcome, name) in [
            (StepOutcome::Planned, "planned"),
            (StepOutcome::Blocked, "blocked"),
            (StepOutcome::Moved, "moved"),
            (StepOutcome::Failed, "failed"),
        ] {
            assert_eq!(serde_json::to_value(outcome).unwrap(), name, "{outcome:?}");
        }
    }
}
