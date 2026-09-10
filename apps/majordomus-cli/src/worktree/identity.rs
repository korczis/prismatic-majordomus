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
    pub fn of(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let real = std::fs::canonicalize(&path).unwrap_or_else(|_| partial_canonical(&path));
        ResolvedPath { path, real }
    }

    /// Is this path the same directory as `other`, whatever it is called?
    pub fn same_as(&self, other: &ResolvedPath) -> bool {
        self.real == other.real
    }

    /// Is this path strictly below `root` — a descendant, not `root` itself?
    ///
    /// Decided on the canonical forms, so `foo-wt/link -> /tmp/elsewhere` is not under
    /// `foo-wt` however its name reads.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trunk {
    /// The branch, short, when one was found.
    pub branch: Option<String>,
    /// How it was found.
    pub source: TrunkSource,
}

impl Trunk {
    /// Is `branch` the trunk?
    pub fn is(&self, branch: &str) -> bool {
        self.branch.as_deref() == Some(branch)
    }
}

/// The repository a command is operating on, and every work tree git has registered for it.
///
/// Built once per command and passed around. A handful of subprocesses go into it and no
/// more: the identity questions, one `worktree list`, and the trunk lookups.
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

    /// The trunk, as discovered.
    pub fn trunk(&self) -> &Trunk {
        &self.trunk
    }

    /// The primary checkout's directory name: what the container is named after.
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
    pub fn is_primary(&self, record: &WorktreeRecord) -> bool {
        ResolvedPath::of(&record.path).same_as(&self.primary)
    }

    /// Is the command running inside this record's work tree?
    pub fn is_current(&self, record: &WorktreeRecord) -> bool {
        ResolvedPath::of(&record.path).same_as(&self.current)
    }

    /// The record of the work tree the command runs in, when git lists it.
    pub fn current_record(&self) -> Option<&WorktreeRecord> {
        self.records.iter().find(|r| self.is_current(r))
    }

    /// The record holding a branch, when one does.
    pub fn record_of_branch(&self, branch: &str) -> Option<&WorktreeRecord> {
        self.records
            .iter()
            .find(|r| r.branch.as_deref() == Some(branch))
    }

    /// The record registered at a path, whatever the path is called.
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

    // ---------------------------------------------------- the trunk, and the identity

    /// A repository with one commit, whose branch is named by the caller. Nothing about the
    /// developer's own git configuration is allowed to reach it: `init.defaultBranch` is set
    /// per-repository by every case that cares.
    fn repo_on(branch: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        git_in(dir.path(), &["init", "-q", "-b", branch, "."]);
        // Neutralise whatever this machine configures globally: an empty value is not a
        // branch name, so the configured-default rule stands down unless a case sets it.
        git_in(dir.path(), &["config", "init.defaultBranch", ""]);
        git_in(
            dir.path(),
            &[
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "i",
            ],
        );
        dir
    }

    fn git_in(cwd: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn records_of(primary: &Path) -> Vec<WorktreeRecord> {
        topology::read(primary).unwrap()
    }

    /// The trunk is discovered, never configured here, and the *order* is the contract: what
    /// the remote calls its default branch outranks a local convention, because a fork whose
    /// primary branch is `develop` must not be told its trunk is `main` merely because that
    /// name exists. Collapsing the order would make the primary-checkout rule — and every
    /// refusal built on it — wrong for exactly those repositories.
    #[test]
    fn the_remotes_own_head_outranks_every_local_guess() {
        let dir = repo_on("develop");
        // A second branch with a conventional name, so that the local rules would answer
        // differently if they were consulted first.
        git_in(dir.path(), &["branch", "main"]);
        git_in(dir.path(), &["config", "init.defaultBranch", "main"]);
        // A remote whose HEAD names `develop`, written locally: no network is touched.
        git_in(
            dir.path(),
            &["remote", "add", "origin", "https://example.invalid/x.git"],
        );
        git_in(
            dir.path(),
            &["update-ref", "refs/remotes/origin/develop", "HEAD"],
        );
        git_in(
            dir.path(),
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/develop",
            ],
        );
        let trunk = detect_trunk(dir.path(), &records_of(dir.path())).unwrap();
        assert_eq!(trunk.source, TrunkSource::RemoteHead);
        assert_eq!(trunk.branch.as_deref(), Some("develop"));
        assert!(trunk.is("develop") && !trunk.is("main"));
    }

    /// `init.defaultBranch` is consulted only when that branch actually exists here. A
    /// configuration naming a branch this repository has never had would otherwise make the
    /// primary checkout permanently "on a non-trunk branch", and the guard would refuse
    /// every commit in a repository that is perfectly in order.
    #[test]
    fn the_configured_default_branch_counts_only_when_the_branch_is_really_here() {
        let dir = repo_on("trunk");
        git_in(dir.path(), &["config", "init.defaultBranch", "trunk"]);
        let trunk = detect_trunk(dir.path(), &records_of(dir.path())).unwrap();
        assert_eq!(trunk.source, TrunkSource::DefaultBranchConfig);
        assert_eq!(trunk.branch.as_deref(), Some("trunk"));

        let dir = repo_on("trunk");
        git_in(
            dir.path(),
            &["config", "init.defaultBranch", "never-created"],
        );
        let trunk = detect_trunk(dir.path(), &records_of(dir.path())).unwrap();
        assert_ne!(
            trunk.source,
            TrunkSource::DefaultBranchConfig,
            "a configured name that is not a branch here is not this repository's trunk"
        );
        assert_eq!(trunk.branch.as_deref(), Some("trunk"));
        assert_eq!(trunk.source, TrunkSource::PrimaryCheckout);
    }

    /// The conventional names are a tie-break, not a rule: `main` and `master` both present
    /// says nothing about which one is the trunk, so the guess is declined and the primary
    /// checkout's own branch answers instead. Picking one would be this tool inventing a
    /// fact about somebody's repository.
    #[test]
    fn one_conventional_name_decides_and_two_decline_to() {
        let dir = repo_on("master");
        let trunk = detect_trunk(dir.path(), &records_of(dir.path())).unwrap();
        assert_eq!(trunk.source, TrunkSource::ConventionalName);
        assert_eq!(trunk.branch.as_deref(), Some("master"));

        git_in(dir.path(), &["branch", "main"]);
        let trunk = detect_trunk(dir.path(), &records_of(dir.path())).unwrap();
        assert_eq!(
            trunk.source,
            TrunkSource::PrimaryCheckout,
            "with both names present the convention says nothing"
        );
        assert_eq!(trunk.branch.as_deref(), Some("master"));
    }

    /// With nothing to go on the trunk is unknown, and it says so. A fabricated trunk would
    /// make the primary checkout's branch an error in a repository that has no trunk at all,
    /// which is why `is_in_place` treats `Unknown` as an exemption rather than a failure.
    #[test]
    fn nothing_to_go_on_is_answered_unknown_rather_than_guessed() {
        let dir = repo_on("develop");
        // Detached: the primary checkout has no branch to fall back to either.
        git_in(dir.path(), &["checkout", "-q", "--detach", "HEAD"]);
        let trunk = detect_trunk(dir.path(), &records_of(dir.path())).unwrap();
        assert_eq!(trunk.source, TrunkSource::Unknown);
        assert_eq!(trunk.branch, None);
        assert!(!trunk.is("develop"), "an unknown trunk is not every branch");
    }

    /// The identity is what the container is named after and what every worktree of the
    /// repository shares. Discovered from any directory inside any work tree, it must answer
    /// the same primary checkout and the same common git directory — this is the property
    /// that makes `<repo>-wt` one container rather than one per directory somebody stood in.
    #[test]
    fn the_identity_is_the_same_from_a_subdirectory_as_from_the_root() {
        let dir = repo_on("master");
        let deep = dir.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();

        let root = RepositoryIdentity::discover(dir.path()).unwrap();
        let inner = RepositoryIdentity::discover(&deep).unwrap();
        assert!(root.primary_worktree().same_as(inner.primary_worktree()));
        assert!(root.git_common_dir().same_as(inner.git_common_dir()));
        assert_eq!(root.repository_name(), inner.repository_name());
        assert_eq!(
            root.repository_name(),
            dir.path().file_name().unwrap().to_string_lossy(),
            "the container is named after the primary checkout's directory and nothing else"
        );
        assert!(
            inner.current_worktree().same_as(root.primary_worktree()),
            "a subdirectory is inside the same work tree"
        );
    }

    /// A directory outside every repository is refused by name. Walking on up to whatever
    /// repository happens to be an ancestor — a home directory under version control is the
    /// case — would answer confidently about the wrong one.
    #[test]
    fn a_directory_in_no_repository_is_refused_rather_than_attributed_to_an_ancestor() {
        let outside = tempfile::tempdir().unwrap();
        match RepositoryIdentity::discover(outside.path()) {
            Err(e) => assert_eq!(e.code(), "NotInGitRepository"),
            Ok(found) => panic!(
                "a temporary directory was attributed to {}",
                found.primary_worktree().path.display()
            ),
        }
    }

    /// The record lookups are what `judge`, the guard and the migration use to tell "this
    /// worktree" from "some other worktree". A path is matched on its resolved form, so the
    /// same directory reached by two spellings is one worktree; a branch is matched exactly,
    /// because a prefix match would confuse `feature/x` with `feature/x-2`.
    #[test]
    fn a_record_is_found_by_resolved_path_and_by_exact_branch_name() {
        let dir = repo_on("master");
        let identity = RepositoryIdentity::discover(dir.path()).unwrap();
        let primary = identity.primary_worktree().clone();

        let found = identity
            .record_at(&primary)
            .expect("the primary checkout is registered");
        assert_eq!(found.branch.as_deref(), Some("master"));
        assert!(identity.is_primary(found) && identity.is_current(found));
        assert_eq!(
            identity.current_record().map(|r| &r.path),
            Some(&found.path)
        );

        // The same directory, spelled with a `.` component: one worktree, not two.
        let spelled = ResolvedPath::of(primary.path.join("."));
        assert!(
            identity.record_at(&spelled).is_some(),
            "a path is matched on what it resolves to, not on how it is written"
        );

        assert_eq!(
            identity.record_of_branch("master").map(|r| &r.path),
            Some(&found.path)
        );
        assert!(identity.record_of_branch("mast").is_none());
        assert!(
            identity.record_of_branch("master-2").is_none(),
            "an exact name, or `feature/x` would claim `feature/x-2`'s worktree"
        );
        assert!(identity
            .record_at(&ResolvedPath::of("/definitely/not/here"))
            .is_none());
    }

    /// A migration moves worktrees and then asks git again. `refresh` is what makes the
    /// second answer the new one: without it the verification would check the registration
    /// as it was before the move and call every migration verified.
    #[test]
    fn refreshing_picks_up_a_worktree_registered_since_discovery() {
        let outer = tempfile::tempdir().unwrap();
        let primary = outer.path().join("nested/repo");
        std::fs::create_dir_all(&primary).unwrap();
        git_in(&primary, &["init", "-q", "-b", "master", "."]);
        git_in(
            &primary,
            &[
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=t",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "i",
            ],
        );
        let mut identity = RepositoryIdentity::discover(&primary).unwrap();
        assert_eq!(identity.registered_worktrees().len(), 1);

        let added = outer.path().join("repo-wt/feature/x");
        git_in(
            &primary,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feature/x",
                added.to_str().unwrap(),
            ],
        );
        assert_eq!(
            identity.registered_worktrees().len(),
            1,
            "the identity is a snapshot until it is refreshed"
        );
        identity.refresh().unwrap();
        assert_eq!(identity.registered_worktrees().len(), 2);
        let registered = identity
            .record_of_branch("feature/x")
            .map(|r| ResolvedPath::of(&r.path))
            .expect("the new worktree is registered after the refresh");
        assert!(
            registered.same_as(&ResolvedPath::of(&added)),
            "{} is not the directory that was added",
            registered.path.display()
        );
        assert!(
            identity
                .primary_worktree()
                .same_as(&ResolvedPath::of(&primary)),
            "refreshing does not move the primary checkout"
        );
    }
}
