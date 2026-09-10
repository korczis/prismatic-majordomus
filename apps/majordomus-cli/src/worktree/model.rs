//! The typed topology: one Rust source of truth that serde, JSON Schema, OpenAPI, MCP, the
//! command line and the Cockpit all render. Nothing in here is decided; it is what the
//! service decided, written down.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The `schema` every topology document carries.
pub const SCHEMA: &str = "majordomus/worktree-topology/v1";

/// Whether a work tree is the repository's own checkout or one linked to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WorktreeKind {
    /// The main work tree. It hosts the trunk, is never moved and is never removed.
    Primary,
    /// A linked work tree. Belongs at its branch's canonical path.
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

/// Where a work tree stands against the topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
    /// The primary checkout: exempt from the path rule, held to the trunk rule.
    Primary,
    /// A linked work tree at exactly its branch's canonical path.
    Canonical,
    /// A linked work tree somewhere else. Migration brings it home.
    Misplaced,
    /// A linked work tree with no branch. It has no canonical path and is never moved.
    Detached,
    /// A linked work tree under the operating system's temporary directory or under the
    /// primary checkout's `.claude/worktrees/`: a session's scratch checkout, owned and
    /// removed by the harness that made it. Reported; migrated only on request.
    Ephemeral,
    /// A registration whose directory is gone. Repair drops it.
    Missing,
}

impl Standing {
    /// The word this standing is reported under.
    pub fn as_str(self) -> &'static str {
        match self {
            Standing::Primary => "primary",
            Standing::Canonical => "canonical",
            Standing::Misplaced => "misplaced",
            Standing::Detached => "detached",
            Standing::Ephemeral => "ephemeral",
            Standing::Missing => "missing",
        }
    }

    /// Whether this standing satisfies the path rule. `Primary` and `Detached` are not
    /// subject to it; `Missing` fails it because a stale registration is a defect;
    /// `Ephemeral` fails it — a branch is being worked on where it does not belong — while
    /// the topology as a whole stays valid, because the harness that made the checkout
    /// removes it.
    pub fn is_acceptable(self) -> bool {
        !matches!(
            self,
            Standing::Misplaced | Standing::Missing | Standing::Ephemeral
        )
    }
}

/// How serious a diagnostic is: the executable's one severity vocabulary, reused rather than
/// redefined. `Error` means the topology is invalid while it stands and the guard refuses;
/// `Warning` is worth acting on; `Info` is a fact worth showing.
pub use crate::model::Severity;

/// The stable machine name of everything that can be wrong with the topology. One code per
/// condition, reused by the command line, the API, MCP, the Cockpit, the tests and the
/// documentation; the prose beside a code is rendered from the typed state.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub enum DiagnosticCode {
    /// A linked work tree is not at its branch's canonical path.
    #[serde(rename = "worktree.path_mismatch")]
    PathMismatch,
    /// A linked work tree occupies the container path itself.
    #[serde(rename = "worktree.container_occupied")]
    ContainerOccupied,
    /// A linked work tree sits inside the primary checkout or inside another work tree.
    #[serde(rename = "worktree.nested")]
    Nested,
    /// The canonical path of a branch is occupied by something that is not its work tree.
    #[serde(rename = "worktree.destination_conflict")]
    DestinationConflict,
    /// A registered work tree's directory does not exist.
    #[serde(rename = "worktree.missing")]
    Missing,
    /// Git reports the registration as prunable.
    #[serde(rename = "worktree.stale_registration")]
    StaleRegistration,
    /// A branch is checked out somewhere other than its canonical path.
    #[serde(rename = "worktree.branch_already_checked_out")]
    BranchAlreadyCheckedOut,
    /// A linked work tree has no branch.
    #[serde(rename = "worktree.detached")]
    Detached,
    /// A linked work tree is a session's scratch checkout: under the temporary directory or
    /// under the primary checkout's `.claude/worktrees/`.
    #[serde(rename = "worktree.ephemeral")]
    Ephemeral,
    /// The primary checkout holds a branch that is not the trunk.
    #[serde(rename = "worktree.primary_on_non_trunk")]
    PrimaryOnNonTrunk,
    /// The trunk is checked out in a linked work tree rather than the primary checkout.
    #[serde(rename = "worktree.trunk_in_linked_worktree")]
    TrunkInLinkedWorktree,
    /// A derived path would leave the container. Cannot happen for a valid branch name.
    #[serde(rename = "worktree.path_escape")]
    PathEscape,
    /// A branch name git accepted that this executable cannot derive a path for.
    #[serde(rename = "worktree.invalid_branch_name")]
    InvalidBranchName,
    /// A work tree is locked, so it cannot be moved until it is unlocked.
    #[serde(rename = "worktree.locked")]
    Locked,
    /// A move happened and the fingerprint after it differs from the one before.
    #[serde(rename = "worktree.migration_verification_failed")]
    MigrationVerificationFailed,
    /// The trunk could not be determined.
    #[serde(rename = "worktree.trunk_unknown")]
    TrunkUnknown,
    /// Two branch names map to one directory on a case-insensitive filesystem.
    #[serde(rename = "worktree.case_collision")]
    CaseCollision,
    /// A move crossed devices and was made by copy, repair and verification.
    #[serde(rename = "worktree.cross_device")]
    CrossDevice,
    /// direnv refuses the work tree's `.envrc` until `direnv allow` runs at its path.
    #[serde(rename = "worktree.envrc_blocked")]
    EnvrcBlocked,
}

impl DiagnosticCode {
    /// The dotted code, as serialised.
    ///
    /// ```
    /// use majordomus_cli::worktree::DiagnosticCode;
    /// assert_eq!(DiagnosticCode::PathMismatch.as_str(), "worktree.path_mismatch");
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticCode::PathMismatch => "worktree.path_mismatch",
            DiagnosticCode::ContainerOccupied => "worktree.container_occupied",
            DiagnosticCode::Nested => "worktree.nested",
            DiagnosticCode::DestinationConflict => "worktree.destination_conflict",
            DiagnosticCode::Missing => "worktree.missing",
            DiagnosticCode::StaleRegistration => "worktree.stale_registration",
            DiagnosticCode::BranchAlreadyCheckedOut => "worktree.branch_already_checked_out",
            DiagnosticCode::Detached => "worktree.detached",
            DiagnosticCode::Ephemeral => "worktree.ephemeral",
            DiagnosticCode::PrimaryOnNonTrunk => "worktree.primary_on_non_trunk",
            DiagnosticCode::TrunkInLinkedWorktree => "worktree.trunk_in_linked_worktree",
            DiagnosticCode::PathEscape => "worktree.path_escape",
            DiagnosticCode::InvalidBranchName => "worktree.invalid_branch_name",
            DiagnosticCode::Locked => "worktree.locked",
            DiagnosticCode::MigrationVerificationFailed => "worktree.migration_verification_failed",
            DiagnosticCode::TrunkUnknown => "worktree.trunk_unknown",
            DiagnosticCode::CaseCollision => "worktree.case_collision",
            DiagnosticCode::CrossDevice => "worktree.cross_device",
            DiagnosticCode::EnvrcBlocked => "worktree.envrc_blocked",
        }
    }

    /// Every code, for the documentation and the tests that hold it complete.
    pub const ALL: &'static [DiagnosticCode] = &[
        DiagnosticCode::PathMismatch,
        DiagnosticCode::ContainerOccupied,
        DiagnosticCode::Nested,
        DiagnosticCode::DestinationConflict,
        DiagnosticCode::Missing,
        DiagnosticCode::StaleRegistration,
        DiagnosticCode::BranchAlreadyCheckedOut,
        DiagnosticCode::Detached,
        DiagnosticCode::Ephemeral,
        DiagnosticCode::PrimaryOnNonTrunk,
        DiagnosticCode::TrunkInLinkedWorktree,
        DiagnosticCode::PathEscape,
        DiagnosticCode::InvalidBranchName,
        DiagnosticCode::Locked,
        DiagnosticCode::MigrationVerificationFailed,
        DiagnosticCode::TrunkUnknown,
        DiagnosticCode::CaseCollision,
        DiagnosticCode::CrossDevice,
        DiagnosticCode::EnvrcBlocked,
    ];
}

/// One thing wrong with, or worth knowing about, the topology.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TopologyDiagnostic {
    /// The stable code.
    pub code: DiagnosticCode,
    /// How serious it is.
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The work tree involved.
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch involved.
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where the work tree belongs.
    pub expected: Option<String>,
    /// What is wrong, in one line.
    pub message: String,
    /// The command that addresses it.
    pub remedy: String,
}

/// Uncommitted work in a work tree, counted from `git status --porcelain`. Untracked
/// content counts: it is exactly what a careless move loses.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DirtyState {
    /// Entries with a change in the index.
    pub staged: usize,
    /// Entries with a change in the work tree that is not in the index.
    pub unstaged: usize,
    /// Untracked, not ignored, files.
    pub untracked: usize,
    /// Entries with an unresolved merge conflict.
    pub conflicted: usize,
    /// Nothing above is non-zero.
    pub clean: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A merge, rebase, cherry-pick, revert or bisect is in progress here.
    pub in_progress: Option<String>,
}

impl DirtyState {
    /// Every count, in words, for a message.
    pub fn summary(&self) -> String {
        if self.clean {
            return "clean".into();
        }
        let mut parts = Vec::new();
        if self.staged > 0 {
            parts.push(format!("{} staged", self.staged));
        }
        if self.unstaged > 0 {
            parts.push(format!("{} unstaged", self.unstaged));
        }
        if self.untracked > 0 {
            parts.push(format!("{} untracked", self.untracked));
        }
        if self.conflicted > 0 {
            parts.push(format!("{} conflicted", self.conflicted));
        }
        if let Some(op) = &self.in_progress {
            parts.push(format!("{op} in progress"));
        }
        parts.join(", ")
    }
}

/// A branch's upstream and how far the two have moved apart, from `for-each-ref` in one
/// subprocess for every branch, never a fetch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UpstreamState {
    /// The upstream ref, short (`origin/master`).
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Commits here that the upstream lacks.
    pub ahead: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Commits upstream that this branch lacks.
    pub behind: Option<usize>,
    /// The upstream ref no longer exists.
    pub gone: bool,
}

/// Whether direnv loads a work tree's `.envrc` on entry. direnv approves by path and
/// content, so the repository's one tracked `.envrc` has a standing per work tree, and a
/// tree that starts blocked is the recurring "direnv does not work again".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnvrcStanding {
    /// No `.envrc` here: nothing for direnv to load, nothing blocked.
    None,
    /// direnv loads it on entry.
    Approved,
    /// direnv refuses it until `direnv allow` runs at this path.
    Blocked,
    /// direnv is not on the PATH, or its status could not be read; nothing can say.
    Unknown,
}

impl EnvrcStanding {
    /// The word this standing is reported under.
    pub fn as_str(self) -> &'static str {
        match self {
            EnvrcStanding::None => "none",
            EnvrcStanding::Approved => "approved",
            EnvrcStanding::Blocked => "blocked",
            EnvrcStanding::Unknown => "unknown",
        }
    }
}

/// A work tree's `.envrc` under direnv: its standing, and whether the file is the primary
/// checkout's — which decides the remedy, because the primary checkout's approval carries
/// to an identical file and to nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnvrcState {
    /// Whether direnv loads it.
    pub standing: EnvrcStanding,
    /// Byte-for-byte the primary checkout's `.envrc`.
    pub same_as_primary: bool,
}

/// One work tree, as the topology sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeState {
    /// Absolute, as git holds it.
    pub path: String,
    /// Primary or linked.
    pub kind: WorktreeKind,
    /// Where it stands against the topology.
    pub standing: Standing,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch checked out here, short. Absent when detached.
    pub branch: Option<String>,
    /// The name it is shown under: the branch, or `detached/<short commit>`.
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commit checked out here.
    pub head: Option<String>,
    /// HEAD is detached here.
    pub detached: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where this branch's work tree belongs. Absent when detached, or for the primary
    /// checkout on the trunk.
    pub expected_path: Option<String>,
    /// The directory git registered still exists on disk.
    pub exists: bool,
    /// The call came from inside this work tree.
    pub current: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Locked, with git's reason; an empty string when it recorded none.
    pub locked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Prunable, with git's reason.
    pub prunable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Uncommitted work here. Absent when it was not asked for: it costs one subprocess per
    /// work tree, and a topology check does not need it.
    pub dirty: Option<DirtyState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch's upstream and its distance from it.
    pub upstream: Option<UpstreamState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The issue this branch provably names: a path component of the branch equal to an
    /// issue id of `.ai/repo/project/issues/`, or beginning with it and a hyphen. Nothing
    /// is inferred from similarity.
    pub issue: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Its `.envrc` under direnv. Absent when it was not asked for: it costs one `direnv
    /// status` per work tree that has an `.envrc`, and a topology check does not need it.
    pub envrc: Option<EnvrcState>,
    /// What is wrong with this work tree, if anything.
    pub diagnostics: Vec<TopologyDiagnostic>,
}

/// One local branch, with or without a work tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BranchState {
    /// The branch, short.
    pub name: String,
    /// The commit it points at.
    pub head: String,
    /// This branch is the trunk.
    pub trunk: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where its work tree belongs.
    pub expected_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where it is checked out, when it is.
    pub worktree: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The upstream and its distance.
    pub upstream: Option<UpstreamState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Reachable from the trunk. Absent when the trunk is unknown.
    pub merged_into_trunk: Option<bool>,
    /// Merged into the trunk, not the trunk, and either not checked out or checked out in a
    /// work tree known to be clean. Derived state, never acted on automatically.
    pub cleanup_eligible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The issue this branch provably names.
    pub issue: Option<String>,
}

/// The repository's identity, as the topology reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryView {
    /// The primary checkout: what the container is named after.
    pub primary_worktree: String,
    /// The common git directory: the repository's identity.
    pub git_common_dir: String,
    /// The primary checkout's directory name.
    pub name: String,
}

/// The container, as the topology reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ContainerView {
    /// The derived path.
    pub path: String,
    /// The suffix it was derived with.
    pub suffix: String,
    /// It exists on disk. Created by the first worktree that needs it.
    pub exists: bool,
}

/// The trunk, as the topology reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TrunkView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch, when known.
    pub branch: Option<String>,
    /// How it was decided.
    pub source: super::identity::TrunkSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where it is checked out.
    pub checked_out_at: Option<String>,
}

/// How many work trees and branches are in each state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TopologyTallies {
    /// Every registered work tree, primary included.
    pub worktrees: usize,
    /// Linked work trees at their canonical path.
    pub canonical: usize,
    /// Linked work trees somewhere else.
    pub misplaced: usize,
    /// Linked work trees without a branch.
    pub detached: usize,
    /// Scratch checkouts of a session.
    pub ephemeral: usize,
    /// Registrations whose directory is gone.
    pub missing: usize,
    /// Work trees git reports as locked.
    pub locked: usize,
    /// Work trees with uncommitted work, when it was asked for.
    pub dirty: usize,
    /// Work trees whose `.envrc` direnv refuses, when it was asked for.
    pub envrc_blocked: usize,
    /// Local branches.
    pub branches: usize,
    /// Local branches with no work tree.
    pub branches_without_worktree: usize,
    /// Branches eligible for cleanup.
    pub cleanup_eligible: usize,
    /// Error-level diagnostics.
    pub errors: usize,
    /// Warning-level diagnostics.
    pub warnings: usize,
}

/// The whole topology: what `worktree list`, the MCP resource, the HTTP route and the
/// Cockpit all read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryTopology {
    /// [`SCHEMA`].
    pub schema: String,
    /// The repository.
    pub repository: RepositoryView,
    /// The container every linked work tree belongs under.
    pub container: ContainerView,
    /// The trunk.
    pub trunk: TrunkView,
    /// The work tree the call was answered from.
    pub observed_from: String,
    /// Every registered work tree, primary first.
    pub worktrees: Vec<WorktreeState>,
    /// Every local branch, by name.
    pub branches: Vec<BranchState>,
    /// Everything wrong with the topology, work trees first, then repository-wide.
    pub diagnostics: Vec<TopologyDiagnostic>,
    /// The counts.
    pub tallies: TopologyTallies,
    /// No error-level diagnostic stands.
    pub valid: bool,
}

/// The answer to "where am I, and is that where I belong".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StatusReport {
    /// [`SCHEMA`].
    pub schema: String,
    /// The repository.
    pub repository: RepositoryView,
    /// The container.
    pub container: ContainerView,
    /// The trunk.
    pub trunk: TrunkView,
    /// The work tree the call came from, with its uncommitted work counted.
    pub worktree: WorktreeState,
    /// Whether this work tree is where it belongs: canonical, or the primary checkout on
    /// the trunk, or detached.
    pub canonical: bool,
    /// How many error-level diagnostics the whole topology carries.
    pub repository_errors: usize,
}

/// What the guard decided, for a hook or a mutation that must not proceed out of place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GuardVerdict {
    /// Proceed.
    pub ok: bool,
    /// The work tree judged.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Its branch.
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Where the branch belongs.
    pub expected_path: Option<String>,
    /// Where it stands.
    pub standing: Standing,
    /// Not subject to the rule: detached, or the trunk in the primary checkout.
    pub exempt: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Why not, when not.
    pub reason: Option<TopologyDiagnostic>,
}

/// One branch, inspected: where its work tree belongs and what is there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InspectReport {
    /// The branch asked about.
    pub branch: String,
    /// The branch exists locally.
    pub branch_exists: bool,
    /// Where its work tree belongs.
    pub expected_path: String,
    /// Something exists at that path.
    pub destination_exists: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The work tree that holds the branch, when one does.
    pub worktree: Option<WorktreeState>,
    /// The branch is checked out at its canonical path.
    pub canonical: bool,
    /// What stands in the way, if anything.
    pub diagnostics: Vec<TopologyDiagnostic>,
}

/// What a repair did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepairReport {
    /// Registrations git dropped, or would drop, because their directory is gone.
    pub pruned: Vec<String>,
    /// What `git worktree repair` reported.
    pub repaired: Vec<String>,
    /// Whether anything was changed.
    pub applied: bool,
}
