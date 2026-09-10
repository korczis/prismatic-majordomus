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

#[cfg(test)]
mod tests {
    use super::*;

    /// One value of every variant, so that a variant added without a `code()` arm, an
    /// `exit_code()` classification or a message cannot slip past these tests.
    fn one_of_each() -> Vec<WorktreeError> {
        use WorktreeError::*;
        vec![
            NotInGitRepository {
                start: PathBuf::from("/elsewhere"),
            },
            BareRepositoryUnsupported {
                git_dir: PathBuf::from("/a/foo.git"),
            },
            CannotDeterminePrimaryWorktree {
                git_common_dir: PathBuf::from("/a/foo/.git"),
            },
            NoParentDirectory {
                primary: PathBuf::from("/"),
            },
            InvalidBranchName {
                given: "a b".into(),
                reason: "it contains ` `".into(),
            },
            PathEscape {
                branch: "x".into(),
                path: PathBuf::from("/etc"),
                root: PathBuf::from("/a/foo-wt"),
            },
            ContainerNotADirectory {
                path: PathBuf::from("/a/foo-wt"),
            },
            WorktreeAlreadyExists {
                path: PathBuf::from("/a/foo-wt/feature/x"),
                branch: "feature/x".into(),
            },
            BranchAlreadyCheckedOut {
                branch: "feature/x".into(),
                path: PathBuf::from("/tmp/scratch"),
                expected: PathBuf::from("/a/foo-wt/feature/x"),
            },
            DestinationConflict {
                path: PathBuf::from("/a/foo-wt/feature/x"),
                what: "a file".into(),
            },
            BaseDoesNotExist {
                base: "origin/nope".into(),
            },
            Misplaced {
                path: PathBuf::from("/tmp/scratch"),
                branch: "feature/x".into(),
                expected: PathBuf::from("/a/foo-wt/feature/x"),
            },
            PrimaryOnNonTrunk {
                path: PathBuf::from("/a/foo"),
                branch: "feature/x".into(),
                trunk: "master".into(),
                expected: PathBuf::from("/a/foo-wt/feature/x"),
            },
            DirtyWorktree {
                path: PathBuf::from("/a/foo-wt/feature/x"),
                summary: "2 unstaged, 1 untracked".into(),
                operation: "remove".into(),
            },
            LockedWorktree {
                path: PathBuf::from("/a/foo-wt/feature/x"),
                reason: Some("in use".into()),
            },
            PrimaryWorktreeProtected {
                path: PathBuf::from("/a/foo"),
                operation: "remove".into(),
            },
            NoSuchWorktree {
                selector: "nope".into(),
            },
            NotThisRepository {
                path: PathBuf::from("/other/repo"),
                git_common_dir: PathBuf::from("/a/foo/.git"),
            },
            MigrationUnsafe {
                path: PathBuf::from("/tmp/scratch"),
                reason: "the step was not planned as a move".into(),
            },
            VerificationFailed {
                path: PathBuf::from("/a/foo-wt/feature/x"),
                differences: vec!["the index differs".into(), "HEAD changed".into()],
            },
            LockUnavailable {
                path: PathBuf::from("/a/foo/.git/majordomus-worktree.lock"),
                holder: Some("pid 42".into()),
            },
            TopologyInvalid { count: 3 },
            GitCommandFailed {
                command: "worktree move -- /a /b".into(),
                status: "128".into(),
                stderr: "fatal: cannot move".into(),
            },
            GitUnavailable {
                reason: "no such file or directory".into(),
            },
            Io {
                path: PathBuf::from("/a/foo"),
                reason: "permission denied".into(),
            },
        ]
    }

    /// `code()` is what a script and a test match on instead of the prose, and the contract
    /// is that it is the variant's own name. `Debug` is derived, so it carries that name
    /// independently of the hand-written `code()` arms: renaming a variant and forgetting
    /// the arm, or copying an arm and leaving the wrong string in it, is exactly what this
    /// catches. Remove the assertion and a stable machine name could drift from the type
    /// silently, and every consumer matching on it would stop matching.
    #[test]
    fn every_variants_code_is_its_own_name_and_no_two_share_one() {
        let all = one_of_each();
        assert_eq!(all.len(), 25, "a variant was added or removed");
        let mut seen = std::collections::BTreeSet::new();
        for e in &all {
            let debug = format!("{e:?}");
            let name: &str = debug
                .split([' ', '{'])
                .next()
                .expect("a debug rendering begins with the variant name");
            assert_eq!(e.code(), name, "the code of {debug} is not its own name");
            assert!(seen.insert(e.code()), "{name} shares a code with another");
        }
        assert_eq!(seen.len(), 25);
    }

    /// The three exit codes are the executable's contract with a shell script: 12 means the
    /// thing asked for is not there, 13 means the tool or git broke, and everything else is
    /// a refusal at 10. Without this a refusal could start exiting 13, and a caller's
    /// `if [ $? -eq 10 ]` would stop recognising the case it was written for.
    #[test]
    fn the_exit_code_separates_missing_from_broken_from_refused() {
        use WorktreeError::*;
        for e in one_of_each() {
            let want = match &e {
                NoSuchWorktree { .. } | BaseDoesNotExist { .. } => EXIT_MISSING,
                GitCommandFailed { .. } | GitUnavailable { .. } | Io { .. } => EXIT_INTERNAL,
                _ => EXIT_REFUSED,
            };
            assert_eq!(e.exit_code(), want, "{}", e.code());
        }
        assert_eq!((EXIT_REFUSED, EXIT_MISSING, EXIT_INTERNAL), (10, 12, 13));
    }

    /// Every message is what a person reads instead of a debug struct, and each one has to
    /// name the thing it is about. A message that lost its path or its branch is a message
    /// that cannot be acted on; a `{path}` left unformatted would still compile.
    #[test]
    fn every_message_names_the_subject_it_refuses_about() {
        for e in one_of_each() {
            let msg = e.to_string();
            assert!(!msg.is_empty(), "{} has no message", e.code());
            assert!(
                !msg.contains('{') && !msg.contains('}'),
                "{} left a format placeholder in its message: {msg}",
                e.code()
            );
        }
        let e = WorktreeError::Misplaced {
            path: PathBuf::from("/tmp/scratch"),
            branch: "feature/x".into(),
            expected: PathBuf::from("/a/foo-wt/feature/x"),
        };
        let msg = e.to_string();
        assert!(msg.contains("/tmp/scratch"), "{msg}");
        assert!(msg.contains("feature/x"), "{msg}");
        assert!(msg.contains("/a/foo-wt/feature/x"), "{msg}");
        assert!(
            msg.contains("majordomus worktree migrate"),
            "the remedy is in the message: {msg}"
        );
    }

    /// Three messages format a field conditionally. A lock with no recorded reason must not
    /// render an empty pair of brackets, and a lock with one must show it — this is the
    /// only difference between a message a person can act on and one that says nothing.
    #[test]
    fn a_lock_without_a_reason_does_not_render_an_empty_parenthesis() {
        let quiet = WorktreeError::LockedWorktree {
            path: PathBuf::from("/a/wt"),
            reason: None,
        };
        assert!(
            quiet.to_string().starts_with("worktree /a/wt is locked;"),
            "{quiet}"
        );
        let empty = WorktreeError::LockedWorktree {
            path: PathBuf::from("/a/wt"),
            reason: Some(String::new()),
        };
        assert!(
            empty.to_string().starts_with("worktree /a/wt is locked;"),
            "git records an empty reason, and an empty reason is not a reason: {empty}"
        );
        let given = WorktreeError::LockedWorktree {
            path: PathBuf::from("/a/wt"),
            reason: Some("in use".into()),
        };
        assert!(
            given
                .to_string()
                .starts_with("worktree /a/wt is locked (in use);"),
            "{given}"
        );
        assert!(
            given.to_string().contains("git worktree unlock /a/wt"),
            "the command that clears it is in the message: {given}"
        );

        let unheld = WorktreeError::LockUnavailable {
            path: PathBuf::from("/a/l"),
            holder: None,
        };
        assert!(!unheld.to_string().contains("held by"), "{unheld}");
        let held = WorktreeError::LockUnavailable {
            path: PathBuf::from("/a/l"),
            holder: Some("pid 42".into()),
        };
        assert!(held.to_string().contains("held by pid 42"), "{held}");
    }

    /// A failed verification is the one message that must carry *what* differs: it is the
    /// only evidence a person has that a move lost something, and joining the differences
    /// is what puts it in front of them.
    #[test]
    fn a_failed_verification_lists_every_difference_it_was_given() {
        let e = WorktreeError::VerificationFailed {
            path: PathBuf::from("/a/foo-wt/feature/x"),
            differences: vec![
                "the index differs".into(),
                "the staged changes differ".into(),
            ],
        };
        let msg = e.to_string();
        assert!(msg.contains("the index differs"), "{msg}");
        assert!(msg.contains("the staged changes differ"), "{msg}");
        assert!(msg.contains("; "), "the differences are joined: {msg}");
        assert_eq!(e.exit_code(), EXIT_REFUSED);
    }

    /// `io` is the one constructor, and it has to carry the operating system's own words
    /// through: a message that said only "the file could not be read" would leave a person
    /// guessing between a missing directory and a permission problem.
    #[test]
    fn the_io_constructor_keeps_the_operating_systems_own_words() {
        let os = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let e = WorktreeError::io("/a/foo", &os);
        assert_eq!(e.code(), "Io");
        assert_eq!(e.exit_code(), EXIT_INTERNAL);
        let msg = e.to_string();
        assert!(msg.contains("/a/foo"), "{msg}");
        assert!(msg.contains("permission denied"), "{msg}");
    }
}
