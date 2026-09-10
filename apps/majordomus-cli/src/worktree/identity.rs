//! Who this repository is, resolved once, from anywhere inside it.
//!
//! The whole subsystem turns on one question: run from `~/dev/foo-wt/issue-123/apps/x/src`,
//! which repository is that? `git rev-parse --show-toplevel` answers "the linked worktree",
//! which is true and useless — deriving a container from it would produce
//! `~/dev/foo-wt/issue-123-wt`, a new container per worktree, recursively. The primary
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
    /// yet created) canonicalises to its lexically normalised self, so a comparison still
    /// has something total to work with.
    pub fn of(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let real = std::fs::canonicalize(&path).unwrap_or_else(|_| lexical(&path));
        ResolvedPath { path, real }
    }

    /// Is this path the same directory as `other`, whatever it is called?
    pub fn same_as(&self, other: &ResolvedPath) -> bool {
        self.real == other.real
    }

    /// Is this path strictly below `root` — a child, not `root` itself?
    ///
    /// Decided on the canonical forms, so `foo-wt/link -> /tmp/elsewhere` is not under
    /// `foo-wt` however its name reads.
    pub fn is_inside(&self, root: &ResolvedPath) -> bool {
        self.real != root.real && self.real.starts_with(&root.real)
    }

    /// Is this path a *direct* child of `root`? The container holds worktrees, not trees of
    /// worktrees; `foo-wt/a/b` is inside the container and is not one of its worktrees.
    pub fn is_directly_inside(&self, root: &ResolvedPath) -> bool {
        self.real.parent() == Some(root.real.as_path())
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
                // `..` above the root is the root: an absolute path cannot escape upwards,
                // and keeping a literal `..` in the canonical form would make two names for
                // the same directory compare unequal — which is exactly what containment
                // must not get wrong.
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

/// The repository a command is operating on, and every work tree git has registered for it.
///
/// Built once per command and passed around. Three subprocesses go into it and no more:
/// two `rev-parse` questions and one `worktree list`.
#[derive(Debug, Clone)]
pub struct RepositoryIdentity {
    git_common_dir: ResolvedPath,
    primary: ResolvedPath,
    current: ResolvedPath,
    records: Vec<WorktreeRecord>,
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
        Ok(RepositoryIdentity {
            git_common_dir,
            primary,
            current,
            records,
        })
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

    /// The primary checkout's directory name: what the container is named after.
    pub fn repository_name(&self) -> String {
        self.primary
            .path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    /// The directory the primary checkout sits in: where a sibling container goes.
    pub fn repository_parent(&self) -> Result<PathBuf> {
        self.primary
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .ok_or_else(|| WorktreeError::NoParentDirectory {
                primary: self.primary.path.clone(),
            })
    }

    /// Every work tree git has registered, primary first, exactly as the porcelain listed
    /// them. Read once at discovery; nothing re-runs `git worktree list`.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexical_normalisation_resolves_dot_and_dotdot_without_the_filesystem() {
        assert_eq!(lexical(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
        assert_eq!(lexical(Path::new("/a/../..")), PathBuf::from("/"));
        // a relative path may still carry `..`, because there is nothing to resolve it against
        assert_eq!(lexical(Path::new("../x")), PathBuf::from("../x"));
    }

    #[test]
    fn containment_is_decided_on_the_resolved_form_and_excludes_the_root_itself() {
        let root = ResolvedPath {
            path: "/a/foo-wt".into(),
            real: "/a/foo-wt".into(),
        };
        let inside = ResolvedPath {
            path: "/a/foo-wt/x".into(),
            real: "/a/foo-wt/x".into(),
        };
        let deeper = ResolvedPath {
            path: "/a/foo-wt/x/y".into(),
            real: "/a/foo-wt/x/y".into(),
        };
        let escaped = ResolvedPath {
            path: "/a/foo-wt/link".into(),
            real: "/tmp/elsewhere".into(),
        };
        assert!(inside.is_inside(&root) && inside.is_directly_inside(&root));
        assert!(deeper.is_inside(&root) && !deeper.is_directly_inside(&root));
        assert!(!root.is_inside(&root));
        assert!(
            !escaped.is_inside(&root),
            "a symlink out of the root is out of the root"
        );
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
        assert!(
            !sibling.is_inside(&root),
            "/a/foo-wt2 is not under /a/foo-wt"
        );
    }
}
