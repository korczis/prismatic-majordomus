//! The typed topology: one Rust source of truth that serde, JSON Schema, OpenAPI, MCP, the
//! command line and the Cockpit all render. Nothing in here is decided; it is what the
//! service decided, written down.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The `schema` every topology document carries.
pub const SCHEMA: &str = "majordomus/worktree-topology/v1";

/// Whether a work tree is the repository's own checkout or one linked to it.
///
/// The distinction is not cosmetic: the two kinds are held to different rules. A linked
/// work tree belongs at its branch's canonical path and can be moved or removed; the
/// primary checkout is exempt from the path rule, hosts the trunk, and is never either.
/// Keeping that as a kind rather than as a path comparison scattered through the callers
/// is what lets the guard state its rule once.
///
/// ```
/// use majordomus_cli::worktree::WorktreeKind;
/// let wire = serde_json::to_string(&WorktreeKind::Linked).unwrap();
/// assert_eq!(wire, "\"linked\"", "the wire form is the lowercase word");
/// assert_eq!(WorktreeKind::Primary.as_str(), "primary");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WorktreeKind {
    /// The main work tree. It hosts the trunk, is never moved and is never removed.
    Primary,
    /// A linked work tree. Belongs at its branch's canonical path.
    Linked,
}

impl WorktreeKind {
    /// The word this kind is reported under, on every surface.
    ///
    /// The same string serde writes, deliberately: a human rendering that spelled a kind
    /// differently from the JSON would make the command line and the API two vocabularies
    /// for one fact, and a reader comparing them would conclude they disagree.
    ///
    /// ```
    /// use majordomus_cli::worktree::WorktreeKind;
    /// for kind in [WorktreeKind::Primary, WorktreeKind::Linked] {
    ///     let wire = serde_json::to_string(&kind).unwrap();
    ///     assert_eq!(wire, format!("\"{}\"", kind.as_str()), "one vocabulary, not two");
    /// }
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            WorktreeKind::Primary => "primary",
            WorktreeKind::Linked => "linked",
        }
    }
}

/// Where a work tree stands against the topology.
///
/// Six standings and not a boolean, because "not where it belongs" covers four situations
/// with four different answers: a linked tree in the wrong place is migrated, a detached
/// one has no canonical path to be moved to, an ephemeral one is a harness's scratch
/// checkout that will remove itself, and a missing one is a registration to prune rather
/// than a directory to move. [`Standing::is_acceptable`] is where those four collapse into
/// the one bit a caller usually wants, and it collapses them in exactly one place.
///
/// ```
/// use majordomus_cli::worktree::Standing;
/// assert!(Standing::Canonical.is_acceptable());
/// assert!(Standing::Primary.is_acceptable(), "the primary checkout is exempt");
/// assert!(!Standing::Misplaced.is_acceptable());
/// assert_eq!(serde_json::to_string(&Standing::Missing).unwrap(), "\"missing\"");
/// ```
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
    /// The word this standing is reported under, on every surface.
    ///
    /// Identical to what serde writes, so the table a person reads and the JSON a script
    /// parses use one set of words. That parity is asserted rather than assumed, because
    /// the two are written out separately and a new variant can be added to one of them.
    ///
    /// ```
    /// use majordomus_cli::worktree::Standing;
    /// let all = [
    ///     Standing::Primary,
    ///     Standing::Canonical,
    ///     Standing::Misplaced,
    ///     Standing::Detached,
    ///     Standing::Ephemeral,
    ///     Standing::Missing,
    /// ];
    /// for standing in all {
    ///     let wire = serde_json::to_string(&standing).unwrap();
    ///     assert_eq!(wire, format!("\"{}\"", standing.as_str()));
    /// }
    /// ```
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
    ///
    /// ```
    /// use majordomus_cli::worktree::Standing;
    /// let all = [
    ///     Standing::Primary,
    ///     Standing::Canonical,
    ///     Standing::Misplaced,
    ///     Standing::Detached,
    ///     Standing::Ephemeral,
    ///     Standing::Missing,
    /// ];
    /// let accepted: Vec<_> = all
    ///     .into_iter()
    ///     .filter(|s| s.is_acceptable())
    ///     .map(|s| s.as_str())
    ///     .collect();
    /// assert_eq!(accepted, ["primary", "canonical", "detached"]);
    /// ```
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
///
/// The codes are namespaced under `worktree.` and are the part of the diagnostic that is
/// promised to stay put. A script gates on the code, a test asserts the code, and the
/// message beside it is free to be rewritten for a reader without breaking either — which
/// is the whole reason the two are separate fields of [`TopologyDiagnostic`].
///
/// ```
/// use majordomus_cli::worktree::DiagnosticCode;
/// // the human string and the serialised string are the same string, for every code
/// for code in DiagnosticCode::ALL {
///     let wire = serde_json::to_string(code).unwrap();
///     assert_eq!(wire, format!("\"{}\"", code.as_str()));
///     assert!(code.as_str().starts_with("worktree."), "namespaced: {}", code.as_str());
/// }
/// // and no condition shares a code with another
/// let mut names: Vec<_> = DiagnosticCode::ALL.iter().map(|c| c.as_str()).collect();
/// let total = names.len();
/// names.sort();
/// names.dedup();
/// assert_eq!(names.len(), total, "one code per condition, never reused");
/// ```
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
}

impl DiagnosticCode {
    /// The dotted code, exactly as serde writes it.
    ///
    /// Written out by hand beside the `serde(rename)` attributes rather than derived from
    /// them, because a `&'static str` is what a message, a table and a test all want and
    /// serialising an enum to get one would be absurd. The cost is that the two lists can
    /// drift, which is why the parity is asserted over [`DiagnosticCode::ALL`] instead of
    /// being trusted.
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
    ];
}

/// One thing wrong with, or worth knowing about, the topology.
///
/// The shape is a contract with the reader: a stable code to gate on, a severity that says
/// whether the topology is invalid while this stands, the paths and branch involved, one
/// line of prose, and the command that fixes it. `remedy` is not optional, and that is the
/// point — a diagnostic nobody can act on is a complaint, and this subsystem raises none.
///
/// ```
/// use majordomus_cli::worktree::{DiagnosticCode, Severity, TopologyDiagnostic};
/// let d = TopologyDiagnostic {
///     code: DiagnosticCode::PathMismatch,
///     severity: Severity::Error,
///     path: Some("/tmp/stray".into()),
///     branch: Some("feature/x".into()),
///     expected: Some("/a/foo-wt/feature/x".into()),
///     message: "this work tree is not at its branch's canonical path".into(),
///     remedy: "majordomus worktree migrate".into(),
/// };
/// assert!(!d.remedy.is_empty(), "every diagnostic names what to do about it");
/// let wire = serde_json::to_value(&d).unwrap();
/// assert_eq!(wire["code"], "worktree.path_mismatch");
/// assert_eq!(wire["expected"], "/a/foo-wt/feature/x");
/// ```
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
///
/// Four counts and not one, because the four are lost in different ways and a person
/// deciding whether to allow a move needs to know which they are risking. `clean` is
/// derived from all four and stored, so every surface agrees on the word; `in_progress`
/// makes a work tree unclean even when the counts are zero, because a half-finished rebase
/// is state no move should carry.
///
/// ```
/// use majordomus_cli::worktree::DirtyState;
/// let clean = DirtyState { clean: true, ..Default::default() };
/// assert_eq!(clean.summary(), "clean");
///
/// let busy = DirtyState {
///     staged: 1,
///     untracked: 2,
///     in_progress: Some("rebase".into()),
///     ..Default::default()
/// };
/// assert_eq!(busy.summary(), "1 staged, 2 untracked, rebase in progress");
/// ```
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
    /// Every count, in words, for a message: `"2 staged, 1 untracked"`, or `"clean"`.
    ///
    /// Only the non-zero counts appear, so the line is about what is there rather than
    /// about what is not, and the operation in progress comes last because it is the part
    /// that stops a move outright. `"clean"` is a word and not an empty string: a refusal
    /// that quoted an empty summary would read as a refusal with no reason.
    ///
    /// ```
    /// use majordomus_cli::worktree::DirtyState;
    /// let d = DirtyState { unstaged: 3, ..Default::default() };
    /// assert_eq!(d.summary(), "3 unstaged", "the zeroes are left out");
    /// let mid = DirtyState {
    ///     clean: true,
    ///     in_progress: Some("merge".into()),
    ///     ..Default::default()
    /// };
    /// assert_eq!(mid.summary(), "clean", "`clean` is the whole line when it is true");
    /// ```
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
///
/// Because nothing fetches, the distances are against the remote-tracking refs this
/// checkout already has, and they are as old as the last fetch. `ahead` and `behind` are
/// optional so that "the upstream ref is gone, there is no distance to report" is a
/// different answer from "the two are level" — the two mean opposite things to anyone
/// deciding whether a branch can be cleaned up.
///
/// ```
/// use majordomus_cli::worktree::UpstreamState;
/// let vanished = UpstreamState {
///     name: "origin/feature/x".into(),
///     ahead: None,
///     behind: None,
///     gone: true,
/// };
/// let wire = serde_json::to_value(&vanished).unwrap();
/// assert_eq!(wire["gone"], true);
/// assert!(wire.get("ahead").is_none(), "a vanished upstream has no distance, not zero");
/// ```
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

/// One work tree, as the topology sees it.
///
/// Everything git said about the work tree, plus everything this subsystem derived from
/// it, in one value that serde, JSON Schema, the command line and the Cockpit all render.
/// The optional fields divide into two groups worth keeping apart: those absent because
/// the work tree has no such thing (`branch` when detached) and those absent because
/// nobody paid for them (`dirty`, which costs a subprocess per work tree and is skipped by
/// a topology read). Both serialise as a missing key, so a consumer must not read a
/// missing `dirty` as a clean tree.
///
/// ```
/// use majordomus_cli::worktree::{Standing, WorktreeState};
/// let s: WorktreeState = serde_json::from_value(serde_json::json!({
///     "path": "/a/foo-wt/feature/x",
///     "kind": "linked",
///     "standing": "canonical",
///     "label": "feature/x",
///     "detached": false,
///     "exists": true,
///     "current": false,
///     "diagnostics": []
/// }))
/// .unwrap();
/// assert_eq!(s.standing, Standing::Canonical);
/// assert!(s.diagnostics.is_empty(), "a canonical work tree has nothing wrong with it");
/// assert!(s.dirty.is_none(), "uncommitted work is absent until something asks for it");
/// ```
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
    /// What is wrong with this work tree, if anything.
    pub diagnostics: Vec<TopologyDiagnostic>,
}

/// One local branch, with or without a work tree.
///
/// The branch half of the topology: every local branch is listed, whether or not a work
/// tree holds it, because "this branch has nowhere to be worked on" is one of the facts
/// the topology exists to report. `cleanup_eligible` is derived here and acted on nowhere
/// — nothing in this subsystem deletes a branch — and `merged_into_trunk` is optional so
/// that an unknown trunk cannot be mistaken for a branch that is not merged.
///
/// ```
/// use majordomus_cli::worktree::BranchState;
/// let b: BranchState = serde_json::from_value(serde_json::json!({
///     "name": "feature/x",
///     "head": "abc123",
///     "trunk": false,
///     "cleanup_eligible": false
/// }))
/// .unwrap();
/// assert_eq!(b.merged_into_trunk, None, "absent means unknown, not `not merged`");
/// assert!(b.worktree.is_none(), "a branch with nowhere to be worked on is still listed");
/// ```
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
///
/// The common git directory is the identity: two paths belong to one repository exactly
/// when it matches, which is what makes "is this worktree mine?" answerable without
/// consulting a remote. The name is the primary checkout's directory name and nothing
/// else — not the remote's, not anything configured — so two clones of one upstream into
/// differently named directories are two repositories with two containers.
///
/// ```
/// use majordomus_cli::worktree::RepositoryView;
/// let r = RepositoryView {
///     primary_worktree: "/src/acme/backend".into(),
///     git_common_dir: "/src/acme/backend/.git".into(),
///     name: "backend".into(),
/// };
/// assert!(r.primary_worktree.ends_with(&r.name), "the name is the directory's own");
/// let wire = serde_json::to_value(&r).unwrap();
/// assert_eq!(wire["git_common_dir"], "/src/acme/backend/.git");
/// ```
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
///
/// The suffix travels with the path so that a reader can see the derivation rather than be
/// told the result: the container is the primary checkout's name with that suffix appended,
/// and nothing configures it. `exists` is false on a repository that has no linked work
/// trees yet, which is a normal state and not a fault — the directory is created by the
/// first worktree that needs it.
///
/// ```
/// use majordomus_cli::worktree::{ContainerView, CONTAINER_SUFFIX};
/// let c = ContainerView {
///     path: "/a/foo-wt".into(),
///     suffix: CONTAINER_SUFFIX.into(),
///     exists: false,
/// };
/// assert!(c.path.ends_with(&c.suffix), "the derivation is visible in the value");
/// assert!(!c.exists, "created by the first worktree that needs it, not before");
/// ```
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
///
/// Three facts, and the second is the one that makes the first arguable: the branch, how it
/// was decided, and where it is checked out. A trunk that could not be determined is
/// reported as an absent branch with an `unknown` source rather than as a conventional
/// name, because inventing one would make the guard refuse work on a branch it merely
/// guessed was not the trunk.
///
/// ```
/// use majordomus_cli::worktree::{TrunkSource, TrunkView};
/// let unknown = TrunkView { branch: None, source: TrunkSource::Unknown, checked_out_at: None };
/// let wire = serde_json::to_value(&unknown).unwrap();
/// assert_eq!(wire["source"], "unknown");
/// assert!(wire.get("branch").is_none(), "no trunk is absent, never an invented name");
/// ```
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
///
/// A summary a person reads before the detail, and the numbers a gate compares. Every
/// count is always serialised, zero included: a skipped field would read as "not measured"
/// where a zero means "none", and those are different claims about a repository. The
/// standings are counted from the same [`Standing`] the work trees carry, so a tally can
/// never name a state the model does not have.
///
/// ```
/// use majordomus_cli::worktree::TopologyTallies;
/// let t = TopologyTallies {
///     worktrees: 5,
///     canonical: 2,
///     misplaced: 1,
///     detached: 1,
///     errors: 1,
///     ..Default::default()
/// };
/// // the primary checkout is one of the work trees and has no standing of its own here
/// assert_eq!(t.canonical + t.misplaced + t.detached, t.worktrees - 1);
/// let wire = serde_json::to_value(&t).unwrap();
/// assert_eq!(wire["missing"], 0, "a zero is reported, not skipped");
/// ```
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
///
/// One value, four surfaces, no second opinion. Nothing here is decided — the service
/// decided it and this is where it was written down — so a surface that rendered a
/// standing differently, or recomputed a tally, would be adding an opinion the model does
/// not have. `valid` is the whole document reduced to the one bit a gate needs, and it is
/// stored rather than derived at render time so that every surface answers the same.
///
/// ```
/// use majordomus_cli::worktree::{
///     ContainerView, RepositoryTopology, RepositoryView, TopologyTallies, TrunkSource,
///     TrunkView, SCHEMA,
/// };
/// let t = RepositoryTopology {
///     schema: SCHEMA.to_string(),
///     repository: RepositoryView {
///         primary_worktree: "/a/foo".into(),
///         git_common_dir: "/a/foo/.git".into(),
///         name: "foo".into(),
///     },
///     container: ContainerView {
///         path: "/a/foo-wt".into(),
///         suffix: "-wt".into(),
///         exists: true,
///     },
///     trunk: TrunkView {
///         branch: Some("master".into()),
///         source: TrunkSource::RemoteHead,
///         checked_out_at: Some("/a/foo".into()),
///     },
///     observed_from: "/a/foo-wt/feature/x".into(),
///     worktrees: Vec::new(),
///     branches: Vec::new(),
///     diagnostics: Vec::new(),
///     tallies: TopologyTallies::default(),
///     valid: true,
/// };
///
/// let wire = serde_json::to_value(&t).unwrap();
/// assert_eq!(wire["schema"], SCHEMA, "every document says which contract it is");
/// assert_eq!(wire["container"]["path"], "/a/foo-wt");
/// // and it comes back unchanged, which is what makes the four surfaces one answer
/// assert_eq!(serde_json::from_value::<RepositoryTopology>(wire).unwrap(), t);
/// ```
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
///
/// One work tree in detail and the repository in one number. That asymmetry is the design:
/// a person standing in a worktree wants to know about this worktree, and a repository
/// error count beside it so that a clean answer here cannot be mistaken for a clean
/// repository. Unlike a topology read, the uncommitted work of this one tree is counted,
/// because there is exactly one subprocess to spend on it.
///
/// ```
/// use majordomus_cli::worktree::{StatusReport, SCHEMA};
/// let r: StatusReport = serde_json::from_value(serde_json::json!({
///     "schema": SCHEMA,
///     "repository": {
///         "primary_worktree": "/a/foo",
///         "git_common_dir": "/a/foo/.git",
///         "name": "foo"
///     },
///     "container": { "path": "/a/foo-wt", "suffix": "-wt", "exists": true },
///     "trunk": { "branch": "master", "source": "remote_head" },
///     "worktree": {
///         "path": "/a/foo-wt/feature/x",
///         "kind": "linked",
///         "standing": "canonical",
///         "label": "feature/x",
///         "detached": false,
///         "exists": true,
///         "current": true,
///         "diagnostics": []
///     },
///     "canonical": true,
///     "repository_errors": 2
/// }))
/// .unwrap();
/// assert!(r.canonical, "this work tree is where its branch belongs");
/// assert!(r.worktree.current, "and it is the one the call came from");
/// assert_eq!(r.repository_errors, 2, "while the repository still has two problems");
/// ```
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
///
/// `ok` is the answer and everything beside it is the argument for it, which is what a
/// hook needs: refusing a commit without saying which work tree, which branch, where it
/// belongs and what to run instead leaves a person with a blocked commit and no move to
/// make. `exempt` says the rule did not apply rather than that it was met — a detached
/// work tree and a canonical one both pass, for different reasons.
///
/// ```
/// use majordomus_cli::worktree::{GuardVerdict, Standing};
/// let verdict = GuardVerdict {
///     ok: true,
///     path: "/a/foo".into(),
///     branch: Some("master".into()),
///     expected_path: None,
///     standing: Standing::Primary,
///     exempt: true,
///     reason: None,
/// };
/// assert!(verdict.ok && verdict.exempt, "the trunk in the primary checkout is exempt");
/// let wire = serde_json::to_value(&verdict).unwrap();
/// assert!(wire.get("reason").is_none(), "a verdict that passes carries no diagnostic");
/// ```
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
///
/// The question asked before creating a worktree, and it is answered for a branch that
/// does not exist yet as readily as for one that does: `expected_path` is derived from the
/// name alone, so a caller learns where the worktree would go and what already stands
/// there in one call. An empty `diagnostics` with `canonical` false is therefore a normal
/// answer and not a contradiction — nothing is wrong, and nothing is there.
///
/// ```
/// use majordomus_cli::worktree::InspectReport;
/// let free = InspectReport {
///     branch: "feature/x".into(),
///     branch_exists: true,
///     expected_path: "/a/foo-wt/feature/x".into(),
///     destination_exists: false,
///     worktree: None,
///     canonical: false,
///     diagnostics: Vec::new(),
/// };
/// assert!(free.diagnostics.is_empty(), "nothing stands in the way");
/// assert!(!free.canonical && free.worktree.is_none(), "and nothing is there yet");
/// assert_eq!(free.expected_path, "/a/foo-wt/feature/x");
/// ```
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

/// What a repair did, or — when `applied` is false — what it would have done.
///
/// One type for the dry run and the real one, so the two cannot disagree about what the
/// repair consists of: the only difference between them is the flag. `pruned` is the
/// registrations dropped because their directory is gone, and `repaired` is what `git
/// worktree repair` said about the ones whose metadata it re-pointed. Neither ever
/// contains a directory that was deleted — a repair fixes registrations, not checkouts.
///
/// ```
/// use majordomus_cli::worktree::RepairReport;
/// let planned = RepairReport {
///     pruned: vec!["/a/foo-wt/feature/gone".into()],
///     repaired: Vec::new(),
///     applied: false,
/// };
/// assert!(!planned.applied, "named what it would do, and did not do it");
/// assert_eq!(planned.pruned.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepairReport {
    /// Registrations git dropped, or would drop, because their directory is gone.
    pub pruned: Vec<String>,
    /// What `git worktree repair` reported.
    pub repaired: Vec<String>,
    /// Whether anything was changed.
    pub applied: bool,
}
