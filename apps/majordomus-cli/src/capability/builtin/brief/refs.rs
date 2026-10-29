//! Git refs read from files, without a process: the brief is composed on every status-line
//! tick, and a ref is one small file (or one line of `packed-refs`).
//!
//! A linked worktree's `.git` is a file naming its own git directory, which holds `HEAD`;
//! the refs themselves live in the common directory that `commondir` names. A ref that is
//! neither loose nor packed is unknown, never guessed.

use std::path::{Path, PathBuf};

/// The git directory of the checkout at `root`: `.git` itself, or the directory a linked
/// worktree's `.git` file names.
pub(crate) fn git_dir(root: &Path) -> Option<PathBuf> {
    let dot = root.join(".git");
    if dot.is_dir() {
        return Some(dot);
    }
    let text = std::fs::read_to_string(&dot).ok()?;
    let named = text
        .lines()
        .find_map(|l| l.strip_prefix("gitdir: "))?
        .trim();
    let p = PathBuf::from(named);
    Some(if p.is_absolute() { p } else { root.join(p) })
}

/// The common git directory: where refs, `packed-refs` and `FETCH_HEAD` of every worktree
/// of the repository live.
pub(crate) fn common_dir(git_dir: &Path) -> PathBuf {
    match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(t) => {
            let p = PathBuf::from(t.trim());
            if p.is_absolute() {
                p
            } else {
                git_dir.join(p)
            }
        }
        Err(_) => git_dir.to_path_buf(),
    }
}

/// The commit a full ref name (`refs/remotes/origin/master`) points at, loose or packed.
pub(crate) fn resolve(common: &Path, name: &str) -> Option<String> {
    if let Ok(t) = std::fs::read_to_string(common.join(name)) {
        let t = t.trim();
        if let Some(target) = t.strip_prefix("ref: ") {
            return resolve(common, target.trim());
        }
        if is_sha(t) {
            return Some(t.to_string());
        }
    }
    let packed = std::fs::read_to_string(common.join("packed-refs")).ok()?;
    packed.lines().find_map(|l| {
        let (sha, n) = l.split_once(' ')?;
        (n.trim() == name && is_sha(sha)).then(|| sha.to_string())
    })
}

/// The branch and head of the checkout at `root`: `(Some(branch), head)`, or `(None, head)`
/// when it is detached.
pub(crate) fn head(root: &Path) -> Option<(Option<String>, Option<String>)> {
    let gd = git_dir(root)?;
    let text = std::fs::read_to_string(gd.join("HEAD")).ok()?;
    let text = text.trim();
    let common = common_dir(&gd);
    match text.strip_prefix("ref: ") {
        Some(r) => {
            let r = r.trim();
            let branch = r.strip_prefix("refs/heads/").map(str::to_string);
            // the branch's own ref is the worktree's when it is per-worktree, else common
            let sha = resolve(&gd, r).or_else(|| resolve(&common, r));
            Some((branch, sha))
        }
        None => Some((None, is_sha(text).then(|| text.to_string()))),
    }
}

/// How long ago `FETCH_HEAD` was written: when this clone last fetched anything.
pub(crate) fn fetched_seconds_ago(root: &Path) -> Option<u64> {
    let common = common_dir(&git_dir(root)?);
    std::fs::metadata(common.join("FETCH_HEAD"))
        .ok()?
        .modified()
        .ok()?
        .elapsed()
        .ok()
        .map(|d| d.as_secs())
}

/// When `FETCH_HEAD` was written, RFC 3339.
pub(crate) fn fetched_at(root: &Path) -> Option<String> {
    let common = common_dir(&git_dir(root)?);
    let t = std::fs::metadata(common.join("FETCH_HEAD"))
        .ok()?
        .modified()
        .ok()?;
    Some(crate::peers::rfc3339(t))
}

fn is_sha(s: &str) -> bool {
    s.len() >= 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// The files answer what `git rev-parse` answers, loose and packed, in the primary
    /// checkout and in a linked worktree.
    #[test]
    fn the_files_answer_what_git_answers() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let root = dir.path().join("repo");
        std::fs::create_dir(&root).unwrap();
        git(&root, &["init", "-q", "-b", "master"]);
        git(
            &root,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "one",
            ],
        );
        let one = git(&root, &["rev-parse", "HEAD"]);
        git(&root, &["update-ref", "refs/remotes/origin/master", &one]);
        assert_eq!(
            head(&root),
            Some((Some("master".into()), Some(one.clone())))
        );
        let common = common_dir(&git_dir(&root).unwrap());
        assert_eq!(
            resolve(&common, "refs/remotes/origin/master"),
            Some(one.clone())
        );
        git(&root, &["pack-refs", "--all"]);
        assert_eq!(
            resolve(&common, "refs/remotes/origin/master"),
            Some(one.clone()),
            "packed"
        );
        assert_eq!(resolve(&common, "refs/remotes/origin/nothing"), None);

        let wt = dir.path().join("wt");
        git(
            &root,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feature/x",
                wt.to_str().unwrap(),
            ],
        );
        assert_eq!(
            head(&wt),
            Some((Some("feature/x".into()), Some(one.clone())))
        );
        let wcommon = common_dir(&git_dir(&wt).unwrap());
        assert_eq!(resolve(&wcommon, "refs/remotes/origin/master"), Some(one));
        assert_eq!(fetched_seconds_ago(&root), None, "nothing was ever fetched");
    }
}
