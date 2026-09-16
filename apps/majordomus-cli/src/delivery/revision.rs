//! Where git put a feature: the paths that implement it, the latest commit touching them at
//! `HEAD` and on the trunk, and whether one commit contains another.
//!
//! Every answer is read from git on the call and none is remembered. What implements a
//! feature is derived from what its file already names, against the registries that own
//! each name — nothing here is a list of paths somebody wrote down:
//!
//! - a capability module: the file its descriptors were composed in, as the registry says;
//! - a shell command: `lib/<command>.sh`, because that is the file `bin/majordomus` sources
//!   to dispatch it, when the repository has one;
//! - a claim: the `implementation` path `docs/CLAIMS.yaml` gives it.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::git::read_only;
use crate::index::Index;
use crate::product::ResolvedRefs;

/// Where a feature's implementation is, relative to the trunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Revision {
    /// Git could not answer: no trunk in this clone, no git, no commit at `HEAD`.
    Unknown {
        /// Why, in one clause.
        reason: String,
    },
    /// The feature names nothing that has an implementation path.
    NoImplementation,
    /// It names paths, and no commit on either side touches them.
    NotFound,
    /// The latest change at `HEAD` is not on the trunk, and the content differs.
    OnBranch {
        /// The latest commit touching the paths reachable from `HEAD`.
        head: String,
        /// The latest commit touching them on the trunk, when there is one.
        master: Option<String>,
    },
    /// The trunk holds the implementation: the latest change at `HEAD` is an ancestor of the
    /// trunk, or the paths are identical on both.
    OnMaster {
        /// The latest commit touching the paths reachable from `HEAD`, when there is one.
        head: Option<String>,
        /// The latest commit touching them on the trunk.
        master: String,
    },
}

/// Whether a commit contains another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Containment {
    /// The ancestor is reachable from the descendant.
    Contains,
    /// Both are known, and it is not.
    DoesNotContain,
    /// This clone does not have one of the two commits, so the question has no answer here.
    CommitUnknown,
}

/// The paths a feature is implemented by, ordered and without repetition.
pub(crate) fn implementation_paths(
    root: &Path,
    feature: &ResolvedRefs,
    index: &Index,
) -> Vec<String> {
    let mut paths = BTreeSet::new();
    for module in &feature.module_refs {
        if !module.source_path.is_empty() {
            paths.insert(module.source_path.clone());
        }
    }
    for command in &feature.feature.commands {
        let lib = format!("lib/{command}.sh");
        if root.join(&lib).is_file() {
            paths.insert(lib);
        }
    }
    for claim in &feature.feature.claims {
        let declared = index
            .objects
            .iter()
            .find(|o| o.kind == "claim" && &o.identity == claim)
            .and_then(|o| o.metadata.get("implementation").and_then(Value::as_str));
        if let Some(path) = declared.map(str::trim) {
            if !path.is_empty() && path != "-" {
                paths.insert(path.to_string());
            }
        }
    }
    paths.into_iter().collect()
}

/// The remote-tracking ref of the trunk, `origin/<trunk>`, when this clone has it.
pub(crate) fn trunk_ref(root: &Path) -> Result<String, String> {
    // The remote decides its own default branch, and `origin/HEAD` is where it says so. The
    // repository's identity answers which branch the primary checkout holds, which is the
    // same branch in a clone of this repository and is not the same thing in general: a
    // checkout whose trunk is `main` against a remote whose default is `master` measured
    // every feature against a ref that does not exist, and reported "this clone has no
    // origin/main" for a repository whose whole history is on origin/master. So the remote
    // is asked first, the identity second, and each candidate must resolve to a commit.
    let mut tried: Vec<String> = Vec::new();
    if let Some(head) = symbolic_origin_head(root) {
        if resolves(root, &format!("refs/remotes/{head}")) {
            return Ok(head);
        }
        tried.push(head);
    }
    let identity = crate::worktree::RepositoryIdentity::discover(root)
        .map_err(|e| format!("the repository could not be read: {e}"))?;
    if let Some(branch) = identity.trunk().branch.clone() {
        let remote = format!("origin/{branch}");
        if resolves(root, &format!("refs/remotes/{remote}")) {
            return Ok(remote);
        }
        tried.push(remote);
    }
    // Neither the remote nor the identity named a ref this clone holds. One remote-tracking
    // branch and only one is not a guess: it is the only trunk this clone could mean. Two or
    // more, and which is the trunk is exactly what was not answered, so the verdict stays
    // unknown rather than picking a name that is usually right.
    let only = single_origin_branch(root);
    if let Some(remote) = only {
        return Ok(remote);
    }
    if tried.is_empty() {
        return Err("no trunk branch was found".to_string());
    }
    tried.dedup();
    Err(format!("this clone has no {}", tried.join(" and no ")))
}

/// The one remote-tracking branch of `origin`, when this clone has exactly one.
fn single_origin_branch(root: &Path) -> Option<String> {
    let out = read_only(root)
        .args([
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/remotes/origin",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut names = text
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty() && *n != "origin/HEAD");
    let first = names.next()?.to_string();
    names.next().is_none().then_some(first)
}

/// What `origin/HEAD` points at, as `origin/<branch>`: the remote's own default branch.
fn symbolic_origin_head(root: &Path) -> Option<String> {
    let out = read_only(root)
        .args(["symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!name.is_empty()).then_some(name)
}

fn resolves(root: &Path, rev: &str) -> bool {
    read_only(root)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The latest commit touching `paths` reachable from `rev`: `Ok(None)` when none does.
fn latest(root: &Path, rev: &str, paths: &[String]) -> Result<Option<String>, String> {
    let out = read_only(root)
        .args(["log", "-1", "--format=%H", rev, "--"])
        .args(paths)
        .output()
        .map_err(|e| format!("git could not run: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git log {rev}: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok((!sha.is_empty()).then_some(sha))
}

/// Are `paths` byte-identical between two revisions?
fn identical(root: &Path, a: &str, b: &str, paths: &[String]) -> bool {
    read_only(root)
        .args(["diff", "--quiet", a, b, "--"])
        .args(paths)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Where the implementation is, measured against `trunk` (`Err` carries why there is none).
pub(crate) fn locate(root: &Path, trunk: Result<&str, &str>, paths: &[String]) -> Revision {
    if paths.is_empty() {
        return Revision::NoImplementation;
    }
    let trunk = match trunk {
        Ok(t) => t,
        Err(reason) => {
            return Revision::Unknown {
                reason: reason.to_string(),
            }
        }
    };
    let (head, master) = match (latest(root, "HEAD", paths), latest(root, trunk, paths)) {
        (Ok(h), Ok(m)) => (h, m),
        (Err(reason), _) | (_, Err(reason)) => return Revision::Unknown { reason },
    };
    match (head, master) {
        (None, None) => Revision::NotFound,
        (None, Some(master)) => Revision::OnMaster { head: None, master },
        (Some(head), None) => Revision::OnBranch { head, master: None },
        (Some(head), Some(master)) => {
            if contains(root, trunk, &head) == Containment::Contains
                || identical(root, trunk, "HEAD", paths)
            {
                Revision::OnMaster {
                    head: Some(head),
                    master,
                }
            } else {
                Revision::OnBranch {
                    head,
                    master: Some(master),
                }
            }
        }
    }
}

/// Does `descendant` contain `ancestor`? A commit this clone lacks is neither yes nor no.
pub(crate) fn contains(root: &Path, descendant: &str, ancestor: &str) -> Containment {
    if !resolves(root, descendant) || !resolves(root, ancestor) {
        return Containment::CommitUnknown;
    }
    match crate::git::is_ancestor(root, ancestor, descendant) {
        Some(true) => Containment::Contains,
        Some(false) => Containment::DoesNotContain,
        None => Containment::CommitUnknown,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::process::Command;

    /// A repository with an `origin/master`, built by committing and moving the remote ref.
    pub(crate) struct Repo {
        pub dir: tempfile::TempDir,
    }

    impl Repo {
        pub(crate) fn new() -> Self {
            let r = Repo {
                dir: tempfile::tempdir().unwrap(),
            };
            r.git(&["init", "-q", "-b", "master", "."]);
            r.git(&["config", "user.email", "t@example.com"]);
            r.git(&["config", "user.name", "t"]);
            r.git(&["commit", "-q", "--allow-empty", "-m", "init"]);
            r
        }

        pub(crate) fn root(&self) -> &Path {
            self.dir.path()
        }

        pub(crate) fn git(&self, args: &[&str]) -> String {
            let out = Command::new("git")
                .arg("-C")
                .arg(self.root())
                .args(args)
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }

        pub(crate) fn commit(&self, path: &str, content: &str) -> String {
            let file = self.root().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, content).unwrap();
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "-m", path]);
            self.git(&["rev-parse", "HEAD"])
        }

        pub(crate) fn publish_master(&self) {
            self.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
        }
    }

    fn paths(p: &[&str]) -> Vec<String> {
        p.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn an_implementation_merged_to_the_trunk_is_on_master() {
        let r = Repo::new();
        let sha = r.commit("lib/a.sh", "a");
        r.publish_master();
        assert_eq!(trunk_ref(r.root()), Ok("origin/master".into()));
        let rev = locate(r.root(), Ok("origin/master"), &paths(&["lib/a.sh"]));
        assert_eq!(
            rev,
            Revision::OnMaster {
                head: Some(sha.clone()),
                master: sha
            }
        );
    }

    #[test]
    fn a_change_only_on_the_branch_is_implemented_on_the_branch() {
        let r = Repo::new();
        let first = r.commit("lib/a.sh", "a");
        r.publish_master();
        r.git(&["checkout", "-q", "-b", "feature/x"]);
        let second = r.commit("lib/a.sh", "b");
        let rev = locate(r.root(), Ok("origin/master"), &paths(&["lib/a.sh"]));
        assert_eq!(
            rev,
            Revision::OnBranch {
                head: second,
                master: Some(first)
            }
        );
        // a path the trunk never had is on the branch with nothing on the trunk
        r.commit("lib/new.sh", "n");
        assert!(matches!(
            locate(r.root(), Ok("origin/master"), &paths(&["lib/new.sh"])),
            Revision::OnBranch { master: None, .. }
        ));
    }

    #[test]
    fn identical_content_on_the_trunk_is_on_master_even_when_the_commit_is_not() {
        let r = Repo::new();
        r.commit("lib/a.sh", "a");
        r.git(&["checkout", "-q", "-b", "side"]);
        r.commit("lib/a.sh", "b");
        // the trunk receives the same bytes by another commit, as a squash merge gives it
        r.git(&["checkout", "-q", "master"]);
        let squash = r.commit("lib/a.sh", "b");
        r.publish_master();
        r.git(&["checkout", "-q", "side"]);
        assert!(matches!(
            locate(r.root(), Ok("origin/master"), &paths(&["lib/a.sh"])),
            Revision::OnMaster { master, .. } if master == squash
        ));
    }

    #[test]
    fn nothing_committed_or_nothing_named_is_not_implemented() {
        let r = Repo::new();
        r.publish_master();
        assert_eq!(
            locate(r.root(), Ok("origin/master"), &paths(&["lib/none.sh"])),
            Revision::NotFound
        );
        assert_eq!(
            locate(r.root(), Ok("origin/master"), &[]),
            Revision::NoImplementation
        );
    }

    #[test]
    fn a_head_behind_the_trunk_still_finds_the_trunk_revision() {
        let r = Repo::new();
        let base = r.git(&["rev-parse", "HEAD"]);
        let sha = r.commit("lib/a.sh", "a");
        r.publish_master();
        r.git(&["checkout", "-q", "-b", "old", &base]);
        assert_eq!(
            locate(r.root(), Ok("origin/master"), &paths(&["lib/a.sh"])),
            Revision::OnMaster {
                head: None,
                master: sha
            }
        );
    }

    #[test]
    fn a_clone_without_the_trunk_is_unknown_not_off_master() {
        let r = Repo::new();
        r.commit("lib/a.sh", "a");
        let trunk = trunk_ref(r.root());
        assert!(trunk.is_err(), "{trunk:?}");
        let reason = trunk.unwrap_err();
        assert!(matches!(
            locate(r.root(), Err(reason.as_str()), &paths(&["lib/a.sh"])),
            Revision::Unknown { .. }
        ));
        // a ref git cannot resolve is unknown too, not a branch
        assert!(matches!(
            locate(r.root(), Ok("origin/nowhere"), &paths(&["lib/a.sh"])),
            Revision::Unknown { .. }
        ));
        let plain = tempfile::tempdir().unwrap();
        assert!(trunk_ref(plain.path()).is_err());
    }

    #[test]
    fn containment_has_three_answers() {
        let r = Repo::new();
        let old = r.commit("a", "1");
        let new = r.commit("a", "2");
        assert_eq!(contains(r.root(), &new, &old), Containment::Contains);
        assert_eq!(contains(r.root(), &old, &new), Containment::DoesNotContain);
        assert_eq!(
            contains(r.root(), "0123456789abcdef0123456789abcdef01234567", &old),
            Containment::CommitUnknown
        );
    }
}
