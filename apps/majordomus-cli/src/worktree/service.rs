//! The worktree service: every decision about where a worktree belongs, what stands where,
//! whether a call is where it should be, and what may be created or removed.
//!
//! This is the only implementation. The command line renders what it answers, the
//! capability registry projects its read-only questions to MCP, HTTP, OpenAPI and the
//! Cockpit, the git hooks ask its guard, and the migration in [`super::migrate`] moves what
//! it judged misplaced. None of them decides anything; a second decision somewhere else is
//! the defect this file exists to prevent.
//!
//! One operation touches the network: creating a branch that does not exist here fetches
//! the trunk, and that branch, from the trunk's remote, so that new work starts from the
//! remote's trunk as it is now and a branch already pushed is taken up rather than started
//! again (see [`StartPoint`]). The fetch is bounded and quiet, and a remote that cannot be
//! read is said, with how far behind the local trunk stands. Nothing else fetches: a ref
//! that does not resolve locally does not resolve, and every other operation works with
//! the machine offline.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::direnv::{self, EnvrcApproval, LocalOverrides};
use super::error::{Result, WorktreeError};
use super::git;
use super::hooks::{self, HooksPath};
use super::identity::{RepositoryIdentity, ResolvedPath, TrunkSource};
use super::lock::WorktreeLock;
use super::model::{
    BranchState, ContainerView, DiagnosticCode, DirtyState, GuardVerdict, InspectReport,
    RepairReport, RepositoryTopology, RepositoryView, Severity, Standing, StatusReport,
    TopologyDiagnostic, TopologyTallies, TrunkView, WorktreeKind, WorktreeState, SCHEMA,
};
use super::path::{self, BranchName, CONTAINER_SUFFIX};
use super::state::{self, BranchRef};
use super::topology::WorktreeRecord;

/// How much of each work tree's state a topology reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    /// The registrations, the branches and the filesystem: no per-worktree subprocess.
    Fast,
    /// Also the uncommitted work of every existing work tree, one `git status` each.
    Full,
}

/// What to create.
#[derive(Debug, Clone, Default)]
pub struct CreateRequest {
    /// The branch to check out or create.
    pub branch: String,
    /// The commit or ref a new branch starts from, as given and never fetched. When absent,
    /// the remote's trunk as fetched now, then the local trunk, then HEAD ([`StartPoint`]).
    pub base: Option<String>,
}

/// Where a new branch started, and why there.
///
/// The local trunk of a long-lived checkout is routinely hundreds of commits behind its
/// remote, because nothing advances it; a branch cut from it carries none of that work and
/// every worker had to reset it by hand. So the default is the remote's trunk, fetched at
/// the moment of creation, and the local trunk only when the remote cannot be read — said,
/// never silent.
///
/// ```
/// use majordomus_cli::worktree::StartPoint;
/// let p = StartPoint::RemoteTrunk { remote: "origin".into(), branch: "master".into() };
/// assert_eq!(p.reference(), "origin/master");
/// assert!(p.fallback().is_none());
/// let fell = StartPoint::LocalTrunk {
///     branch: "master".into(),
///     unreachable: Some("origin".into()),
///     reason: Some("no answer within 60s".into()),
///     behind: Some(779),
/// };
/// assert_eq!(fell.reference(), "master");
/// assert!(fell.fallback().unwrap().contains("779 commits behind origin/master"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum StartPoint {
    /// The ref `--base` named, as given; never fetched.
    Given {
        /// The ref.
        reference: String,
    },
    /// The trunk on its remote, fetched by this command.
    RemoteTrunk {
        /// The remote.
        remote: String,
        /// The trunk branch.
        branch: String,
    },
    /// The branch already on the remote: the local branch is created at it and tracks it.
    RemoteBranch {
        /// The remote.
        remote: String,
        /// The branch, the same name on both sides.
        branch: String,
        /// Whether it was fetched now; `false` is the remote-tracking ref as last fetched,
        /// because the remote could not be read.
        fetched: bool,
    },
    /// The local trunk: the trunk has no remote, or its remote could not be read.
    LocalTrunk {
        /// The trunk branch.
        branch: String,
        /// The remote that could not be read; `None` when the trunk has no remote.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unreachable: Option<String>,
        /// Why it could not be read.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        /// How many commits the remote's trunk, as last fetched, has that the local trunk
        /// lacks; `None` when it was never fetched.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        behind: Option<u64>,
    },
    /// HEAD, because no trunk is known.
    Head,
}

impl StartPoint {
    /// The ref git is given to start the branch from.
    ///
    /// ```
    /// use majordomus_cli::worktree::StartPoint;
    ///
    /// let remote = StartPoint::RemoteTrunk { remote: "origin".into(), branch: "master".into() };
    /// assert_eq!(remote.reference(), "origin/master");
    /// assert_eq!(StartPoint::Head.reference(), "HEAD");
    /// ```
    pub fn reference(&self) -> String {
        match self {
            StartPoint::Given { reference } => reference.clone(),
            StartPoint::RemoteTrunk { remote, branch }
            | StartPoint::RemoteBranch { remote, branch, .. } => format!("{remote}/{branch}"),
            StartPoint::LocalTrunk { branch, .. } => branch.clone(),
            StartPoint::Head => "HEAD".into(),
        }
    }

    /// The sentence a person must read when the start is not the remote's state as it is
    /// now; `None` when it is, or when nothing better could have been asked.
    ///
    /// ```
    /// use majordomus_cli::worktree::StartPoint;
    ///
    /// let stale = StartPoint::LocalTrunk {
    ///     branch: "master".into(),
    ///     unreachable: Some("origin".into()),
    ///     reason: Some("offline".into()),
    ///     behind: Some(3),
    /// };
    /// let said = stale.fallback().expect("a stale start is said");
    /// assert!(said.contains("3 commits behind origin/master"), "{said}");
    /// let remote = StartPoint::RemoteTrunk { remote: "origin".into(), branch: "master".into() };
    /// assert_eq!(remote.fallback(), None);
    /// ```
    pub fn fallback(&self) -> Option<String> {
        match self {
            StartPoint::LocalTrunk {
                branch,
                unreachable: Some(remote),
                reason,
                behind,
            } => {
                let why = reason.as_deref().unwrap_or("it did not answer");
                let distance = match behind {
                    Some(0) => format!("level with {remote}/{branch} as last fetched"),
                    Some(1) => format!("1 commit behind {remote}/{branch} as last fetched"),
                    Some(n) => format!("{n} commits behind {remote}/{branch} as last fetched"),
                    None => format!("{remote}/{branch} was never fetched, so how far behind is unknown"),
                };
                Some(format!(
                    "could not fetch {branch} from {remote} ({why}): started from the local {branch}, {distance}; \
                     once {remote} answers, `git fetch {remote} && git reset --hard {remote}/{branch}` in the worktree before the first commit"
                ))
            }
            StartPoint::RemoteBranch {
                remote,
                branch,
                fetched: false,
            } => Some(format!(
                "could not fetch {branch} from {remote}: tracking {remote}/{branch} as last fetched, which may be behind"
            )),
            _ => None,
        }
    }
}

/// What a create did.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateReport {
    /// Where the worktree is.
    pub path: String,
    /// The branch checked out in it.
    pub branch: String,
    /// Whether that branch was created by this command.
    pub branch_created: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The ref a new branch started from (`origin/master`).
    pub base: Option<String>,
    /// Where a branch this command created started, and why there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<StartPoint>,
    /// The exact commit a branch this command created starts at, read back from the
    /// worktree rather than assumed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_commit: Option<String>,
    /// The container it went into.
    pub container: String,
    /// Whether the container was created by this command.
    pub container_created: bool,
    /// The worktree already existed at its canonical path and nothing was created.
    pub existed: bool,
    /// What became of its `.envrc` under direnv: the primary checkout's approval carried to
    /// this path, or why it was not. A worktree that starts blocked is the recurring
    /// "direnv does not work again"; this says so before the first `cd` does.
    pub envrc: EnvrcApproval,
    /// What became of the primary checkout's machine-local `.envrc.local` here.
    #[serde(default = "default_local")]
    pub envrc_local: LocalOverrides,
    /// Which hooks it runs: its own tree's, bound per worktree when the shared
    /// `core.hooksPath` would have sent it to the primary checkout's (I2718).
    #[serde(default = "default_hooks")]
    pub hooks: HooksPath,
}

/// The remote the trunk belongs to: the one whose HEAD names it, else the one the trunk's
/// configuration tracks. `None` when the trunk lives only here.
fn trunk_remote(primary: &Path, trunk: &str) -> Result<Option<String>> {
    let remotes = git::try_run(primary, &["remote"])?;
    if remotes.status != Some(0) {
        return Ok(None);
    }
    let remotes: Vec<String> = remotes
        .text()?
        .lines()
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(str::to_string)
        .collect();
    for remote in &remotes {
        // A remote with no HEAD, or one git cannot be asked about, names no trunk.
        let head = git::try_run(
            primary,
            &["symbolic-ref", "-q", &format!("refs/remotes/{remote}/HEAD")],
        )
        .ok()
        .filter(|head| head.status == Some(0));
        if let Some(head) = head {
            if head.text()? == format!("refs/remotes/{remote}/{trunk}") {
                return Ok(Some(remote.clone()));
            }
        }
    }
    let configured = git::try_run(
        primary,
        &["config", "--get", &format!("branch.{trunk}.remote")],
    )?;
    if configured.status == Some(0) {
        let remote = configured.text()?;
        if remotes.contains(&remote) {
            return Ok(Some(remote));
        }
    }
    Ok(None)
}

fn default_local() -> LocalOverrides {
    LocalOverrides::NonePrimary
}

fn default_hooks() -> HooksPath {
    HooksPath::NotConfigured
}

/// What a remove did.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RemoveReport {
    /// What was removed.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch it held. It still exists: removing a worktree never deletes a branch.
    pub branch: Option<String>,
    /// Whether `--force` was needed.
    pub forced: bool,
}

/// The repository's worktrees, decided once.
///
/// Built from a directory — any directory inside any work tree — and nothing else.
#[derive(Debug, Clone)]
pub struct WorktreeService {
    identity: RepositoryIdentity,
    container: ResolvedPath,
    issue_ids: Vec<String>,
    scratch_roots: Vec<ScratchRoot>,
}

/// A directory a checkout of somebody else's lives under: the tool's own temporary roots
/// or a provider's, as `share/providers.yaml` declares them (ADR 0024).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScratchRoot {
    /// The provider that creates checkouts there; `None` for the tool's own roots.
    pub provider: Option<String>,
    /// The root, expanded.
    pub path: PathBuf,
}

impl WorktreeService {
    /// Resolve the repository from `start`. The scratch roots come from the distribution's
    /// provider declarations, located the way every command locates the share; a checkout
    /// with no share in reach declares none, and a temporary checkout is then what its path
    /// says it is.
    pub fn open(start: &Path) -> Result<Self> {
        let identity = RepositoryIdentity::discover(start)?;
        Self::over(identity)
    }

    /// The service over an identity already resolved.
    pub fn over(identity: RepositoryIdentity) -> Result<Self> {
        let primary = identity.primary_worktree().path.clone();
        let current = identity.current_worktree().path.clone();
        let scratch_roots = declared_scratch_roots(&current, &primary);
        Self::over_with(identity, scratch_roots)
    }

    /// The service over an identity, with the scratch roots given rather than located.
    pub fn over_with(
        identity: RepositoryIdentity,
        scratch_roots: Vec<ScratchRoot>,
    ) -> Result<Self> {
        let container = ResolvedPath::of(path::container_root(&identity.primary_worktree().path)?);
        let issue_ids = state::issue_ids(&identity.primary_worktree().path);
        Ok(WorktreeService {
            identity,
            container,
            issue_ids,
            scratch_roots,
        })
    }

    /// The scratch roots in force, expanded.
    pub fn scratch_roots(&self) -> &[ScratchRoot] {
        &self.scratch_roots
    }

    /// The repository this service speaks for.
    pub fn identity(&self) -> &RepositoryIdentity {
        &self.identity
    }

    /// The canonical container.
    pub fn container(&self) -> &ResolvedPath {
        &self.container
    }

    /// Where the worktree of `branch` belongs.
    pub fn expected_path_of(&self, branch: &str) -> Result<PathBuf> {
        path::expected_path(&self.container.path, &BranchName::parse(branch)?)
    }

    // ------------------------------------------------------------ views

    fn repository_view(&self) -> RepositoryView {
        RepositoryView {
            primary_worktree: display(&self.identity.primary_worktree().path),
            git_common_dir: display(&self.identity.git_common_dir().path),
            name: self.identity.repository_name(),
        }
    }

    fn container_view(&self) -> ContainerView {
        ContainerView {
            path: display(&self.container.path),
            suffix: CONTAINER_SUFFIX.to_string(),
            exists: self.container.path.is_dir(),
        }
    }

    fn trunk_view(&self) -> TrunkView {
        let trunk = self.identity.trunk();
        TrunkView {
            branch: trunk.branch.clone(),
            source: trunk.source,
            checked_out_at: trunk
                .branch
                .as_deref()
                .and_then(|b| self.identity.record_of_branch(b))
                .map(|r| display(&r.path)),
        }
    }

    // ------------------------------------------------------------ judging one work tree

    /// Everything the topology says about one registered work tree. The one place a
    /// standing is decided.
    pub fn judge(
        &self,
        record: &WorktreeRecord,
        detail: Detail,
        branches: &BTreeMap<String, BranchRef>,
    ) -> WorktreeState {
        let dirty = Self::measures_dirty(record, detail)
            .then(|| state::dirty_state(&record.path).ok())
            .flatten();
        self.judge_with(record, branches, dirty)
    }

    /// Does the topology take a `git status` of this work tree at this detail? The one
    /// place the condition is written, because the whole topology takes these measurements
    /// at once ([`state::dirty_states`]) and the batch must select exactly the work trees
    /// [`Self::judge`] would have measured one at a time.
    fn measures_dirty(record: &WorktreeRecord, detail: Detail) -> bool {
        detail == Detail::Full && record.path.is_dir() && !record.bare
    }

    /// [`Self::judge`] with the uncommitted work already measured — or deliberately not
    /// measured, which is what `None` means here, exactly as it does in the answer.
    fn judge_with(
        &self,
        record: &WorktreeRecord,
        branches: &BTreeMap<String, BranchRef>,
        dirty: Option<DirtyState>,
    ) -> WorktreeState {
        let resolved = ResolvedPath::of(&record.path);
        let is_primary = self.identity.is_primary(record);
        let exists = record.path.is_dir();
        let trunk = self.identity.trunk();
        let mut diagnostics = Vec::new();
        let path_text = display(&record.path);

        let label = match (&record.branch, &record.head) {
            (Some(b), _) => b.clone(),
            (None, Some(h)) => path::detached_label(h),
            (None, None) => "(unborn)".to_string(),
        };

        // where the branch belongs, if it has one and the name derives
        let expected: Option<PathBuf> = match record.branch.as_deref() {
            Some(b) => match BranchName::parse(b)
                .and_then(|n| path::expected_path(&self.container.path, &n))
            {
                Ok(p) => Some(p),
                Err(e) => {
                    diagnostics.push(TopologyDiagnostic {
                        code: match e {
                            WorktreeError::PathEscape { .. } => DiagnosticCode::PathEscape,
                            _ => DiagnosticCode::InvalidBranchName,
                        },
                        severity: Severity::Error,
                        path: Some(path_text.clone()),
                        branch: Some(b.to_string()),
                        expected: None,
                        message: e.to_string(),
                        remedy: "rename the branch (git branch -m) to a name git and the filesystem both accept".into(),
                    });
                    None
                }
            },
            None => None,
        };
        let expected_text = expected.as_deref().map(display);

        let standing = if is_primary {
            if let Some(b) = record.branch.as_deref() {
                if let Some(t) = trunk.branch.as_deref() {
                    if b != t {
                        diagnostics.push(TopologyDiagnostic {
                            code: DiagnosticCode::PrimaryOnNonTrunk,
                            severity: Severity::Error,
                            path: Some(path_text.clone()),
                            branch: Some(b.to_string()),
                            expected: expected_text.clone(),
                            message: format!(
                                "the primary checkout holds '{b}', and the primary checkout hosts the trunk '{t}'"
                            ),
                            remedy: format!(
                                "when the primary checkout is clean: git switch {t}; then majordomus worktree create {b}"
                            ),
                        });
                    }
                }
            }
            Standing::Primary
        } else if record.prunable.is_some() || !exists {
            diagnostics.push(TopologyDiagnostic {
                code: if record.prunable.is_some() {
                    DiagnosticCode::StaleRegistration
                } else {
                    DiagnosticCode::Missing
                },
                severity: Severity::Warning,
                path: Some(path_text.clone()),
                branch: record.branch.clone(),
                expected: expected_text.clone(),
                message: match &record.prunable {
                    Some(reason) if !reason.is_empty() => {
                        format!("git reports it prunable: {reason}")
                    }
                    _ => "the directory git registered does not exist".into(),
                },
                remedy: "majordomus worktree repair".into(),
            });
            Standing::Missing
        } else if record.branch.is_none() {
            diagnostics.push(TopologyDiagnostic {
                code: DiagnosticCode::Detached,
                severity: Severity::Info,
                path: Some(path_text.clone()),
                branch: None,
                expected: None,
                message: "HEAD is detached: no branch, so no canonical path; nothing moves it"
                    .into(),
                remedy: "git switch -c <branch> there, then majordomus worktree migrate".into(),
            });
            Standing::Detached
        } else if let Some(root) = self.scratch_root_of(&resolved) {
            diagnostics.push(TopologyDiagnostic {
                code: DiagnosticCode::Ephemeral,
                severity: Severity::Warning,
                path: Some(path_text.clone()),
                branch: record.branch.clone(),
                expected: expected_text.clone(),
                message: format!(
                    "a session's scratch checkout under {}{}, holding branch '{}' which belongs at {}; nothing moves it while the session that made it may be running",
                    display(&root.path),
                    root.provider
                        .as_deref()
                        .map(|p| format!(" (created by {p})"))
                        .unwrap_or_default(),
                    label,
                    expected_text.as_deref().unwrap_or("-")
                ),
                remedy: format!(
                    "detach it (git switch --detach) and continue in the canonical worktree (majordomus worktree create {}); or majordomus worktree migrate --include-ephemeral --only {}",
                    label, label
                ),
            });
            Standing::Ephemeral
        } else if resolved.same_as(&self.container) {
            diagnostics.push(TopologyDiagnostic {
                code: DiagnosticCode::ContainerOccupied,
                severity: Severity::Error,
                path: Some(path_text.clone()),
                branch: record.branch.clone(),
                expected: expected_text.clone(),
                message: format!(
                    "it occupies the container {} itself; the container holds worktrees and cannot be one",
                    display(&self.container.path)
                ),
                remedy: "majordomus worktree migrate".into(),
            });
            Standing::Misplaced
        } else if expected.as_deref().is_some_and(|e| {
            ResolvedPath::of(e).same_as(&resolved) && !symlink_below(&self.container.path, e)
        }) {
            Standing::Canonical
        } else if expected.is_some() {
            diagnostics.push(TopologyDiagnostic {
                code: DiagnosticCode::PathMismatch,
                severity: Severity::Error,
                path: Some(path_text.clone()),
                branch: record.branch.clone(),
                expected: expected_text.clone(),
                message: format!(
                    "branch '{}' belongs at {}",
                    label,
                    expected_text.as_deref().unwrap_or("-")
                ),
                remedy: "majordomus worktree migrate".into(),
            });
            let inside_primary = resolved.is_inside(self.identity.primary_worktree());
            let inside_other = self
                .identity
                .registered_worktrees()
                .iter()
                .filter(|r| r.path != record.path)
                .any(|r| resolved.is_inside(&ResolvedPath::of(&r.path)));
            if inside_primary || inside_other {
                diagnostics.push(TopologyDiagnostic {
                    code: DiagnosticCode::Nested,
                    severity: Severity::Warning,
                    path: Some(path_text.clone()),
                    branch: record.branch.clone(),
                    expected: expected_text.clone(),
                    message: if inside_primary {
                        "it sits inside the primary checkout".into()
                    } else {
                        "it sits inside another worktree of this repository".into()
                    },
                    remedy: "majordomus worktree migrate".into(),
                });
            }
            Standing::Misplaced
        } else {
            // a branch whose name did not derive: already diagnosed above
            Standing::Misplaced
        };

        if standing == Standing::Misplaced {
            if let Some(reason) = &record.locked {
                diagnostics.push(TopologyDiagnostic {
                    code: DiagnosticCode::Locked,
                    severity: Severity::Warning,
                    path: Some(path_text.clone()),
                    branch: record.branch.clone(),
                    expected: expected_text.clone(),
                    message: if reason.is_empty() {
                        "it is locked, so git will not move it".into()
                    } else {
                        format!("it is locked ({reason}), so git will not move it")
                    },
                    remedy: format!("git worktree unlock {path_text}"),
                });
            }
            if let Some(e) = expected.as_deref() {
                if let Some(d) = self.destination_conflict(e, Some(record)) {
                    diagnostics.push(d);
                }
            }
        }

        if !is_primary && record.branch.as_deref().is_some_and(|b| trunk.is(b)) {
            diagnostics.push(TopologyDiagnostic {
                code: DiagnosticCode::TrunkInLinkedWorktree,
                severity: Severity::Warning,
                path: Some(path_text.clone()),
                branch: record.branch.clone(),
                expected: Some(display(&self.identity.primary_worktree().path)),
                message: "the trunk is checked out in a linked worktree; the primary checkout hosts it".into(),
                remedy: "switch the primary checkout to the trunk and remove this worktree when it is clean".into(),
            });
        }

        WorktreeState {
            path: path_text,
            kind: if is_primary {
                WorktreeKind::Primary
            } else {
                WorktreeKind::Linked
            },
            standing,
            branch: record.branch.clone(),
            label,
            head: record.head.clone(),
            detached: record.detached,
            expected_path: if is_primary && record.branch.as_deref().is_some_and(|b| trunk.is(b)) {
                None
            } else {
                expected_text
            },
            exists,
            current: self.identity.is_current(record),
            locked: record.locked.clone(),
            prunable: record.prunable.clone(),
            dirty,
            upstream: record
                .branch
                .as_deref()
                .and_then(|b| branches.get(b))
                .and_then(|b| b.upstream.clone()),
            issue: record
                .branch
                .as_deref()
                .and_then(|b| state::issue_of(b, &self.issue_ids)),
            diagnostics,
        }
    }

    /// The scratch root a path lies under, when it does. The roots are the declarations'
    /// (`share/providers.yaml`): the tool's own temporary directories and every provider's,
    /// such as `.claude/worktrees/` under the primary checkout, which Claude Code creates and
    /// removes with the session that asked for it, or the thread directory an orchestrator
    /// keeps under its data directory. A root the primary checkout itself lives under is
    /// skipped, as a test fixture's temporary directory is: nothing beside the repository
    /// is a scratch checkout of it.
    pub fn scratch_root_of(&self, path: &ResolvedPath) -> Option<ScratchRoot> {
        let primary = self.identity.primary_worktree();
        for root in &self.scratch_roots {
            let resolved = ResolvedPath::of(root.path.clone());
            if primary.is_inside(&resolved) || primary.same_as(&resolved) {
                continue;
            }
            if path.is_inside(&resolved) {
                return Some(ScratchRoot {
                    provider: root.provider.clone(),
                    path: resolved.path,
                });
            }
        }
        None
    }

    /// [`Self::scratch_root_of`], the path alone.
    pub fn ephemeral_root_of(&self, path: &ResolvedPath) -> Option<PathBuf> {
        self.scratch_root_of(path).map(|r| r.path)
    }

    /// What occupies `expected`, when something other than `own` does.
    pub(crate) fn destination_conflict(
        &self,
        expected: &Path,
        own: Option<&WorktreeRecord>,
    ) -> Option<TopologyDiagnostic> {
        let meta = std::fs::symlink_metadata(expected).ok()?;
        let resolved = ResolvedPath::of(expected);
        // A link is a conflict whatever it points at — including this very worktree: the
        // canonical path has to be the directory, not a name for it somewhere else.
        if !meta.file_type().is_symlink() {
            if let Some(r) = self.identity.record_at(&resolved) {
                if own.is_some_and(|o| o.path == r.path) {
                    return None;
                }
                return Some(TopologyDiagnostic {
                code: DiagnosticCode::DestinationConflict,
                severity: Severity::Error,
                path: own.map(|o| display(&o.path)),
                branch: own.and_then(|o| o.branch.clone()),
                expected: Some(display(expected)),
                message: format!(
                    "the canonical path is occupied by a registered worktree of this repository holding {}",
                    r.branch.clone().unwrap_or_else(|| "a detached HEAD".into())
                ),
                remedy: "move that worktree to its own canonical path first (majordomus worktree migrate)".into(),
            });
            }
        }
        let what = if meta.file_type().is_symlink() {
            "a symbolic link".to_string()
        } else if meta.is_dir() {
            if expected.join(".git").exists() {
                "a git checkout this repository has not registered".to_string()
            } else if std::fs::read_dir(expected)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false)
            {
                "an empty directory".to_string()
            } else {
                "an unrelated directory with content".to_string()
            }
        } else {
            "a file".to_string()
        };
        Some(TopologyDiagnostic {
            code: DiagnosticCode::DestinationConflict,
            severity: Severity::Error,
            path: own.map(|o| display(&o.path)),
            branch: own.and_then(|o| o.branch.clone()),
            expected: Some(display(expected)),
            message: format!("the canonical path is occupied by {what}; nothing overwrites it"),
            remedy: format!(
                "inspect and move it aside by hand: ls -la {}",
                display(expected)
            ),
        })
    }

    // ------------------------------------------------------------ the topology

    /// The whole topology.
    pub fn topology(&self, detail: Detail) -> Result<RepositoryTopology> {
        let primary = &self.identity.primary_worktree().path;
        let trunk = self.identity.trunk();
        let branch_refs = state::branches(primary)?;
        let branches: BTreeMap<String, BranchRef> = branch_refs
            .iter()
            .map(|b| (b.name.clone(), b.clone()))
            .collect();
        let merged: Option<BTreeSet<String>> = match trunk.branch.as_deref() {
            Some(t) => Some(state::merged_into_trunk(primary, t)?),
            None => None,
        };

        // Every `git status` this topology needs, taken at once rather than one work tree
        // after the next: the measurements do not depend on one another, and on a
        // repository with a hundred-odd registered work trees the sequence was most of
        // what a topology cost. Judging itself runs no subprocess.
        let records = self.identity.registered_worktrees();
        let probes: Vec<Option<&Path>> = records
            .iter()
            .map(|r| Self::measures_dirty(r, detail).then_some(r.path.as_path()))
            .collect();
        let mut worktrees: Vec<WorktreeState> = records
            .iter()
            .zip(state::dirty_states(&probes))
            .map(|(r, d)| self.judge_with(r, &branches, d))
            .collect();

        // repository-wide facts
        let mut repository_diagnostics = Vec::new();
        if trunk.branch.is_none() {
            repository_diagnostics.push(TopologyDiagnostic {
                code: DiagnosticCode::TrunkUnknown,
                severity: Severity::Warning,
                path: None,
                branch: None,
                expected: None,
                message: "the trunk could not be determined: no remote HEAD, no init.defaultBranch, neither main nor master alone, and the primary checkout is detached".into(),
                remedy: "git remote set-head origin --auto, or git config init.defaultBranch <branch>".into(),
            });
        }
        let mut by_folded: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for w in &worktrees {
            if let (Some(e), Some(b)) = (&w.expected_path, &w.branch) {
                by_folded
                    .entry(e.to_lowercase())
                    .or_default()
                    .push(b.clone());
            }
        }
        for (_, names) in by_folded.iter().filter(|(_, n)| n.len() > 1) {
            repository_diagnostics.push(TopologyDiagnostic {
                code: DiagnosticCode::CaseCollision,
                severity: Severity::Warning,
                path: None,
                branch: Some(names.join(", ")),
                expected: None,
                message: "these branch names derive paths that differ only by case, which one directory on a case-insensitive filesystem".into(),
                remedy: "rename one of the branches (git branch -m)".into(),
            });
        }

        let by_path: BTreeMap<String, &WorktreeState> =
            worktrees.iter().map(|w| (w.path.clone(), w)).collect();
        let mut branch_states: Vec<BranchState> = branch_refs
            .iter()
            .map(|b| {
                let is_trunk = trunk.is(&b.name);
                let worktree_path = b.worktree.as_deref().map(display);
                let holder = worktree_path.as_deref().and_then(|p| by_path.get(p));
                let merged_into_trunk = merged.as_ref().map(|m| m.contains(&b.name));
                // A session's scratch checkout is never cleanup-eligible however merged and
                // clean: the harness that made it removes it, and a listing that offered it
                // would offer another session's floor.
                let cleanup_eligible = !is_trunk
                    && merged_into_trunk == Some(true)
                    && match holder {
                        None => true,
                        Some(h) => {
                            h.standing != Standing::Ephemeral
                                && h.dirty.as_ref().is_some_and(|d| d.clean)
                        }
                    };
                BranchState {
                    name: b.name.clone(),
                    head: b.head.clone(),
                    trunk: is_trunk,
                    expected_path: if is_trunk {
                        None
                    } else {
                        self.expected_path_of(&b.name).ok().map(|p| display(&p))
                    },
                    worktree: worktree_path,
                    upstream: b.upstream.clone(),
                    merged_into_trunk,
                    cleanup_eligible,
                    issue: state::issue_of(&b.name, &self.issue_ids),
                }
            })
            .collect();
        crate::order::canonical(&mut branch_states);

        let mut tallies = TopologyTallies {
            worktrees: worktrees.len(),
            branches: branch_states.len(),
            branches_without_worktree: branch_states
                .iter()
                .filter(|b| b.worktree.is_none())
                .count(),
            cleanup_eligible: branch_states.iter().filter(|b| b.cleanup_eligible).count(),
            ..TopologyTallies::default()
        };
        for w in &worktrees {
            match w.standing {
                Standing::Canonical => tallies.canonical += 1,
                Standing::Misplaced => tallies.misplaced += 1,
                Standing::Detached => tallies.detached += 1,
                Standing::Ephemeral => tallies.ephemeral += 1,
                Standing::Missing => tallies.missing += 1,
                Standing::Primary => {}
            }
            if w.locked.is_some() {
                tallies.locked += 1;
            }
            if w.dirty.as_ref().is_some_and(|d| !d.clean) {
                tallies.dirty += 1;
            }
        }
        let mut diagnostics: Vec<TopologyDiagnostic> = worktrees
            .iter_mut()
            .flat_map(|w| w.diagnostics.clone())
            .collect();
        diagnostics.extend(repository_diagnostics);
        tallies.errors = diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        tallies.warnings = diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count();

        Ok(RepositoryTopology {
            schema: SCHEMA.to_string(),
            repository: self.repository_view(),
            container: self.container_view(),
            trunk: self.trunk_view(),
            observed_from: display(&self.identity.current_worktree().path),
            valid: tallies.errors == 0,
            worktrees,
            branches: branch_states,
            diagnostics,
            tallies,
        })
    }

    /// Where this call is, and whether that is where it belongs.
    pub fn status(&self) -> Result<StatusReport> {
        let topology = self.topology(Detail::Fast)?;
        let record = self.identity.current_record().cloned().ok_or_else(|| {
            WorktreeError::NotThisRepository {
                path: self.identity.current_worktree().path.clone(),
                git_common_dir: self.identity.git_common_dir().path.clone(),
            }
        })?;
        let branches: BTreeMap<String, BranchRef> =
            state::branches(&self.identity.primary_worktree().path)?
                .into_iter()
                .map(|b| (b.name.clone(), b))
                .collect();
        let worktree = self.judge(&record, Detail::Full, &branches);
        let canonical = is_in_place(&worktree, self.identity.trunk().source);
        Ok(StatusReport {
            schema: SCHEMA.to_string(),
            repository: topology.repository,
            container: topology.container,
            trunk: topology.trunk,
            worktree,
            canonical,
            repository_errors: topology.tallies.errors,
            trunk_distance: self.trunk_distance(),
        })
    }

    /// How far the work tree this call came from, and the local trunk, are from the remote
    /// trunk, judged against the thresholds its policy declares. Reads git, fetches nothing.
    ///
    /// ```
    /// use std::process::Command;
    /// use majordomus_cli::worktree::WorktreeService;
    /// use majordomus_cli::worktree::trunk::TrunkStanding;
    ///
    /// let root = std::env::temp_dir().join(format!("mj-trunk-distance-{}", std::process::id()));
    /// std::fs::create_dir_all(&root).unwrap();
    /// let git = |args: &[&str]| assert!(Command::new("git").current_dir(&root).args(args)
    ///     .env("GIT_AUTHOR_NAME", "a").env("GIT_AUTHOR_EMAIL", "a@b.c")
    ///     .env("GIT_COMMITTER_NAME", "a").env("GIT_COMMITTER_EMAIL", "a@b.c")
    ///     .status().unwrap().success());
    /// git(&["init", "-q", "-b", "master"]);
    /// git(&["commit", "-q", "--allow-empty", "-m", "base"]);
    ///
    /// // a repository with no remote has no remote trunk to be counted against
    /// let d = WorktreeService::open(&root).unwrap().trunk_distance();
    /// assert_eq!(d.standing, TrunkStanding::Unmeasured);
    /// assert!(d.checkout.is_none() && d.remedy.is_none());
    /// std::fs::remove_dir_all(&root).unwrap();
    /// ```
    pub fn trunk_distance(&self) -> super::trunk::TrunkDistance {
        let here = &self.identity.current_worktree().path;
        super::trunk::measure(
            &self.identity,
            here,
            super::trunk::TrunkThresholds::declared_at(here),
        )
    }

    /// The guard: may a mutation proceed from the work tree this call came from?
    pub fn guard(&self) -> Result<GuardVerdict> {
        let status = self.status()?;
        let w = status.worktree;
        let exempt = matches!(w.standing, Standing::Detached)
            || (w.standing == Standing::Primary && w.expected_path.is_none());
        let reason = w
            .diagnostics
            .iter()
            .find(|d| d.severity == Severity::Error || d.code == DiagnosticCode::Ephemeral)
            .cloned();
        Ok(GuardVerdict {
            ok: status.canonical,
            path: w.path,
            branch: w.branch,
            expected_path: w.expected_path,
            standing: w.standing,
            exempt,
            reason: if status.canonical { None } else { reason },
        })
    }

    /// One branch: where its worktree belongs and what is there.
    pub fn inspect(&self, branch: &str) -> Result<InspectReport> {
        let name = BranchName::parse(branch)?;
        let expected = path::expected_path(&self.container.path, &name)?;
        let primary = &self.identity.primary_worktree().path;
        let branches: BTreeMap<String, BranchRef> = state::branches(primary)?
            .into_iter()
            .map(|b| (b.name.clone(), b))
            .collect();
        let worktree = self
            .identity
            .record_of_branch(branch)
            .map(|r| self.judge(r, Detail::Full, &branches));
        let canonical = worktree
            .as_ref()
            .is_some_and(|w| w.standing == Standing::Canonical);
        let mut diagnostics: Vec<TopologyDiagnostic> = worktree
            .as_ref()
            .map(|w| w.diagnostics.clone())
            .unwrap_or_default();
        if worktree.is_none() {
            if let Some(d) = self.destination_conflict(&expected, None) {
                diagnostics.push(d);
            }
        }
        Ok(InspectReport {
            branch: branch.to_string(),
            branch_exists: branches.contains_key(branch),
            expected_path: display(&expected),
            destination_exists: std::fs::symlink_metadata(&expected).is_ok(),
            worktree,
            canonical,
            diagnostics,
        })
    }

    // ------------------------------------------------------------ selectors

    /// Resolve a selector to exactly one registered work tree: an exact branch name, or an
    /// exact path. Nothing is matched by prefix, substring or similarity — these selectors
    /// are given to commands that delete things.
    pub fn resolve(&self, selector: &str) -> Result<WorktreeRecord> {
        if let Some(r) = self.identity.record_of_branch(selector) {
            return Ok(r.clone());
        }
        let wanted = ResolvedPath::of(PathBuf::from(selector));
        if let Some(r) = self.identity.record_at(&wanted) {
            return Ok(r.clone());
        }
        Err(WorktreeError::NoSuchWorktree {
            selector: selector.to_string(),
        })
    }

    // ------------------------------------------------------------ create

    /// Create the canonical worktree of a branch. The destination is derived and never
    /// given. Everything that could go wrong is checked before git is asked to do anything,
    /// so a refusal leaves the repository exactly as it was.
    pub fn create(&self, request: &CreateRequest) -> Result<CreateReport> {
        self.create_inner(request, false)
    }

    /// The same, answering the existing canonical worktree rather than refusing when it is
    /// already there.
    pub fn ensure(&self, request: &CreateRequest) -> Result<CreateReport> {
        self.create_inner(request, true)
    }

    fn create_inner(&self, request: &CreateRequest, ensure: bool) -> Result<CreateReport> {
        let name = BranchName::parse(&request.branch)?;
        // a branch with no place in the container is refused before the network is asked
        let target = path::expected_path(&self.container.path, &name)?;
        let primary = self.identity.primary_worktree().path.clone();

        // The network is asked before the lock is taken, so a slow remote never holds every
        // other create in this repository behind it; the decision is re-checked under it.
        // A branch git cannot be asked about here is asked about again under the lock.
        let unknown_here = matches!(git::branch_exists(&primary, name.as_str()), Ok(false));
        let planned = if request.base.is_none() && unknown_here {
            Some(self.plan_start(&primary, name.as_str())?)
        } else {
            None
        };
        self.create_locked(request, ensure, name, target, planned)
    }

    /// The create under the lock, given the branch and its place as `create_inner` checked
    /// them and what was planned before it: `planned` is `None` when
    /// the branch existed (or could not be asked about) before the lock, and the start is
    /// then planned here if the branch is gone by the time the lock is held.
    fn create_locked(
        &self,
        request: &CreateRequest,
        ensure: bool,
        name: BranchName,
        target: PathBuf,
        planned: Option<StartPoint>,
    ) -> Result<CreateReport> {
        let resolved_target = ResolvedPath::of(&target);
        let primary = self.identity.primary_worktree().path.clone();

        // One lock around the whole check-then-act, and the topology re-read under it.
        let _lock = WorktreeLock::acquire(&self.identity.git_common_dir().path)?;
        let mut identity = self.identity.clone();
        identity.refresh()?;

        if let Some(existing) = identity.record_of_branch(name.as_str()) {
            if ResolvedPath::of(&existing.path).same_as(&resolved_target) {
                if ensure {
                    // Found rather than made, and approved all the same: a worktree that
                    // exists and is blocked is what `ensure` is most often asked about.
                    let envrc = direnv::approve(&primary, &existing.path);
                    let envrc_local = direnv::link_local(&primary, &existing.path);
                    let hooks = hooks::carry_hooks_path(&primary, &existing.path);
                    return Ok(CreateReport {
                        path: display(&existing.path),
                        branch: name.as_str().to_string(),
                        branch_created: false,
                        base: None,
                        start: None,
                        start_commit: None,
                        container: display(&self.container.path),
                        container_created: false,
                        existed: true,
                        envrc,
                        envrc_local,
                        hooks,
                    });
                }
                return Err(WorktreeError::WorktreeAlreadyExists {
                    path: existing.path.clone(),
                    branch: name.as_str().to_string(),
                });
            }
            return Err(WorktreeError::BranchAlreadyCheckedOut {
                branch: name.as_str().to_string(),
                path: existing.path.clone(),
                expected: target,
            });
        }
        if let Some(d) = self.destination_conflict(&target, None) {
            return Err(WorktreeError::DestinationConflict {
                path: target,
                what: d.message,
            });
        }
        if !git::branch_name_is_valid(&primary, name.as_str())? {
            return Err(WorktreeError::InvalidBranchName {
                given: name.as_str().to_string(),
                reason: "git does not accept it as a reference name".into(),
            });
        }
        let branch_exists = git::branch_exists(&primary, name.as_str())?;
        let start = if branch_exists {
            None
        } else {
            let start = match (&request.base, planned) {
                (Some(given), _) => StartPoint::Given {
                    reference: given.clone(),
                },
                (None, Some(planned)) => planned,
                (None, None) => self.plan_start(&primary, name.as_str())?,
            };
            let base = start.reference();
            if !git::rev_exists(&primary, &base)? {
                return Err(WorktreeError::BaseDoesNotExist { base });
            }
            Some(start)
        };
        let base = start.as_ref().map(StartPoint::reference);

        let container_created = self.ensure_container()?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| WorktreeError::io(parent, &e))?;
        }
        let target_text = display(&target);
        let mut args: Vec<String> = vec!["worktree".into(), "add".into()];
        match (&start, &base) {
            (None, _) | (_, None) => {
                args.push("--".into());
                args.push(target_text.clone());
                args.push(name.as_str().to_string());
            }
            (Some(start), Some(base)) => {
                match start {
                    // The same branch on both sides: it is the remote's, and tracks it.
                    StartPoint::RemoteBranch { .. } => args.push("--track".into()),
                    // New work cut from the remote's trunk does not track the trunk; its
                    // upstream is its own branch, set when it is first pushed.
                    StartPoint::RemoteTrunk { .. } => args.push("--no-track".into()),
                    _ => {}
                }
                args.push("-b".into());
                args.push(name.as_str().to_string());
                args.push("--".into());
                args.push(target_text.clone());
                args.push(base.clone());
            }
        }
        git::run(&primary, &args)?;

        // Verify rather than assume: the answer is what the topology says.
        identity.refresh()?;
        let registered = identity
            .record_at(&resolved_target)
            .cloned()
            .ok_or_else(|| WorktreeError::MigrationUnsafe {
                path: target.clone(),
                reason: "git reported success and `git worktree list` does not show the worktree; \
                         nothing was cleaned up because that would risk deleting real work"
                    .into(),
            })?;
        // The path exists now; this is the moment direnv's approval is carried to it.
        let envrc = direnv::approve(&primary, &registered.path);
        let envrc_local = direnv::link_local(&primary, &registered.path);
        // And the moment it is bound to the hooks of its own tree, before its first commit.
        let hooks = hooks::carry_hooks_path(&primary, &registered.path);
        let start_commit = match start {
            // The worktree is made; a commit git cannot read back is not a reason to say not.
            Some(_) => git::head_of(&registered.path).ok().flatten(),
            None => None,
        };
        Ok(CreateReport {
            path: display(&registered.path),
            branch: name.as_str().to_string(),
            branch_created: base.is_some(),
            base,
            start,
            start_commit,
            container: display(&self.container.path),
            container_created,
            existed: false,
            envrc,
            envrc_local,
            hooks,
        })
    }

    /// Decide where a branch that does not exist here starts, asking the trunk's remote.
    ///
    /// The trunk and the branch are fetched from the trunk's remote. A branch the remote
    /// has is taken up where it stands; any other starts at the remote's trunk. A remote
    /// that cannot be read leaves the remote-tracking refs as last fetched: a branch known
    /// there is still taken up, and a new one starts from the local trunk, with how far
    /// behind it is. A trunk with no remote is the local trunk, and no trunk at all is HEAD.
    fn plan_start(&self, primary: &Path, branch: &str) -> Result<StartPoint> {
        let Some(trunk) = self.identity.trunk().branch.clone() else {
            return Ok(StartPoint::Head);
        };
        let Some(remote) = trunk_remote(primary, &trunk)? else {
            return Ok(StartPoint::LocalTrunk {
                branch: trunk,
                unreachable: None,
                reason: None,
                behind: None,
            });
        };
        let tracking = |b: &str| format!("refs/remotes/{remote}/{b}");
        let refspec = |b: &str| format!("+refs/heads/{b}:{}", tracking(b));
        let trunk_fetch =
            git::fetch_bounded(primary, &remote, &[refspec(&trunk)], git::FETCH_TIMEOUT);
        if trunk_fetch.is_ok() {
            match git::fetch_bounded(primary, &remote, &[refspec(branch)], git::FETCH_TIMEOUT) {
                Ok(()) => {
                    return Ok(StartPoint::RemoteBranch {
                        remote,
                        branch: branch.to_string(),
                        fetched: true,
                    })
                }
                // A remote-tracking ref left behind by a branch since deleted there is
                // not taken up: the remote has just said it is gone.
                Err(git::FetchFailure::NoSuchRemoteRef) => {
                    return Ok(StartPoint::RemoteTrunk {
                        remote,
                        branch: trunk,
                    })
                }
                Err(git::FetchFailure::Unreachable(_)) => {}
            }
        }
        // Either fetch failed: a branch known here as last fetched is still taken up.
        if git::rev_exists(primary, &tracking(branch))? {
            return Ok(StartPoint::RemoteBranch {
                remote,
                branch: branch.to_string(),
                fetched: false,
            });
        }
        let reason = match trunk_fetch {
            // The trunk arrived and only the branch could not be asked about.
            Ok(()) => {
                return Ok(StartPoint::RemoteTrunk {
                    remote,
                    branch: trunk,
                })
            }
            Err(git::FetchFailure::NoSuchRemoteRef) => format!("{remote} has no branch {trunk}"),
            Err(git::FetchFailure::Unreachable(why)) => why,
        };
        let behind = git::commits_between(primary, &trunk, &tracking(&trunk));
        Ok(StartPoint::LocalTrunk {
            branch: trunk,
            unreachable: Some(remote),
            reason: Some(reason),
            behind,
        })
    }

    /// Make sure the container exists, and answer whether this call created it. It lives
    /// beside the repository and is never committed, so it is created lazily, by the first
    /// worktree that needs it.
    pub(crate) fn ensure_container(&self) -> Result<bool> {
        let p = &self.container.path;
        if p.is_dir() {
            return Ok(false);
        }
        if std::fs::symlink_metadata(p).is_ok() {
            return Err(WorktreeError::ContainerNotADirectory { path: p.clone() });
        }
        std::fs::create_dir_all(p).map_err(|e| WorktreeError::io(p, &e))?;
        Ok(true)
    }

    // ------------------------------------------------------------ remove

    /// Remove one linked worktree. Never a branch, never the primary checkout, never
    /// uncommitted work without `force`.
    pub fn remove(&self, selector: &str, force: bool) -> Result<RemoveReport> {
        let _lock = WorktreeLock::acquire(&self.identity.git_common_dir().path)?;
        let mut identity = self.identity.clone();
        identity.refresh()?;
        let record = self.resolve(selector)?;
        if identity.is_primary(&record) {
            return Err(WorktreeError::PrimaryWorktreeProtected {
                path: record.path.clone(),
                operation: "remove".into(),
            });
        }
        let locked = record.locked.is_some();
        if locked && !force {
            return Err(WorktreeError::LockedWorktree {
                path: record.path.clone(),
                reason: record.locked.clone(),
            });
        }
        let mut forced = false;
        if record.path.is_dir() {
            let dirty = state::dirty_state(&record.path)?;
            if !dirty.clean {
                if !force {
                    return Err(WorktreeError::DirtyWorktree {
                        path: record.path.clone(),
                        summary: dirty.summary(),
                        operation: "remove".into(),
                    });
                }
                forced = true;
            }
        }
        let mut args: Vec<String> = vec!["worktree".into(), "remove".into()];
        if forced || locked {
            args.push("--force".into());
        }
        if locked {
            args.push("--force".into());
        }
        args.push("--".into());
        args.push(display(&record.path));
        git::run(&identity.primary_worktree().path, &args)?;
        Ok(RemoveReport {
            path: display(&record.path),
            branch: record.branch.clone(),
            forced: forced || locked,
        })
    }

    // ------------------------------------------------------------ repair

    /// Drop the registrations of work trees whose directories are gone, and let git repair
    /// the administrative links of the ones that exist. Deletes no directory.
    pub fn repair(&self, dry_run: bool) -> Result<RepairReport> {
        let _lock = WorktreeLock::acquire(&self.identity.git_common_dir().path)?;
        let primary = &self.identity.primary_worktree().path;
        let mut args: Vec<String> = vec!["worktree".into(), "prune".into(), "--verbose".into()];
        if dry_run {
            args.push("--dry-run".into());
        }
        let pruned = lines_of(&git::run(primary, &args)?.stdout);
        let repaired = if dry_run {
            Vec::new()
        } else {
            let mut args: Vec<String> = vec!["worktree".into(), "repair".into()];
            for r in self.identity.registered_worktrees().iter().skip(1) {
                if r.path.is_dir() {
                    args.push(display(&r.path));
                }
            }
            let out = git::run(primary, &args)?;
            let mut lines = lines_of(&out.stdout);
            lines.extend(lines_of(out.stderr.as_bytes()));
            lines
        };
        // With the links repaired, each linked worktree is bound to the hooks of its own
        // tree: the repair is how a worktree moved or made by hand rejoins the topology.
        let hooks = if dry_run {
            BTreeMap::new()
        } else {
            self.identity
                .registered_worktrees()
                .iter()
                .skip(1)
                .filter(|r| r.path.is_dir())
                .map(|r| (display(&r.path), hooks::carry_hooks_path(primary, &r.path)))
                .collect()
        };
        Ok(RepairReport {
            pruned,
            repaired,
            hooks,
            applied: !dry_run,
        })
    }
}

/// Is any component of `path` below `container` a symbolic link? A link inside the container
/// pointing at a worktree elsewhere would make that worktree canonicalise to its expected
/// path; the expected path has to be a real directory, not a name for one somewhere else.
fn symlink_below(container: &Path, path: &Path) -> bool {
    let Ok(rest) = path.strip_prefix(container) else {
        return false;
    };
    let mut cursor = container.to_path_buf();
    for c in rest.components() {
        cursor.push(c);
        if std::fs::symlink_metadata(&cursor)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

/// Whether a judged work tree is where it belongs: canonical, or the primary checkout on
/// the trunk (or with no trunk to be held to), or detached.
fn is_in_place(w: &WorktreeState, trunk_source: TrunkSource) -> bool {
    match w.standing {
        Standing::Canonical | Standing::Detached => true,
        Standing::Primary => {
            trunk_source == TrunkSource::Unknown
                || !w
                    .diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::PrimaryOnNonTrunk)
        }
        Standing::Misplaced | Standing::Missing | Standing::Ephemeral => false,
    }
}

/// The scratch roots the distribution declares, expanded against `primary`, in declaration
/// order: the tool's own first, then each provider's. Located the way every command locates
/// the share (`--share` excepted, which this path does not carry), from `current` — the
/// checkout the command runs in, so that a repository supervising itself reads the
/// declarations of the branch it is on rather than the primary checkout's; a checkout with
/// no share in reach declares none. The standard library's answer for the temporary
/// directory is added when the declarations name `$TMPDIR` and the environment does not, so
/// that what the tool itself creates under `std::env::temp_dir()` is recognised on every
/// platform.
pub fn declared_scratch_roots(current: &Path, primary: &Path) -> Vec<ScratchRoot> {
    let Ok(share) = crate::share::Share::locate(None, current) else {
        return Vec::new();
    };
    let Ok(decls) = share.providers() else {
        return Vec::new();
    };
    scratch_roots_from(&decls, primary)
}

/// [`declared_scratch_roots`] over declarations already read.
pub fn scratch_roots_from(
    decls: &crate::share::ProviderDeclarations,
    primary: &Path,
) -> Vec<ScratchRoot> {
    let mut out: Vec<ScratchRoot> = decls
        .scratch_roots(primary)
        .into_iter()
        .map(|(provider, path)| ScratchRoot { provider, path })
        .collect();
    if decls.scratch_roots.iter().any(|r| r == "$TMPDIR") {
        let temp = std::env::temp_dir();
        if !out.iter().any(|r| r.provider.is_none() && r.path == temp) {
            out.push(ScratchRoot {
                provider: None,
                path: temp,
            });
        }
    }
    out
}

fn lines_of(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// A path as text. Absolute, as git holds it; nothing is shortened or made relative, because
/// these strings are copied into commands.
pub(crate) fn display(p: &Path) -> String {
    p.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The distribution beside this crate, as the shipped declarations.
    fn dist_declarations() -> crate::share::ProviderDeclarations {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share");
        crate::share::Share::locate(Some(&dir), &dir)
            .unwrap()
            .providers()
            .unwrap()
    }

    /// A repository with a primary checkout and whatever linked worktrees a test asks for.
    /// Under the temporary directory on purpose: `ephemeral_root_of` disables the temporary
    /// roots for a repository that is itself inside one, so a worktree outside the container
    /// is `Misplaced` rather than `Ephemeral`.
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

        fn worktree(&self, branch: &str, at: &Path) {
            if let Some(parent) = at.parent() {
                std::fs::create_dir_all(parent).expect("mkdir");
            }
            self.git(&["worktree", "add", "-q", "-b", branch, &at.to_string_lossy()]);
        }

        fn service(&self) -> WorktreeService {
            WorktreeService::open(&self.primary).expect("a service")
        }
    }

    #[test]
    fn a_worktree_resolves_by_branch_or_by_path_and_says_so_when_it_is_neither() {
        let repo = Repo::new();
        let svc = repo.service();
        let home = svc.expected_path_of("feature/x").expect("a path");
        repo.worktree("feature/x", &home);
        let svc = repo.service();

        // by branch
        let by_branch = svc.resolve("feature/x").expect("resolved by branch");
        assert_eq!(by_branch.branch.as_deref(), Some("feature/x"));
        // by path, which is the same record
        let by_path = svc
            .resolve(&home.to_string_lossy())
            .expect("resolved by path");
        assert_eq!(by_path.path, by_branch.path);
        // and a selector that is neither names itself in the refusal
        let err = svc.resolve("feature/never").expect_err("no such worktree");
        assert!(err.to_string().contains("feature/never"), "{err}");
    }

    #[test]
    fn a_branch_whose_worktree_is_home_passes_the_guard_and_one_that_is_not_does_not() {
        let repo = Repo::new();
        let home = repo
            .service()
            .expected_path_of("feature/home")
            .expect("path");
        repo.worktree("feature/home", &home);

        // the guard is asked from inside a worktree; open the service there
        let inside = WorktreeService::open(&home).expect("a service inside the worktree");
        let verdict = inside.guard().expect("a verdict");
        assert!(verdict.ok, "{verdict:?}");
        assert!(verdict.reason.is_none());
        assert_eq!(verdict.branch.as_deref(), Some("feature/home"));

        // and one that is misplaced fails it, with the reason and the destination
        let away = repo.primary.parent().unwrap().join("elsewhere");
        repo.worktree("feature/away", &away);
        let outside = WorktreeService::open(&away).expect("a service inside the worktree");
        let verdict = outside.guard().expect("a verdict");
        assert!(!verdict.ok, "{verdict:?}");
        assert!(verdict.reason.is_some(), "a refusal carries its reason");
        assert!(verdict.expected_path.is_some(), "and where it belongs");
    }

    #[test]
    fn the_expected_path_of_a_branch_is_under_the_container_and_a_bad_name_is_refused() {
        let repo = Repo::new();
        let svc = repo.service();
        let container = svc.container().path.clone();

        let path = svc.expected_path_of("feature/nested/deep").expect("a path");
        assert!(path.starts_with(&container), "{path:?}");
        assert!(path.ends_with("feature/nested/deep"), "{path:?}");

        // a name that would climb out of the container is refused rather than resolved
        for bad in ["../escape", "..", "feature/../../escape"] {
            assert!(
                svc.expected_path_of(bad).is_err(),
                "{bad} resolved to a path"
            );
        }
    }

    #[test]
    fn inspecting_a_branch_reports_where_it_belongs_whether_or_not_it_is_there() {
        let repo = Repo::new();
        let svc = repo.service();

        // a branch with no worktree still has a canonical path
        let absent = svc.inspect("feature/absent").expect("a report");
        assert!(
            absent.expected_path.contains("feature/absent"),
            "{}",
            absent.expected_path
        );

        let home = svc.expected_path_of("feature/there").expect("a path");
        repo.worktree("feature/there", &home);
        let present = repo.service().inspect("feature/there").expect("a report");
        assert_eq!(present.expected_path, display(&home));
    }

    #[test]
    fn the_declared_scratch_roots_name_the_temporary_directory_and_every_providers_own() {
        let primary = Path::new("/srv/repo");
        let roots = scratch_roots_from(&dist_declarations(), primary);
        let own: Vec<&PathBuf> = roots
            .iter()
            .filter(|r| r.provider.is_none())
            .map(|r| &r.path)
            .collect();
        assert!(
            own.contains(&&std::env::temp_dir()),
            "the tool's own roots hold the temporary directory: {own:?}"
        );
        assert!(own.contains(&&PathBuf::from("/tmp")));
        let claude = roots
            .iter()
            .find(|r| r.provider.as_deref() == Some("claude-code"))
            .expect("claude-code declares a root");
        assert_eq!(
            claude.path,
            PathBuf::from("/srv/repo/.claude/worktrees"),
            "expanded against the primary checkout"
        );
        let bb = roots
            .iter()
            .find(|r| r.provider.as_deref() == Some("bb"))
            .expect("bb declares a root");
        assert!(
            bb.path
                .ends_with("plugins/environment-git-worktree/host-data/worktrees"),
            "{}",
            bb.path.display()
        );
        let home = std::env::var("HOME").unwrap();
        assert!(
            bb.path
                .starts_with(std::env::var("BB_DATA_DIR").unwrap_or(format!("{home}/.bb"))),
            "the data directory defaults to ~/.bb: {}",
            bb.path.display()
        );
    }

    #[test]
    fn a_checkout_under_an_orchestrators_root_is_a_scratch_checkout_named_after_it() {
        let home = tempfile::tempdir().unwrap();
        let primary = home.path().join("elsewhere/repo");
        std::fs::create_dir_all(&primary).unwrap();
        let run = |args: &[&str]| {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(&primary)
                .args(args)
                .status()
                .unwrap()
                .success());
        };
        run(&["init", "-q", "."]);
        run(&[
            "-c",
            "user.email=t@example.com",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "i",
        ]);
        let identity = RepositoryIdentity::discover(&primary).unwrap();
        // the declarations expanded with bb's data directory pointed into the fixture, so
        // that nothing about this machine's own installation is read
        let data = home.path().join("bb-data");
        let roots: Vec<ScratchRoot> = scratch_roots_from(&dist_declarations(), &primary)
            .into_iter()
            .map(|r| match r.provider.as_deref() {
                Some("bb") => ScratchRoot {
                    provider: r.provider,
                    path: data.join("plugins/environment-git-worktree/host-data/worktrees"),
                },
                _ => r,
            })
            .collect();
        let svc = WorktreeService::over_with(identity, roots).unwrap();
        let thread = ResolvedPath::of(
            data.join("plugins/environment-git-worktree/host-data/worktrees/thr_1/repo"),
        );
        let root = svc
            .scratch_root_of(&thread)
            .expect("a thread's checkout is a scratch checkout");
        assert_eq!(root.provider.as_deref(), Some("bb"));
        let sibling = ResolvedPath::of(home.path().join("sibling"));
        assert!(svc.scratch_root_of(&sibling).is_none());
    }

    #[test]
    fn a_checkout_under_the_temporary_directory_is_ephemeral_unless_the_repository_lives_there() {
        // A real repository whose primary checkout is *outside* the temporary directory: a
        // worktree under it is a session's scratch checkout.
        let home = tempfile::tempdir().unwrap();
        let primary = home.path().join("elsewhere/repo");
        std::fs::create_dir_all(&primary).unwrap();
        let run = |args: &[&str]| {
            assert!(std::process::Command::new("git")
                .arg("-C")
                .arg(&primary)
                .args(args)
                .status()
                .unwrap()
                .success());
        };
        run(&["init", "-q", "."]);
        run(&[
            "-c",
            "user.email=t@example.com",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "i",
        ]);
        let identity = RepositoryIdentity::discover(&primary).unwrap();
        let svc = WorktreeService::over_with(
            identity,
            scratch_roots_from(&dist_declarations(), &primary),
        )
        .unwrap();
        // the fixture itself lives under the temporary directory, so the temporary roots are
        // disabled for it, and only the agent directory rule applies
        let agent = ResolvedPath::of(primary.join(".claude/worktrees/x"));
        assert_eq!(
            svc.scratch_root_of(&agent).unwrap().provider.as_deref(),
            Some("claude-code")
        );
        let sibling = ResolvedPath::of(home.path().join("sibling"));
        assert!(svc.ephemeral_root_of(&sibling).is_none(), "the repository lives in the temporary directory, so nothing beside it is a scratch checkout");
    }

    /// A report written before the hooks were carried reads as hooks that were not
    /// configured, never as a parse failure.
    #[test]
    fn a_create_report_without_hooks_reads_as_not_configured() {
        assert_eq!(default_hooks(), HooksPath::NotConfigured);
    }

    /// A repair binds every existing linked worktree to its hooks and says which, by path;
    /// a dry run changes nothing and binds nothing.
    #[test]
    fn a_repair_names_the_hooks_of_every_linked_worktree_and_a_dry_run_none() {
        let repo = Repo::new();
        let home = repo.service().expected_path_of("topic").expect("a path");
        repo.worktree("topic", &home);
        let svc = repo.service();

        let dry = svc.repair(true).expect("a dry run");
        assert!(!dry.applied);
        assert!(dry.hooks.is_empty(), "{dry:?}");

        let applied = svc.repair(false).expect("a repair");
        assert!(applied.applied);
        let paths: Vec<&String> = applied.hooks.keys().collect();
        assert_eq!(paths.len(), 1, "{applied:?}");
        assert!(paths[0].ends_with("topic"), "{applied:?}");
    }

    /// Run git in the primary checkout with arguments that need not be UTF-8.
    fn git_bytes(repo: &Repo, args: &[&[u8]]) {
        use std::os::unix::ffi::OsStrExt;
        let ok = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo.primary)
            .args(args.iter().map(|a| std::ffi::OsStr::from_bytes(a)))
            .output()
            .expect("git runs")
            .status
            .success();
        assert!(ok, "git {args:?}");
    }

    /// A repository whose trunk is pushed to a bare `origin` beside it and tracks it there.
    fn with_origin() -> (Repo, PathBuf) {
        let repo = Repo::new();
        let origin = repo._home.path().join("origin.git");
        let url = origin.to_string_lossy().to_string();
        repo.git(&["init", "-q", "--bare", &url]);
        repo.git(&["remote", "add", "origin", &url]);
        repo.git(&["push", "-q", "-u", "origin", "master"]);
        (repo, origin)
    }

    /// Put `branch` on the remote one commit ahead of the trunk without leaving a local
    /// branch or a remote-tracking ref for it.
    fn push_only(repo: &Repo, branch: &str) {
        repo.git(&["checkout", "-q", "-b", "side"]);
        repo.git(&["commit", "-q", "--allow-empty", "-m", "side"]);
        repo.git(&["push", "-q", "origin", &format!("side:{branch}")]);
        repo.git(&["checkout", "-q", "master"]);
        repo.git(&["branch", "-q", "-D", "side"]);
        repo.git(&["update-ref", "-d", &format!("refs/remotes/origin/{branch}")]);
    }

    fn request(branch: &str, base: Option<&str>) -> CreateRequest {
        CreateRequest {
            branch: branch.into(),
            base: base.map(str::to_string),
        }
    }

    #[test]
    fn a_start_point_names_its_ref_and_says_when_it_is_not_the_remotes_state() {
        let given = StartPoint::Given {
            reference: "v1.0".into(),
        };
        assert_eq!(given.reference(), "v1.0");
        assert_eq!(given.fallback(), None);
        assert_eq!(StartPoint::Head.reference(), "HEAD");
        assert_eq!(StartPoint::Head.fallback(), None);
        let local = |behind, reason: Option<&str>| StartPoint::LocalTrunk {
            branch: "master".into(),
            unreachable: Some("origin".into()),
            reason: reason.map(str::to_string),
            behind,
        };
        assert_eq!(local(Some(0), None).reference(), "master");
        let level = local(Some(0), None).fallback().expect("said");
        assert!(level.contains("(it did not answer)"), "{level}");
        assert!(level.contains("level with origin/master"), "{level}");
        let one = local(Some(1), Some("offline")).fallback().expect("said");
        assert!(one.contains("(offline)"), "{one}");
        assert!(one.contains("1 commit behind origin/master"), "{one}");
        let many = local(Some(4), Some("offline")).fallback().expect("said");
        assert!(many.contains("4 commits behind origin/master"), "{many}");
        let never = local(None, Some("offline")).fallback().expect("said");
        assert!(never.contains("was never fetched"), "{never}");
        let alone = StartPoint::LocalTrunk {
            branch: "master".into(),
            unreachable: None,
            reason: None,
            behind: None,
        };
        assert_eq!(alone.fallback(), None);
        let stale = StartPoint::RemoteBranch {
            remote: "origin".into(),
            branch: "fix/x".into(),
            fetched: false,
        };
        assert_eq!(stale.reference(), "origin/fix/x");
        let said = stale.fallback().expect("said");
        assert!(
            said.contains("tracking origin/fix/x as last fetched"),
            "{said}"
        );
        let fresh = StartPoint::RemoteBranch {
            remote: "origin".into(),
            branch: "fix/x".into(),
            fetched: true,
        };
        assert_eq!(fresh.fallback(), None);
    }

    #[test]
    fn the_trunks_remote_is_the_one_whose_head_names_it_else_the_configured_one() {
        let home = tempfile::tempdir().expect("tempdir");
        // no repository there: `git remote` refuses, and no remote is named
        let absent = home.path().join("absent");
        assert_eq!(trunk_remote(&absent, "master").expect("an answer"), None);
        // a git that cannot be run is an error, not an answer
        assert!(trunk_remote(Path::new("no\0where"), "master").is_err());

        let (repo, _origin) = with_origin();
        let p = repo.primary.clone();
        // pushed with -u: the configuration names origin, which has no HEAD
        assert_eq!(
            trunk_remote(&p, "master").expect("an answer"),
            Some("origin".into())
        );
        // a HEAD that names another branch is passed over; one that names the trunk wins
        let other = repo._home.path().join("other.git");
        repo.git(&["init", "-q", "--bare", &other.to_string_lossy()]);
        repo.git(&["remote", "add", "other", &other.to_string_lossy()]);
        repo.git(&[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/develop",
        ]);
        repo.git(&[
            "symbolic-ref",
            "refs/remotes/other/HEAD",
            "refs/remotes/other/master",
        ]);
        assert_eq!(
            trunk_remote(&p, "master").expect("an answer"),
            Some("other".into())
        );
        // a trunk configured to a remote that is not there lives only here
        repo.git(&["config", "branch.lonely.remote", "gone"]);
        assert_eq!(trunk_remote(&p, "lonely").expect("an answer"), None);
        // a trunk git cannot be asked about is an error
        assert!(trunk_remote(&p, "ma\0ster").is_err());
        // a configured remote that is not text is an error
        git_bytes(&repo, &[b"config", b"branch.odd.remote", b"o\xff"]);
        assert!(trunk_remote(&p, "odd").is_err());
        // a HEAD that is not text is an error
        repo.git(&["symbolic-ref", "-d", "refs/remotes/other/HEAD"]);
        std::fs::write(
            p.join(".git/refs/remotes/origin/HEAD"),
            b"ref: refs/remotes/origin/\xff\n",
        )
        .expect("write");
        assert!(trunk_remote(&p, "master").is_err());
    }

    #[test]
    fn a_remote_whose_name_is_not_text_refuses_the_create_before_anything_is_made() {
        let (repo, _origin) = with_origin();
        let svc = repo.service();
        git_bytes(&repo, &[b"remote", b"add", b"o\xff", b"/nowhere"]);
        assert!(trunk_remote(&repo.primary, "master").is_err());
        assert!(svc.create(&request("feature/odd", None)).is_err());
        // and so is one whose start is planned only under the lock
        let (name, target) = placed(&svc, "feature/odd");
        assert!(svc
            .create_locked(&request("feature/odd", None), false, name, target, None)
            .is_err());
        assert!(!svc
            .expected_path_of("feature/odd")
            .expect("a path")
            .exists());
    }

    #[test]
    fn a_repository_with_no_trunk_starts_from_head_and_one_with_no_remote_from_the_trunk() {
        let repo = Repo::new();
        assert_eq!(
            repo.service()
                .plan_start(&repo.primary, "feature/x")
                .expect("a plan"),
            StartPoint::LocalTrunk {
                branch: "master".into(),
                unreachable: None,
                reason: None,
                behind: None,
            }
        );
        repo.git(&["branch", "-m", "master", "topic"]);
        repo.git(&["checkout", "-q", "--detach"]);
        let svc = repo.service();
        if svc.identity.trunk().branch.is_some() {
            eprintln!("a trunk was found in the environment's configuration; nothing to test");
            return;
        }
        assert_eq!(
            svc.plan_start(&repo.primary, "feature/x").expect("a plan"),
            StartPoint::Head
        );
        let made = svc.create(&request("feature/x", None)).expect("created");
        assert_eq!(made.base.as_deref(), Some("HEAD"));
    }

    #[test]
    fn the_remote_decides_where_a_branch_starts_and_its_last_fetched_refs_when_it_cannot() {
        let (repo, origin) = with_origin();
        let p = repo.primary.clone();
        let svc = repo.service();
        let plan = |b: &str| svc.plan_start(&p, b).expect("a plan");
        let remote_trunk = StartPoint::RemoteTrunk {
            remote: "origin".into(),
            branch: "master".into(),
        };

        // the remote has it: taken up as it is there
        push_only(&repo, "fix/theirs");
        assert_eq!(
            plan("fix/theirs"),
            StartPoint::RemoteBranch {
                remote: "origin".into(),
                branch: "fix/theirs".into(),
                fetched: true,
            }
        );
        // the remote does not have it: the remote's trunk
        assert_eq!(plan("feature/new"), remote_trunk);

        // the trunk arrives and the branch cannot be written here: never fetched, the
        // remote's trunk; fetched before, as last fetched
        push_only(&repo, "fix/locked");
        let lock = p.join(".git/refs/remotes/origin/fix/locked.lock");
        std::fs::create_dir_all(lock.parent().expect("a parent")).expect("mkdir");
        std::fs::write(&lock, "").expect("write");
        assert_eq!(plan("fix/locked"), remote_trunk);
        std::fs::remove_file(&lock).expect("unlock");
        repo.git(&["update-ref", "refs/remotes/origin/fix/locked", "master"]);
        std::fs::write(&lock, "").expect("lock again");
        assert_eq!(
            plan("fix/locked"),
            StartPoint::RemoteBranch {
                remote: "origin".into(),
                branch: "fix/locked".into(),
                fetched: false,
            }
        );
        std::fs::remove_file(&lock).expect("unlock");

        // the remote cannot be read: a branch known here is taken up, a new one starts
        // from the local trunk, level with the trunk as last fetched
        let gone = origin.with_file_name("gone.git");
        repo.git(&["remote", "set-url", "origin", &gone.to_string_lossy()]);
        assert_eq!(
            plan("fix/locked"),
            StartPoint::RemoteBranch {
                remote: "origin".into(),
                branch: "fix/locked".into(),
                fetched: false,
            }
        );
        match plan("feature/offline") {
            StartPoint::LocalTrunk {
                branch,
                unreachable,
                reason,
                behind,
            } => {
                assert_eq!(branch, "master");
                assert_eq!(unreachable.as_deref(), Some("origin"));
                assert!(reason.is_some());
                assert_eq!(behind, Some(0));
            }
            other => panic!("{other:?}"),
        }
        // a branch git cannot be asked about is an error, not a guess
        assert!(svc.plan_start(&p, "feature/no\0pe").is_err());

        // a remote without the trunk says so, and how far behind is unknown
        let empty = origin.with_file_name("empty.git");
        repo.git(&["init", "-q", "--bare", &empty.to_string_lossy()]);
        repo.git(&["remote", "set-url", "origin", &empty.to_string_lossy()]);
        repo.git(&["update-ref", "-d", "refs/remotes/origin/master"]);
        assert_eq!(
            plan("feature/empty"),
            StartPoint::LocalTrunk {
                branch: "master".into(),
                unreachable: Some("origin".into()),
                reason: Some("origin has no branch master".into()),
                behind: None,
            }
        );
    }

    #[test]
    fn a_given_base_is_taken_as_given_and_an_existing_branch_is_checked_out_as_it_is() {
        let repo = Repo::new();
        let svc = repo.service();
        let made = svc
            .create(&request("feature/given", Some("master")))
            .expect("created");
        assert_eq!(
            made.start,
            Some(StartPoint::Given {
                reference: "master".into()
            })
        );
        assert!(made.branch_created);
        assert!(made.start_commit.is_some());
        assert!(svc
            .create(&request("feature/nothing", Some("no-such-ref")))
            .is_err());

        repo.git(&["branch", "feature/there"]);
        let svc = repo.service();
        let made = svc
            .create(&request("feature/there", None))
            .expect("created");
        assert!(!made.branch_created);
        assert_eq!(made.start, None);
        assert_eq!(made.base, None);
        assert_eq!(made.start_commit, None);
    }

    /// A branch deleted between the look taken before the lock and the one taken under it is
    /// planned under the lock, rather than created from nowhere.
    /// A branch and the place `create_inner` would have checked for it.
    fn placed(svc: &WorktreeService, branch: &str) -> (BranchName, PathBuf) {
        let name = BranchName::parse(branch).expect("a branch name");
        let target = path::expected_path(&svc.container.path, &name).expect("a place");
        (name, target)
    }

    #[test]
    fn a_branch_deleted_while_the_create_waits_for_the_lock_is_planned_under_it() {
        // The branch existed when the create looked before the lock, so nothing was planned;
        // by the time the lock is held it is gone, and the start is planned under the lock.
        let repo = Repo::new();
        let svc = repo.service();
        let (name, target) = placed(&svc, "feature/raced");
        let made = svc
            .create_locked(&request("feature/raced", None), false, name, target, None)
            .expect("created");
        assert!(made.branch_created);
        assert_eq!(
            made.start,
            Some(StartPoint::LocalTrunk {
                branch: "master".into(),
                unreachable: None,
                reason: None,
                behind: None,
            })
        );
    }
}
