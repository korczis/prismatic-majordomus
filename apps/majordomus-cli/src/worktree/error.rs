//! What can stop a worktree operation, and what a person is told to do about it.
//!
//! Every variant names the path or branch involved, the invariant that was not met, and
//! the command that repairs it. A caller never renders a debug struct: the `Display` of
//! this type is the message, and the transports add nothing to it.

use std::path::PathBuf;

/// The exit code of a refusal: the operation is well-formed and the policy or the state of
/// the tree says no. The executable's contract already spends `10` on "contract unmet".
pub const EXIT_REFUSED: u8 = 10;
/// The exit code when the thing asked for does not exist.
pub const EXIT_MISSING: u8 = 12;
/// The exit code when git itself failed, or the topology cannot be read at all.
pub const EXIT_INTERNAL: u8 = 13;

/// Everything that stops a worktree operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorktreeError {
    /// The start directory is not inside a git work tree.
    #[error("not a git repository: {start} is not inside a git work tree; worktree management needs one")]
    NotInGitRepository {
        /// Where the search began.
        start: PathBuf,
    },

    /// The repository is bare. Worktree management is defined against a primary checkout.
    #[error(
        "worktree management requires a non-bare primary checkout; {git_dir} is a bare repository, \
         which has no primary work tree to derive a sibling container from"
    )]
    BareRepositoryUnsupported {
        /// The bare repository directory.
        git_dir: PathBuf,
    },

    /// `git worktree list` answered, and nothing in it is the primary worktree.
    #[error(
        "cannot determine the primary worktree of {git_common_dir}: `git worktree list --porcelain` \
         named no non-bare main work tree; the repository metadata is incomplete"
    )]
    CannotDeterminePrimaryWorktree {
        /// The common git directory the topology was read from.
        git_common_dir: PathBuf,
    },

    /// The primary checkout is at the filesystem root, so it has no parent to be a sibling in.
    #[error(
        "cannot derive a sibling worktree container for {primary}: it has no parent directory; \
         the `sibling` strategy places the container beside the primary checkout"
    )]
    NoParentDirectory {
        /// The primary checkout.
        primary: PathBuf,
    },

    /// The policy names a root strategy this executable does not implement.
    #[error(
        "unknown worktree root strategy '{strategy}' in {path}; this executable implements `sibling` \
         (the container is the primary checkout's name plus the suffix, beside it)"
    )]
    UnknownRootStrategy {
        /// What the policy said.
        strategy: String,
        /// The policy file.
        path: String,
    },

    /// The policy's suffix is empty or would not produce a separate directory.
    #[error(
        "the worktree root suffix in {path} is {reason}; with the `sibling` strategy the suffix is \
         what separates the container from the checkout, so it must be a non-empty name fragment"
    )]
    InvalidRootSuffix {
        /// Why it cannot be used.
        reason: String,
        /// The policy file.
        path: String,
    },

    /// A name given on the command line cannot be a single directory under the container.
    #[error(
        "invalid worktree name '{given}': {reason}; a worktree name becomes exactly one directory \
         under the canonical root, so it carries no '/', is not '.' or '..', and is not empty"
    )]
    InvalidWorktreeName {
        /// What was given.
        given: String,
        /// Why it was refused.
        reason: String,
    },

    /// A branch name cannot be used as given.
    #[error("invalid branch name '{given}': {reason}")]
    InvalidBranchName {
        /// What was given.
        given: String,
        /// Why it was refused.
        reason: String,
    },

    /// The destination is already a registered worktree of this repository.
    #[error(
        "a worktree is already registered at {path}{}; nothing was created. \
         Use it (cd \"$(majordomus worktree path {name})\"), or remove it first \
         (majordomus worktree remove {name})",
        branch.as_ref().map(|b| format!(" on branch {b}")).unwrap_or_default()
    )]
    WorktreeAlreadyExists {
        /// The registered path.
        path: PathBuf,
        /// Its directory name, which is also its selector.
        name: String,
        /// The branch it holds, when it holds one.
        branch: Option<String>,
    },

    /// The destination path exists and is not a registered worktree.
    #[error(
        "{path} already exists and is not a registered worktree of this repository; \
         nothing was created and nothing was deleted. Move it aside, or choose another name"
    )]
    TargetCollision {
        /// The occupied path.
        path: PathBuf,
    },

    /// The canonical container path is itself taken by something that is not a directory.
    #[error(
        "the canonical worktree root {path} exists and is not a directory; it is the container every \
         linked worktree of this repository belongs in, so it cannot be a file or a symlink to one"
    )]
    RootNotADirectory {
        /// The container path.
        path: PathBuf,
    },

    /// The canonical container path is occupied by a registered worktree of this repository.
    #[error(
        "the canonical worktree root {path} is itself a registered worktree of this repository{}. \
         The container cannot also be a checkout. Move that worktree elsewhere \
         (git worktree move {path} <new-path>) and run `majordomus worktree migrate --plan`",
        branch.as_ref().map(|b| format!(" on branch {b}")).unwrap_or_default()
    )]
    RootIsAWorktree {
        /// The container path.
        path: PathBuf,
        /// The branch it holds.
        branch: Option<String>,
    },

    /// The branch asked for is checked out in another worktree already.
    #[error(
        "branch '{branch}' is already checked out at {path}; git allows one checkout of a branch at a \
         time. Work there (cd \"$(majordomus worktree path {branch})\"), or create the worktree on \
         another branch (--branch <name>)"
    )]
    BranchAlreadyCheckedOut {
        /// The branch.
        branch: String,
        /// Where it is checked out.
        path: PathBuf,
    },

    /// The base ref asked for does not resolve.
    #[error("base ref '{base}' does not resolve in this repository; nothing was created and nothing was fetched")]
    BaseDoesNotExist {
        /// What was asked for.
        base: String,
    },

    /// A linked worktree sits outside the canonical container.
    #[error(
        "worktree {path} is outside the canonical root {root}; every linked worktree of this \
         repository belongs under it. Plan the move with `majordomus worktree migrate --plan`"
    )]
    OutsideCanonicalRoot {
        /// The offending worktree.
        path: PathBuf,
        /// Where it belongs.
        root: PathBuf,
    },

    /// The worktree has uncommitted or untracked content and the operation would destroy it.
    #[error(
        "worktree {path} is dirty ({summary}); {operation} would lose that work. Commit or stash it \
         there first, or pass --force to say that losing it is intended"
    )]
    DirtyWorktree {
        /// The worktree.
        path: PathBuf,
        /// What is dirty, in words.
        summary: String,
        /// The operation that refused.
        operation: String,
    },

    /// The worktree is locked; git refuses to move or remove it and so does this.
    #[error(
        "worktree {path} is locked{}; unlock it first (git worktree unlock {path})",
        reason.as_ref().filter(|r| !r.is_empty()).map(|r| format!(" ({r})")).unwrap_or_default()
    )]
    LockedWorktree {
        /// The worktree.
        path: PathBuf,
        /// The reason git recorded, when it recorded one.
        reason: Option<String>,
    },

    /// The primary checkout was named to an operation that only linked worktrees accept.
    #[error(
        "{path} is this repository's primary checkout; `worktree {operation}` operates on linked \
         worktrees only and will never move or delete the primary checkout"
    )]
    PrimaryWorktreeProtected {
        /// The primary checkout.
        path: PathBuf,
        /// The operation that refused.
        operation: String,
    },

    /// A path was named that this repository does not own.
    #[error(
        "{path} is not a registered worktree of this repository (whose git directory is \
         {git_common_dir}); a worktree operation only ever touches a path git has registered here"
    )]
    RepositoryIdentityMismatch {
        /// The path named.
        path: PathBuf,
        /// This repository's common git directory.
        git_common_dir: PathBuf,
    },

    /// A selector matched nothing.
    #[error(
        "no worktree of this repository matches '{selector}'; a selector is an exact path, an exact \
         directory name under the canonical root, or an exact branch name. \
         Run `majordomus worktree list` to see them"
    )]
    NoSuchWorktree {
        /// What was asked for.
        selector: String,
    },

    /// A selector matched more than one worktree; a destructive command never guesses.
    #[error(
        "'{selector}' matches {} worktrees ({}); name one exactly — nothing was changed",
        candidates.len(),
        candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
    )]
    AmbiguousSelector {
        /// What was asked for.
        selector: String,
        /// Everything it matched.
        candidates: Vec<PathBuf>,
    },

    /// A migration cannot be carried out safely, with the reason.
    #[error("cannot migrate {path} safely: {reason}; nothing was moved")]
    MigrationUnsafe {
        /// The worktree that would have moved.
        path: PathBuf,
        /// Why it was refused.
        reason: String,
    },

    /// The repository-scoped lock could not be taken.
    #[error(
        "another process is changing this repository's worktrees (lock {path}{}); \
         nothing was changed. Retry when it finishes",
        holder.as_ref().map(|h| format!(", held by {h}")).unwrap_or_default()
    )]
    LockUnavailable {
        /// The lock file.
        path: PathBuf,
        /// What the lock file says holds it, when it says.
        holder: Option<String>,
    },

    /// A git command failed; its own words are carried through.
    #[error("git {command} failed ({status}): {stderr}")]
    GitCommandFailed {
        /// The arguments, joined for reading.
        command: String,
        /// How it exited.
        status: String,
        /// What it said on standard error.
        stderr: String,
    },

    /// `git` could not be run at all.
    #[error("cannot run git: {reason}")]
    GitUnavailable {
        /// Why not.
        reason: String,
    },

    /// A file or directory could not be read or written.
    #[error("{path}: {reason}")]
    Io {
        /// The path.
        path: PathBuf,
        /// What the operating system said.
        reason: String,
    },
}

impl WorktreeError {
    /// The exit code this error leaves the process with.
    ///
    /// ```
    /// use majordomus_cli::worktree::WorktreeError;
    /// use std::path::PathBuf;
    /// let e = WorktreeError::NoSuchWorktree { selector: "nope".into() };
    /// assert_eq!(e.exit_code(), 12);
    /// let e = WorktreeError::DirtyWorktree {
    ///     path: PathBuf::from("/a"), summary: "1 change".into(), operation: "remove".into(),
    /// };
    /// assert_eq!(e.exit_code(), 10);
    /// ```
    pub fn exit_code(&self) -> u8 {
        use WorktreeError::*;
        match self {
            NoSuchWorktree { .. } | BaseDoesNotExist { .. } => EXIT_MISSING,
            GitCommandFailed { .. } | GitUnavailable { .. } | Io { .. } => EXIT_INTERNAL,
            _ => EXIT_REFUSED,
        }
    }

    /// The stable machine name of this refusal: what a script or a test matches on, so that
    /// neither has to match the prose. It is the variant's own name.
    ///
    /// ```
    /// use majordomus_cli::worktree::WorktreeError;
    /// use std::path::PathBuf;
    /// let e = WorktreeError::OutsideCanonicalRoot {
    ///     path: PathBuf::from("/tmp/x"), root: PathBuf::from("/a/foo-wt"),
    /// };
    /// assert_eq!(e.code(), "OutsideCanonicalRoot");
    /// ```
    pub fn code(&self) -> &'static str {
        use WorktreeError::*;
        match self {
            NotInGitRepository { .. } => "NotInGitRepository",
            BareRepositoryUnsupported { .. } => "BareRepositoryUnsupported",
            CannotDeterminePrimaryWorktree { .. } => "CannotDeterminePrimaryWorktree",
            NoParentDirectory { .. } => "NoParentDirectory",
            UnknownRootStrategy { .. } => "UnknownRootStrategy",
            InvalidRootSuffix { .. } => "InvalidRootSuffix",
            InvalidWorktreeName { .. } => "InvalidWorktreeName",
            InvalidBranchName { .. } => "InvalidBranchName",
            WorktreeAlreadyExists { .. } => "WorktreeAlreadyExists",
            TargetCollision { .. } => "TargetCollision",
            RootNotADirectory { .. } => "RootNotADirectory",
            RootIsAWorktree { .. } => "RootIsAWorktree",
            BranchAlreadyCheckedOut { .. } => "BranchAlreadyCheckedOut",
            BaseDoesNotExist { .. } => "BaseDoesNotExist",
            OutsideCanonicalRoot { .. } => "OutsideCanonicalRoot",
            DirtyWorktree { .. } => "DirtyWorktree",
            LockedWorktree { .. } => "LockedWorktree",
            PrimaryWorktreeProtected { .. } => "PrimaryWorktreeProtected",
            RepositoryIdentityMismatch { .. } => "RepositoryIdentityMismatch",
            NoSuchWorktree { .. } => "NoSuchWorktree",
            AmbiguousSelector { .. } => "AmbiguousSelector",
            MigrationUnsafe { .. } => "MigrationUnsafe",
            LockUnavailable { .. } => "LockUnavailable",
            GitCommandFailed { .. } => "GitCommandFailed",
            GitUnavailable { .. } => "GitUnavailable",
            Io { .. } => "Io",
        }
    }

    /// An IO failure against a path.
    pub fn io(path: impl Into<PathBuf>, e: &std::io::Error) -> Self {
        WorktreeError::Io {
            path: path.into(),
            reason: e.to_string(),
        }
    }
}

/// The result of every worktree operation.
pub type Result<T> = std::result::Result<T, WorktreeError>;
