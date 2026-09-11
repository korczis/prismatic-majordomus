//! Who this repository is, resolved once, from anywhere inside it.
//!
//! The whole subsystem turns on one question: run from `~/dev/foo-wt/feature/x/apps/src`,
//! which repository is that? `git rev-parse --show-toplevel` answers "the linked worktree",
//! which is true and useless — deriving a container from it would produce
//! `~/dev/foo-wt/feature/x-wt`, a new container per worktree, recursively. The primary
//! checkout is what the container is named after, and git names it: every work tree of one
//! repository shares a *common* git directory, and `git worktree list --porcelain` lists the
//! main work tree first. So the identity is read from the common directory and the porcelain
//! list, never from the current directory, and the answer is the same from every one of them.
//!
//! Paths are kept twice on purpose. `path` is what git reported, which is what a person
//! recognises and what git accepts back. `real` is that path canonicalised, which is what
//! containment and equality are decided on, so that a symlink cannot make two names for one
//! directory look like two directories — or one directory outside the container look like
//! one inside it.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::error::{Result, WorktreeError};
use super::git;
use super::topology::{self, WorktreeRecord};

/// A path as git names it and as the filesystem resolves it.
///
/// Both are needed and neither will do alone. The canonical form is the only one an
/// equality or a containment question can be answered on, because a symlink gives one
/// directory two names; the reported form is the only one a person recognises and the only
/// one git will take back. Keeping them in one value is what stops a caller from comparing
/// the wrong one — the mistake that lets a symlink outside the container read as inside it.
///
/// ```
/// use majordomus_cli::worktree::ResolvedPath;
/// let container = ResolvedPath { path: "/a/foo-wt".into(), real: "/a/foo-wt".into() };
/// let link = ResolvedPath { path: "/a/foo-wt/x".into(), real: "/tmp/elsewhere".into() };
/// assert!(!link.is_inside(&container), "the name says inside, the filesystem says no");
/// assert_eq!(link.path, std::path::Path::new("/a/foo-wt/x"), "the name is still reported");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPath {
    /// As reported: what a person reads and what git accepts back.
    pub path: PathBuf,
    /// Canonicalised, when the path exists: what comparisons use.
    pub real: PathBuf,
}

impl ResolvedPath {
    /// Resolve `path`. A path that does not exist (a prunable worktree, a destination not
    /// yet created) canonicalises to its lexically normalised self — with as much of its
    /// existing prefix resolved as exists — so a comparison still has something total to
    /// work with.
    ///
    /// Never fails, and that is the requirement: "is the path this worktree belongs at the
    /// path it is registered at?" has to be decidable before either path exists, and an
    /// error here would turn a question about a plan into a question about the filesystem.
    ///
    /// ```
    /// use majordomus_cli::worktree::ResolvedPath;
    /// let dir = tempfile::tempdir().unwrap();
    /// let root = dir.path().canonicalize().unwrap();
    ///
    /// // an existing directory resolves to its canonical self
    /// assert_eq!(ResolvedPath::of(dir.path()).real, root);
    ///
    /// // and a path that does not exist yet still gets a total answer, resolved through
    /// // the parent that does exist — which on macOS is a symlink more often than not
    /// let planned = ResolvedPath::of(dir.path().join("feature/x"));
    /// assert_eq!(planned.real, root.join("feature/x"));
    /// assert_eq!(planned.path, dir.path().join("feature/x"), "the name is kept as given");
    /// ```
    pub fn of(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let real = std::fs::canonicalize(&path).unwrap_or_else(|_| partial_canonical(&path));
        ResolvedPath { path, real }
    }

    /// Is this path the same directory as `other`, whatever either of them is called?
    ///
    /// Decided on the canonical forms only. Two paths that differ in every character name
    /// one directory when a symlink stands between them, and a subsystem that decides
    /// "this worktree is already registered" by comparing reported paths will register it
    /// twice.
    ///
    /// ```
    /// use majordomus_cli::worktree::ResolvedPath;
    /// let via_link = ResolvedPath { path: "/tmp/foo".into(), real: "/private/tmp/foo".into() };
    /// let direct = ResolvedPath {
    ///     path: "/private/tmp/foo".into(),
    ///     real: "/private/tmp/foo".into(),
    /// };
    /// assert!(via_link.same_as(&direct), "two names, one directory");
    ///
    /// let other = ResolvedPath { path: "/tmp/bar".into(), real: "/private/tmp/bar".into() };
    /// assert!(!via_link.same_as(&other));
    /// ```
    pub fn same_as(&self, other: &ResolvedPath) -> bool {
        self.real == other.real
    }

    /// Is this path strictly below `root` — a descendant, not `root` itself?
    ///
    /// Decided on the canonical forms, so `foo-wt/link -> /tmp/elsewhere` is not under
    /// `foo-wt` however its name reads.
    ///
    /// Strictly below, because the container is not one of its own worktrees, and
    /// component-wise, because `foo-wt2` shares a textual prefix with `foo-wt` and is a
    /// different repository's container.
    ///
    /// ```
    /// use majordomus_cli::worktree::ResolvedPath;
    /// let at = |p: &str| ResolvedPath { path: p.into(), real: p.into() };
    /// let container = at("/a/foo-wt");
    ///
    /// assert!(at("/a/foo-wt/feature/x").is_inside(&container));
    /// assert!(!container.is_inside(&container), "the container is not below itself");
    /// assert!(!at("/a/foo-wt2").is_inside(&container), "a name prefix is not containment");
    /// ```
    pub fn is_inside(&self, root: &ResolvedPath) -> bool {
        self.real != root.real && self.real.starts_with(&root.real)
    }
}

/// Canonicalise the longest existing prefix of a path and append the rest lexically
/// normalised. Never fails and never invents: a symlinked parent of a path that does not
/// exist yet is still resolved, which is what makes "the expected path equals this
/// registered path" decidable before the expected path is created.
fn partial_canonical(p: &Path) -> PathBuf {
    let normalised = lexical(p);
    let mut existing = normalised.clone();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if let Ok(real) = std::fs::canonicalize(&existing) {
            let mut out = real;
            for c in rest.iter().rev() {
                out.push(c);
            }
            return out;
        }
        match (existing.file_name(), existing.parent()) {
            (Some(name), Some(parent)) if !parent.as_os_str().is_empty() => {
                rest.push(name.to_os_string());
                existing = parent.to_path_buf();
            }
            _ => return normalised,
        }
    }
}

/// Lexical normalisation for a path that does not exist: drop `.`, resolve `..` against the
/// component before it, and leave everything else alone. Never touches the filesystem.
fn lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                let at_root = out.parent().is_none() && out.has_root();
                if !at_root && !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Where the trunk was learned from, in the order it is looked for.
///
/// The source is reported and not only used, because the four ways of learning a trunk are
/// not equally trustworthy: the remote's own HEAD is the repository's answer, and the
/// primary checkout's current branch is a guess that happens to be right most of the time.
/// A diagnostic that says the trunk is `master` without saying which of the four said so
/// cannot be argued with, and `Unknown` has to be a value rather than a default name for
/// exactly that reason.
///
/// ```
/// use majordomus_cli::worktree::TrunkSource;
/// // the wire form every report, schema and transport carries
/// let json = serde_json::to_string(&TrunkSource::RemoteHead).unwrap();
/// assert_eq!(json, "\"remote_head\"");
/// let back: TrunkSource = serde_json::from_str("\"conventional_name\"").unwrap();
/// assert_eq!(back, TrunkSource::ConventionalName);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrunkSource {
    /// `refs/remotes/<remote>/HEAD`: what the remote calls its default branch.
    RemoteHead,
    /// `init.defaultBranch` in the git configuration, and that branch exists locally.
    DefaultBranchConfig,
    /// Exactly one of `main` and `master` exists locally.
    ConventionalName,
    /// The branch the primary checkout holds, because nothing else said.
    PrimaryCheckout,
    /// Nothing said; the trunk is unknown and every check that needs it says so.
    Unknown,
}

/// The trunk branch and how it was decided. Discovered state, never domain logic: `master`
/// here is a fact about this repository and `main` would be one about another.
///
/// The branch is optional and the source is not. A repository whose trunk could not be
/// discovered has no trunk — not `main`, not the first branch that happens to exist — and
/// every check built on the trunk has to report that it cannot decide rather than decide
/// against a name it invented.
///
/// ```
/// use majordomus_cli::worktree::{Trunk, TrunkSource};
/// let found = Trunk { branch: Some("master".into()), source: TrunkSource::RemoteHead };
/// assert!(found.is("master"));
/// assert!(!found.is("main"), "a repository has one trunk, not both conventional names");
///
/// let unknown = Trunk { branch: None, source: TrunkSource::Unknown };
/// assert!(!unknown.is("master"), "an unknown trunk matches nothing, not everything");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trunk {
    /// The branch, short, when one was found.
    pub branch: Option<String>,
    /// How it was found.
    pub source: TrunkSource,
}

impl Trunk {
    /// Is `branch` the trunk of this repository?
    ///
    /// False for every branch when the trunk is unknown, which is the answer that makes
    /// the guard safe: a repository whose trunk was not discovered must not have some
    /// branch treated as the trunk by default, because the one thing the guard does with
    /// the trunk is decide which branch is allowed to live in the primary checkout.
    ///
    /// ```
    /// use majordomus_cli::worktree::{Trunk, TrunkSource};
    /// let t = Trunk { branch: Some("master".into()), source: TrunkSource::PrimaryCheckout };
    /// assert!(t.is("master"));
    /// assert!(!t.is("feature/x"));
    /// ```
    pub fn is(&self, branch: &str) -> bool {
        self.branch.as_deref() == Some(branch)
    }
}

/// The repository a command is operating on, and every work tree git has registered for it.
///
/// Built once per command and passed around. A handful of subprocesses go into it and no
/// more: the identity questions, one `worktree list`, and the trunk lookups.
///
/// Every question it answers is about the repository and none is about the directory the
/// command was run from, with one exception that is named: [`Self::current_worktree`],
/// which is the only place the current directory decides anything. That asymmetry is the
/// whole point of the type — a subsystem that asked git afresh in each function would
/// answer "the linked worktree" to half of them and "the repository" to the other half.
///
/// ```no_run
/// use majordomus_cli::worktree::RepositoryIdentity;
/// use std::path::Path;
///
/// // run from deep inside a linked worktree, not from the primary checkout
/// let repo = RepositoryIdentity::discover(Path::new("/a/foo-wt/feature/x/apps/cli")).unwrap();
/// assert_eq!(repo.primary_worktree().path, Path::new("/a/foo"), "not where we stand");
/// assert_eq!(repo.repository_name(), "foo", "what the container is named after");
/// assert!(!repo.registered_worktrees().is_empty(), "the main work tree is always listed");
/// ```
#[derive(Debug, Clone)]
pub struct RepositoryIdentity {
    git_common_dir: ResolvedPath,
    primary: ResolvedPath,
    current: ResolvedPath,
    records: Vec<WorktreeRecord>,
    trunk: Trunk,
}

impl RepositoryIdentity {
    /// Resolve the repository from any directory inside any of its work trees.
    ///
    /// The one entry point, and the only function here that runs git for identity. A bare
    /// repository is refused by name rather than half-supported: the topology is defined
    /// against a primary checkout, and a bare repository has none to name a container
    /// after. A directory outside any work tree is refused too, with the directory in the
    /// message, because "not a git repository" without saying which path was looked at is
    /// the least useful thing this subsystem could say.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::RepositoryIdentity;
    /// use std::path::Path;
    ///
    /// // the central invariant: every directory of every work tree gives one answer
    /// let deep = RepositoryIdentity::discover(Path::new("/a/foo-wt/feature/x/apps")).unwrap();
    /// let top = RepositoryIdentity::discover(Path::new("/a/foo")).unwrap();
    /// assert!(deep.git_common_dir().same_as(top.git_common_dir()));
    /// assert!(deep.primary_worktree().same_as(top.primary_worktree()));
    /// ```
    pub fn discover(start: &Path) -> Result<Self> {
        let probe = git::try_run(
            start,
            &["rev-parse", "--is-bare-repository", "--git-common-dir"],
        )?;
        if probe.status != Some(0) {
            return Err(WorktreeError::NotInGitRepository {
                start: start.to_path_buf(),
            });
        }
        let text = probe.text()?;
        let mut lines = text.lines();
        let bare = lines.next().unwrap_or("false").trim() == "true";
        let common_raw = lines.next().unwrap_or("").trim().to_string();
        if common_raw.is_empty() {
            return Err(WorktreeError::NotInGitRepository {
                start: start.to_path_buf(),
            });
        }
        // `--git-common-dir` is relative to the current directory when the current directory
        // is inside the work tree, and absolute otherwise. Both forms are answers.
        let common = {
            let p = PathBuf::from(&common_raw);
            if p.is_absolute() {
                p
            } else {
                start.join(p)
            }
        };
        let git_common_dir = ResolvedPath::of(common);
        if bare {
            return Err(WorktreeError::BareRepositoryUnsupported {
                git_dir: git_common_dir.path.clone(),
            });
        }

        // The current work tree: `--show-toplevel` is exactly this question, and here it is
        // the right one — this is the only place the current directory decides anything.
        let top = git::run(start, &["rev-parse", "--show-toplevel"])?.text()?;
        let current = ResolvedPath::of(PathBuf::from(top));

        let records = topology::read(start)?;
        let primary = ResolvedPath::of(topology::primary(&records, &git_common_dir.path)?);
        let trunk = detect_trunk(&primary.path, &records)?;
        Ok(RepositoryIdentity {
            git_common_dir,
            primary,
            current,
            records,
            trunk,
        })
    }

    /// Re-read the registered work trees. Under the lock, before a mutation, because the
    /// topology may have changed since discovery.
    ///
    /// Only the records are re-read. The identity itself — the common git directory, the
    /// primary checkout, the work tree the command runs in — cannot change under a running
    /// command, so re-deriving it would spend subprocesses to confirm what is already
    /// known. What can change is the set of worktrees, and it changes because another agent
    /// is working the same repository, which is why the call belongs after the lock is held
    /// and not before.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::{RepositoryIdentity, WorktreeLock};
    /// use std::path::Path;
    ///
    /// let mut repo = RepositoryIdentity::discover(Path::new("/a/foo")).unwrap();
    /// let _lock = WorktreeLock::acquire(&repo.git_common_dir().path).unwrap();
    /// repo.refresh().unwrap();
    /// assert!(!repo.registered_worktrees().is_empty());
    /// ```
    pub fn refresh(&mut self) -> Result<()> {
        self.records = topology::read(&self.primary.path)?;
        Ok(())
    }

    /// The common git directory every work tree of this repository shares. It is the
    /// repository's identity: two paths belong to one repository exactly when this matches.
    pub fn git_common_dir(&self) -> &ResolvedPath {
        &self.git_common_dir
    }

    /// The primary checkout — the main work tree, never a linked one, whatever directory the
    /// command was run from.
    pub fn primary_worktree(&self) -> &ResolvedPath {
        &self.primary
    }

    /// The work tree the command was run inside.
    pub fn current_worktree(&self) -> &ResolvedPath {
        &self.current
    }

    /// The trunk this repository was found to have, together with which of the four
    /// lookups found it. Discovered at construction and not re-asked: the trunk of a
    /// repository does not change under a command, and the source travels with the name so
    /// that a diagnostic can say why it believes what it says.
    pub fn trunk(&self) -> &Trunk {
        &self.trunk
    }

    /// The primary checkout's directory name: what the container is named after.
    ///
    /// The directory name, not the remote's name and not anything configured. Two clones of
    /// one upstream into differently named directories are two repositories with two
    /// containers, which is the behaviour a person expects from a name they chose.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::{container_root, RepositoryIdentity};
    /// use std::path::Path;
    ///
    /// let repo = RepositoryIdentity::discover(Path::new("/a/foo")).unwrap();
    /// let container = container_root(&repo.primary_worktree().path).unwrap();
    /// assert!(container.ends_with(format!("{}-wt", repo.repository_name())));
    /// ```
    pub fn repository_name(&self) -> String {
        self.primary
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    /// Every work tree git has registered, primary first, exactly as the porcelain listed
    /// them.
    pub fn registered_worktrees(&self) -> &[WorktreeRecord] {
        &self.records
    }

    /// Is this record the primary checkout?
    ///
    /// Compared on the resolved paths, not on the reported ones, so a record git named
    /// through a symlink is still recognised as the main work tree. Exactly one record of a
    /// non-bare repository answers true, and it is the one the primary checkout is at —
    /// never the one the command happens to be running in.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::RepositoryIdentity;
    /// use std::path::Path;
    ///
    /// let repo = RepositoryIdentity::discover(Path::new("/a/foo-wt/feature/x")).unwrap();
    /// let records = repo.registered_worktrees();
    /// assert!(repo.is_primary(&records[0]), "git lists the main work tree first");
    /// assert_eq!(records.iter().filter(|r| repo.is_primary(r)).count(), 1);
    /// ```
    pub fn is_primary(&self, record: &WorktreeRecord) -> bool {
        ResolvedPath::of(&record.path).same_as(&self.primary)
    }

    /// Is the command running inside this record's work tree?
    ///
    /// The one question here whose answer depends on where the command was started. It is
    /// path identity and not containment: the record whose work tree the current directory
    /// belongs to, resolved through symlinks, so at most one record can answer true.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::RepositoryIdentity;
    /// use std::path::Path;
    ///
    /// let repo = RepositoryIdentity::discover(Path::new("/a/foo-wt/feature/x")).unwrap();
    /// let here = repo.registered_worktrees().iter().filter(|r| repo.is_current(r)).count();
    /// assert!(here <= 1, "a directory is inside at most one work tree");
    /// ```
    pub fn is_current(&self, record: &WorktreeRecord) -> bool {
        ResolvedPath::of(&record.path).same_as(&self.current)
    }

    /// The record of the work tree the command runs in, when git lists it.
    ///
    /// `None` is a real answer and not an error: a worktree can be removed from disk while
    /// a shell still stands in it, and a command that reports the topology from there
    /// should describe the repository rather than refuse to speak.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::RepositoryIdentity;
    /// use std::path::Path;
    ///
    /// let repo = RepositoryIdentity::discover(Path::new("/a/foo-wt/feature/x")).unwrap();
    /// let here = repo.current_record().expect("this work tree is registered");
    /// assert!(repo.is_current(here));
    /// ```
    pub fn current_record(&self) -> Option<&WorktreeRecord> {
        self.records.iter().find(|r| self.is_current(r))
    }

    /// The record holding a branch, when one does.
    ///
    /// At most one, because git refuses to check a branch out in two work trees, and that
    /// refusal is what makes the branch-to-worktree mapping a function rather than a
    /// convention. A detached work tree holds no branch and is never the answer.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::RepositoryIdentity;
    /// use std::path::Path;
    ///
    /// let repo = RepositoryIdentity::discover(Path::new("/a/foo")).unwrap();
    /// if let Some(r) = repo.record_of_branch("feature/x") {
    ///     assert_eq!(r.branch.as_deref(), Some("feature/x"));
    ///     assert!(!r.detached, "a record holding a branch is not detached");
    /// }
    /// ```
    pub fn record_of_branch(&self, branch: &str) -> Option<&WorktreeRecord> {
        self.records
            .iter()
            .find(|r| r.branch.as_deref() == Some(branch))
    }

    /// The record registered at a path, whatever the path is called.
    ///
    /// The lookup a destination check needs: "is anything already registered here?" has to
    /// be answered about the directory and not about the spelling, or a worktree reached
    /// through a symlinked parent looks like a free path and `git worktree add` fails half
    /// way through instead of being refused up front.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::RepositoryIdentity;
    /// use std::path::Path;
    ///
    /// let repo = RepositoryIdentity::discover(Path::new("/a/foo")).unwrap();
    /// let found = repo
    ///     .record_at(repo.primary_worktree())
    ///     .expect("the primary checkout is always registered");
    /// assert!(repo.is_primary(found));
    /// ```
    pub fn record_at(&self, path: &ResolvedPath) -> Option<&WorktreeRecord> {
        self.records
            .iter()
            .find(|r| ResolvedPath::of(&r.path).same_as(path))
    }
}

/// The trunk of a repository, found in the order [`TrunkSource`] lists. Never hardcoded:
/// the remote's HEAD is asked first, then the configuration, then the two conventional
/// names, then the primary checkout's own branch.
pub fn detect_trunk(primary: &Path, records: &[WorktreeRecord]) -> Result<Trunk> {
    // 1. what the remote calls its default branch
    let remotes = git::try_run(primary, &["remote"])?;
    if remotes.status == Some(0) {
        let text = remotes.text()?;
        for remote in text.lines().map(str::trim).filter(|r| !r.is_empty()) {
            let head = git::try_run(
                primary,
                &["symbolic-ref", "-q", &format!("refs/remotes/{remote}/HEAD")],
            )?;
            if head.status == Some(0) {
                let full = head.text()?;
                if let Some(short) = full.strip_prefix(&format!("refs/remotes/{remote}/")) {
                    if !short.is_empty() {
                        return Ok(Trunk {
                            branch: Some(short.to_string()),
                            source: TrunkSource::RemoteHead,
                        });
                    }
                }
            }
        }
    }
    // 2. the configured default branch, when it exists here
    let configured = git::try_run(primary, &["config", "--get", "init.defaultBranch"])?;
    if configured.status == Some(0) {
        let name = configured.text()?;
        if !name.is_empty() && git::branch_exists(primary, &name)? {
            return Ok(Trunk {
                branch: Some(name),
                source: TrunkSource::DefaultBranchConfig,
            });
        }
    }
    // 3. exactly one of the conventional names
    let conventional: Vec<&str> = ["main", "master"]
        .into_iter()
        .filter(|b| git::branch_exists(primary, b).unwrap_or(false))
        .collect();
    if conventional.len() == 1 {
        return Ok(Trunk {
            branch: Some(conventional[0].to_string()),
            source: TrunkSource::ConventionalName,
        });
    }
    // 4. the primary checkout's own branch
    if let Some(branch) = records.first().and_then(|r| r.branch.clone()) {
        return Ok(Trunk {
            branch: Some(branch),
            source: TrunkSource::PrimaryCheckout,
        });
    }
    Ok(Trunk {
        branch: None,
        source: TrunkSource::Unknown,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexical_normalisation_resolves_dot_and_dotdot_without_the_filesystem() {
        assert_eq!(lexical(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
        assert_eq!(lexical(Path::new("/a/../..")), PathBuf::from("/"));
        assert_eq!(lexical(Path::new("../x")), PathBuf::from("../x"));
    }

    #[test]
    fn containment_is_decided_on_the_resolved_form_and_excludes_the_root_itself() {
        let root = ResolvedPath {
            path: "/a/foo-wt".into(),
            real: "/a/foo-wt".into(),
        };
        let inside = ResolvedPath {
            path: "/a/foo-wt/feature/x".into(),
            real: "/a/foo-wt/feature/x".into(),
        };
        let escaped = ResolvedPath {
            path: "/a/foo-wt/link".into(),
            real: "/tmp/elsewhere".into(),
        };
        assert!(inside.is_inside(&root));
        assert!(!root.is_inside(&root));
        assert!(!escaped.is_inside(&root));
    }

    #[test]
    fn a_sibling_prefix_is_not_containment() {
        let root = ResolvedPath {
            path: "/a/foo-wt".into(),
            real: "/a/foo-wt".into(),
        };
        let sibling = ResolvedPath {
            path: "/a/foo-wt2".into(),
            real: "/a/foo-wt2".into(),
        };
        assert!(!sibling.is_inside(&root));
    }

    #[test]
    fn a_path_that_does_not_exist_yet_resolves_through_its_existing_parent() {
        let dir = tempfile::tempdir().unwrap();
        let real_dir = dir.path().canonicalize().unwrap();
        let target = dir.path().join("not/yet/here");
        let r = ResolvedPath::of(&target);
        assert_eq!(r.real, real_dir.join("not/yet/here"));
        // and through a symlinked parent, which is the case that matters on macOS where
        // /tmp is a link to /private/tmp
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&real_dir, dir.path().join("link")).unwrap();
            let via = ResolvedPath::of(dir.path().join("link/deeper/x"));
            assert_eq!(via.real, real_dir.join("deeper/x"));
        }
    }
}
