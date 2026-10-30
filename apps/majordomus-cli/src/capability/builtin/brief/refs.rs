//! Git refs read from files, without a process: the brief is composed on every status-line
//! tick, and a ref is one small file (or one line of `packed-refs`).
//!
//! A linked worktree's `.git` is a file naming its own git directory, which holds `HEAD`;
//! the refs themselves live in the common directory that `commondir` names. A ref that is
//! neither loose nor packed is unknown, never guessed.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

/// When this clone last fetched `origin/<base>`: the newest of the `FETCH_HEAD` files that
/// name that branch (a linked worktree's own, in its git directory, and the common one) and
/// the last entry of the remote-tracking ref's reflog. `FETCH_HEAD` alone was wrong both
/// ways (2026-10-30): a fetch of `master` in a linked worktree wrote the worktree's
/// `FETCH_HEAD` and left the common one old, and a fetch of `gh-pages` rewrote the common one
/// although `origin/master` had not moved. None when no source names the branch.
pub(crate) fn fetched_instant(root: &Path, base: &str) -> Option<SystemTime> {
    let gd = git_dir(root)?;
    let common = common_dir(&gd);
    let marker = format!("branch '{base}' of");
    let head = |dir: &Path| -> Option<SystemTime> {
        let p = dir.join("FETCH_HEAD");
        let text = std::fs::read_to_string(&p).ok()?;
        if !text.lines().any(|l| l.contains(&marker)) {
            return None;
        }
        std::fs::metadata(&p).ok()?.modified().ok()
    };
    let logged = || -> Option<SystemTime> {
        let text =
            std::fs::read_to_string(common.join("logs/refs/remotes/origin").join(base)).ok()?;
        reflog_time(text.lines().rev().find(|l| !l.trim().is_empty())?)
    };
    [head(&gd), head(&common), logged()]
        .into_iter()
        .flatten()
        .max()
}

/// The time of one reflog line: `<old> <new> <name> <<email>> <epoch> <zone>\t<message>`.
pub(crate) fn reflog_time(line: &str) -> Option<SystemTime> {
    let (head, _) = line.split_once('\t').unwrap_or((line, ""));
    let mut words = head.rsplit(' ');
    let _zone = words.next()?;
    let epoch: u64 = words.next()?.parse().ok()?;
    Some(UNIX_EPOCH + std::time::Duration::from_secs(epoch))
}

/// The newest reflog entry of `refs/remotes/origin/<branch>` that moved it to `sha`, with its
/// time: when this clone's remote-tracking ref took that head, by a push from here or a fetch.
pub(crate) fn took_head_at(root: &Path, branch: &str, sha: &str) -> Option<SystemTime> {
    let common = common_dir(&git_dir(root)?);
    let text =
        std::fs::read_to_string(common.join("logs/refs/remotes/origin").join(branch)).ok()?;
    text.lines()
        .rev()
        .find(|l| l.split(' ').nth(1) == Some(sha))
        .and_then(reflog_time)
}

/// How long ago this clone last fetched `origin/<base>` ([`fetched_instant`]).
pub(crate) fn fetched_seconds_ago(root: &Path, base: &str) -> Option<u64> {
    fetched_instant(root, base)?
        .elapsed()
        .ok()
        .map(|d| d.as_secs())
}

/// When this clone last fetched `origin/<base>`, RFC 3339.
pub(crate) fn fetched_at(root: &Path, base: &str) -> Option<String> {
    fetched_instant(root, base).map(crate::peers::rfc3339)
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
        assert_eq!(
            fetched_seconds_ago(&root, "master"),
            None,
            "nothing was ever fetched"
        );
    }

    /// The fetch age is the branch's own: a `FETCH_HEAD` that names another branch is not it,
    /// a linked worktree's own `FETCH_HEAD` is, and the remote-tracking ref's reflog is.
    #[test]
    fn the_fetch_age_is_the_branchs_own() {
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
        let gd = git_dir(&root).unwrap();
        std::fs::write(
            gd.join("FETCH_HEAD"),
            "abc\t\tbranch 'gh-pages' of github.com:o/r\n",
        )
        .unwrap();
        assert_eq!(
            fetched_instant(&root, "master"),
            None,
            "a fetch of gh-pages is not a fetch of master"
        );
        std::fs::write(
            gd.join("FETCH_HEAD"),
            "abc\t\tbranch 'master' of github.com:o/r\n",
        )
        .unwrap();
        assert!(fetched_seconds_ago(&root, "master").is_some_and(|a| a < 60));
        let logs = gd.join("logs/refs/remotes/origin");
        std::fs::create_dir_all(&logs).unwrap();
        let old = "0000000000000000000000000000000000000000";
        let new = "1111111111111111111111111111111111111111";
        std::fs::write(
            logs.join("fix-x"),
            format!("{old} {new} T <t@t> 1761800000 +0100\tupdate by push\n"),
        )
        .unwrap();
        assert_eq!(
            took_head_at(&root, "fix-x", new),
            Some(UNIX_EPOCH + std::time::Duration::from_secs(1_761_800_000))
        );
        assert_eq!(took_head_at(&root, "fix-x", old), None);
        assert_eq!(
            reflog_time(&format!("{old} {new} A B <a@b> 12 -0700\tfetch")),
            Some(UNIX_EPOCH + std::time::Duration::from_secs(12))
        );
    }
}
