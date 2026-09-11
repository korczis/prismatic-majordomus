//! What can stop a worktree operation, and what a person is told to do about it.
//!
//! Every variant names the path or branch involved, the invariant that was not met, and
//! the command that repairs it. A caller never renders a debug struct: the `Display` of
//! this type is the message, and the transports add nothing to it.

use std::path::PathBuf;

/// The exit code of a refusal: the operation is well-formed and the state of the tree says
/// no. The executable's contract already spends `10` on "contract unmet".
pub const EXIT_REFUSED: u8 = 10;
/// The exit code when the thing asked for does not exist.
pub const EXIT_MISSING: u8 = 12;
/// The exit code when git itself failed, or the topology cannot be read at all.
pub const EXIT_INTERNAL: u8 = 13;

/// Everything that stops a worktree operation.
///
/// The variants divide into three outcomes and not one: a refusal, where the request was
/// well formed and the state of the tree says no; a miss, where what was named does not
/// exist; and an internal failure, where git or the filesystem did not answer. Which of
/// the three a variant is comes from [`WorktreeError::exit_code`] and is not repeated
/// anywhere, so a caller never has to classify a refusal by reading its prose.
///
/// Each variant carries the paths and branches involved, because a message that says a
/// move was refused without saying which two directories were involved sends the reader
/// back to `git worktree list`.
///
/// ```
/// use majordomus_cli::worktree::WorktreeError;
/// use std::path::PathBuf;
///
/// let e = WorktreeError::Misplaced {
///     path: PathBuf::from("/tmp/stray"),
///     branch: "feature/x".into(),
///     expected: PathBuf::from("/a/foo-wt/feature/x"),
/// };
/// assert_eq!(e.code(), "Misplaced", "the machine name is the variant's own name");
/// assert_eq!(e.exit_code(), 10, "a misplaced worktree is a refusal, not a miss");
/// let shown = e.to_string();
/// assert!(shown.contains("/tmp/stray") && shown.contains("/a/foo-wt/feature/x"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorktreeError {
    /// The start directory is not inside a git work tree.
    #[error("not a git repository: {start} is not inside a git work tree; the worktree topology needs one")]
    NotInGitRepository {
        /// Where the search began.
        start: PathBuf,
    },

    /// The repository is bare. The topology is defined against a primary checkout.
    #[error(
        "the worktree topology requires a non-bare primary checkout; {git_dir} is a bare repository, \
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
        "cannot derive a sibling worktree container for {primary}: it has no parent directory"
    )]
    NoParentDirectory {
        /// The primary checkout.
        primary: PathBuf,
    },

    /// A branch name cannot be used as given.
    #[error("invalid branch name '{given}': {reason}")]
    InvalidBranchName {
        /// What was given.
        given: String,
        /// Why it was refused.
        reason: String,
    },

    /// The derived worktree path does not stay inside the container. Cannot happen for a
    /// valid branch name; refused by name rather than assumed away.
    #[error(
        "the worktree path derived for branch '{branch}' ({path}) is not inside the container {root}; \
         nothing was created"
    )]
    PathEscape {
        /// The branch.
        branch: String,
        /// What was derived.
        path: PathBuf,
        /// Where it had to be.
        root: PathBuf,
    },

    /// The container path exists and is not a directory.
    #[error(
        "the worktree container {path} exists and is not a directory; it is where every linked \
         worktree of this repository belongs, so it cannot be a file or a symlink to one"
    )]
    ContainerNotADirectory {
        /// The container path.
        path: PathBuf,
    },

    /// The worktree asked for is already registered at its canonical path.
    #[error(
        "branch '{branch}' already has its canonical worktree at {path}; nothing was created. \
         Work there: cd \"$(majordomus worktree path {branch})\""
    )]
    WorktreeAlreadyExists {
        /// The registered path.
        path: PathBuf,
        /// The branch it holds.
        branch: String,
    },

    /// The branch is checked out in a worktree that is not its canonical one.
    #[error(
        "branch '{branch}' is checked out at {path}, which is not its canonical worktree \
         {expected}; git allows one checkout of a branch at a time. Bring it home with \
         `majordomus worktree migrate` and work there"
    )]
    BranchAlreadyCheckedOut {
        /// The branch.
        branch: String,
        /// Where it is checked out.
        path: PathBuf,
        /// Where it belongs.
        expected: PathBuf,
    },

    /// The destination exists and is something else.
    #[error(
        "{path} already exists and is {what}; nothing was created, nothing was moved and nothing \
         was deleted. Move it aside, or choose another branch name"
    )]
    DestinationConflict {
        /// The occupied path.
        path: PathBuf,
        /// What occupies it, in words.
        what: String,
    },

    /// The base ref asked for does not resolve.
    #[error("base ref '{base}' does not resolve in this repository; nothing was created and nothing was fetched")]
    BaseDoesNotExist {
        /// What was asked for.
        base: String,
    },

    /// The current worktree is not where its branch belongs.
    #[error(
        "worktree {path} holds branch '{branch}' but that branch belongs at {expected}; \
         feature work happens in the canonical worktree. Run `majordomus worktree migrate --plan` \
         to see the move, `majordomus worktree migrate` to make it, then continue there"
    )]
    Misplaced {
        /// The offending worktree.
        path: PathBuf,
        /// Its branch.
        branch: String,
        /// Where the branch belongs.
        expected: PathBuf,
    },

    /// The primary checkout holds a branch that is not the trunk.
    #[error(
        "the primary checkout {path} is on branch '{branch}', and the primary checkout hosts the trunk \
         ('{trunk}'); that branch belongs at {expected}. Nothing was changed: switch the primary \
         checkout back to the trunk when it is clean, and work on '{branch}' in its canonical worktree \
         (`majordomus worktree create {branch}`)"
    )]
    PrimaryOnNonTrunk {
        /// The primary checkout.
        path: PathBuf,
        /// The branch it holds.
        branch: String,
        /// The trunk.
        trunk: String,
        /// Where the branch belongs.
        expected: PathBuf,
    },

    /// The worktree has uncommitted or untracked content and the operation would destroy it.
    #[error(
        "worktree {path} is dirty ({summary}); {operation} would lose that work. Commit it there \
         first, or pass --force to say that losing it is intended"
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
         worktrees only and never moves or deletes the primary checkout"
    )]
    PrimaryWorktreeProtected {
        /// The primary checkout.
        path: PathBuf,
        /// The operation that refused.
        operation: String,
    },

    /// A selector matched nothing.
    #[error(
        "no worktree of this repository matches '{selector}'; a selector is an exact branch name \
         or an exact path. `majordomus worktree list` shows them"
    )]
    NoSuchWorktree {
        /// What was asked for.
        selector: String,
    },

    /// A path outside every worktree of this repository was named.
    #[error(
        "{path} is not inside a worktree of this repository (git directory {git_common_dir}); \
         the topology answers only about its own repository"
    )]
    NotThisRepository {
        /// The path named.
        path: PathBuf,
        /// This repository's common git directory.
        git_common_dir: PathBuf,
    },

    /// A migration cannot be carried out safely, with the reason.
    #[error("cannot migrate {path} safely: {reason}; nothing was moved")]
    MigrationUnsafe {
        /// The worktree that would have moved.
        path: PathBuf,
        /// Why it was refused.
        reason: String,
    },

    /// The fingerprint after a move differs from the one before it.
    #[error(
        "migration of {path} could not be verified: {}. The worktree is registered at {path}; \
         inspect it before touching anything else (majordomus worktree status --repo {path})",
        differences.join("; ")
    )]
    VerificationFailed {
        /// Where the worktree is now.
        path: PathBuf,
        /// What differs, one line each.
        differences: Vec<String>,
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

    /// The topology is invalid: an error-level diagnostic stands.
    #[error("the worktree topology has {count} error(s); `majordomus worktree doctor` lists them")]
    TopologyInvalid {
        /// How many error-level diagnostics.
        count: usize,
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
    /// let e = WorktreeError::Misplaced {
    ///     path: PathBuf::from("/tmp/x"), branch: "feature/x".into(), expected: PathBuf::from("/a/foo-wt/feature/x"),
    /// };
    /// assert_eq!(e.code(), "Misplaced");
    /// ```
    pub fn code(&self) -> &'static str {
        use WorktreeError::*;
        match self {
            NotInGitRepository { .. } => "NotInGitRepository",
            BareRepositoryUnsupported { .. } => "BareRepositoryUnsupported",
            CannotDeterminePrimaryWorktree { .. } => "CannotDeterminePrimaryWorktree",
            NoParentDirectory { .. } => "NoParentDirectory",
            InvalidBranchName { .. } => "InvalidBranchName",
            PathEscape { .. } => "PathEscape",
            ContainerNotADirectory { .. } => "ContainerNotADirectory",
            WorktreeAlreadyExists { .. } => "WorktreeAlreadyExists",
            BranchAlreadyCheckedOut { .. } => "BranchAlreadyCheckedOut",
            DestinationConflict { .. } => "DestinationConflict",
            BaseDoesNotExist { .. } => "BaseDoesNotExist",
            Misplaced { .. } => "Misplaced",
            PrimaryOnNonTrunk { .. } => "PrimaryOnNonTrunk",
            DirtyWorktree { .. } => "DirtyWorktree",
            LockedWorktree { .. } => "LockedWorktree",
            PrimaryWorktreeProtected { .. } => "PrimaryWorktreeProtected",
            NoSuchWorktree { .. } => "NoSuchWorktree",
            NotThisRepository { .. } => "NotThisRepository",
            MigrationUnsafe { .. } => "MigrationUnsafe",
            VerificationFailed { .. } => "VerificationFailed",
            LockUnavailable { .. } => "LockUnavailable",
            TopologyInvalid { .. } => "TopologyInvalid",
            GitCommandFailed { .. } => "GitCommandFailed",
            GitUnavailable { .. } => "GitUnavailable",
            Io { .. } => "Io",
        }
    }

    /// Wrap a filesystem failure so that the path it happened to is part of the message.
    ///
    /// `std::io::Error` knows what went wrong and not what it went wrong to, and a bare
    /// "permission denied" from inside a topology operation is unactionable. This is the
    /// only way an IO failure enters this type, so every one of them names its path.
    ///
    /// ```
    /// use majordomus_cli::worktree::WorktreeError;
    /// let os = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    /// let e = WorktreeError::io("/a/foo-wt", &os);
    /// assert_eq!(e.code(), "Io");
    /// assert_eq!(e.exit_code(), 13, "the filesystem failing is internal, not a refusal");
    /// assert!(e.to_string().starts_with("/a/foo-wt: "), "the path leads the message");
    /// ```
    pub fn io(path: impl Into<PathBuf>, e: &std::io::Error) -> Self {
        WorktreeError::Io {
            path: path.into(),
            reason: e.to_string(),
        }
    }
}

/// The result of every worktree operation.
pub type Result<T> = std::result::Result<T, WorktreeError>;
