//! The readings `worktree cleanup --remove` adds before it removes a merged worktree, and the
//! locks it holds while it does (I3832).
//!
//! `cleanup` already refused a worktree whose branch reaches no remote, whose tree is dirty or
//! that a process works inside. On 2026-10-20 and 21 that was not enough on a machine where a
//! dozen sessions build at once: a worktree at the trunk's tip was a session's verification
//! tree, a merged worktree was the one a queued derive was about to take, a build directory
//! with no open file belonged to a cargo that had only just started, and a worktree a session
//! had left minutes ago was removed under it. Each of those is one reading here, taken at the
//! moment of removal; and the removal itself holds every cargo lock in the worktree's build
//! output, so a build that starts while the worktree goes either waits or finds no directory.
//!
//! ```
//! use majordomus_cli::worktree::sweep::{named_by_derive_lock, named_in};
//! assert!(named_by_derive_lock("derive:fix/x 4242", "fix/x"));
//! assert!(!named_by_derive_lock("derive:fix/x-and-more 4242", "fix/x"));
//! let ps = "cargo build CARGO_BUILD_BUILD_DIR=/w/fix-x/apps/majordomus-cli/target\nsleep 9\n";
//! assert!(named_in(ps, std::path::Path::new("/w/fix-x")));
//! assert!(!named_in(ps, std::path::Path::new("/w/fix")));
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::disk::LOCK_FILES;

/// Does the derive lock's owner line name `branch`? The line reads `<who>:<branch> <pid>`
/// (lib/machine_lock.sh), and only the whole branch name counts.
pub fn named_by_derive_lock(owner_line: &str, branch: &str) -> bool {
    owner_line
        .split_whitespace()
        .next()
        .and_then(|who_branch| who_branch.split_once(':'))
        .is_some_and(|(_, b)| b == branch)
}

/// The derive lock's owner line, when a lock is held: `$MAJORDOMUS_DERIVE_LOCK/owner`, else
/// `.derive.lock/owner` beside the repository's checkouts, as lib/machine_lock.sh places it.
pub fn derive_lock_owner(primary: &Path) -> Option<String> {
    let dir = std::env::var_os("MAJORDOMUS_DERIVE_LOCK")
        .map(PathBuf::from)
        .or_else(|| primary.parent().map(|p| p.join(".derive.lock")))?;
    fs::read_to_string(dir.join("owner"))
        .ok()
        .map(|s| s.trim().to_string())
}

/// Does any line of a process listing (command lines, with their environment) name `dir`
/// or a path under it? A `CARGO_BUILD_BUILD_DIR` or `CARGO_TARGET_DIR` pinned into a
/// worktree is such a name, and so is a command started on a file inside it. A word is
/// compared as written and, when it names a path that exists, as that path resolved: on
/// macOS `/var/folders/...` and `/private/var/folders/...` are one directory, and a process
/// started through one spelling names the worktree the topology knows by the other.
pub fn named_in(listing: &str, dir: &Path) -> bool {
    let raw = dir.to_string_lossy().trim_end_matches('/').to_string();
    let canon = fs::canonicalize(dir)
        .ok()
        .map(|c| c.to_string_lossy().trim_end_matches('/').to_string());
    let under = |w: &str, d: &str| w == d || w.starts_with(&format!("{d}/"));
    // only a word that holds the worktree's own name can resolve to a path under it, so
    // only those are resolved: a listing with every environment carries thousands of paths
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    listing.lines().any(|l| {
        l.split(|c: char| c.is_whitespace() || c == '=' || c == ':')
            .filter(|w| w.starts_with('/'))
            .any(|w| {
                under(w, &raw)
                    || canon.as_deref().is_some_and(|c| {
                        under(w, c)
                            || (!name.is_empty()
                                && w.contains(name.as_str())
                                && resolved(w).is_some_and(|r| under(&r, c)))
                    })
            })
    })
}

/// `word` resolved through the longest prefix of it that exists, the rest appended as
/// written; `None` when nothing of it exists.
fn resolved(word: &str) -> Option<String> {
    let mut p = Path::new(word);
    let mut rest: Vec<&std::ffi::OsStr> = Vec::new();
    loop {
        if let Ok(c) = fs::canonicalize(p) {
            let mut out = c;
            for r in rest.iter().rev() {
                out.push(r);
            }
            return Some(out.to_string_lossy().trim_end_matches('/').to_string());
        }
        rest.push(p.file_name()?);
        p = p.parent()?;
    }
}

/// Does a live mesh claim name this worktree? Its issue's slug is the branch's last
/// component (`feature/<slug>`, the plan's naming), or a scope entry or its intent names the
/// branch or the worktree's path. A claim is how a session on any machine says it is working
/// on something, and the branch it works on is the one place the claim and the worktree meet.
///
/// ```
/// use majordomus_cli::worktree::sweep::claim_names;
/// let wt = std::path::Path::new("/w/feature/x");
/// assert!(claim_names(&[], None, Some("x"), "feature/x", wt));
/// assert!(claim_names(&[], Some("finishing feature/x"), None, "feature/x", wt));
/// assert!(claim_names(&["/w/feature/x/src".into()], None, None, "feature/x", wt));
/// assert!(!claim_names(&["apps/cli".into()], Some("feature/xy"), Some("y"), "feature/x", wt));
/// ```
pub fn claim_names(
    scope: &[String],
    intent: Option<&str>,
    issue_slug: Option<&str>,
    branch: &str,
    dir: &Path,
) -> bool {
    let last = branch.rsplit('/').next().unwrap_or(branch);
    if issue_slug.is_some_and(|s| s == last || s == branch) {
        return true;
    }
    // a word of the text that is the branch, or the worktree's path or a path under it
    let names = |text: &str| {
        text.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '(' | ')' | '`' | '"'))
            .map(|w| w.trim_end_matches(['.', ':']))
            .any(|w| w == branch || (w.starts_with('/') && named_in(w, dir)))
    };
    scope.iter().any(|s| names(s)) || intent.is_some_and(names)
}

/// The slug the plan gives issue `id`, read from `<primary>/.ai/repo/project/issues/<id>.yaml`.
pub fn issue_slug(primary: &Path, id: &str) -> Option<String> {
    if !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let text =
        fs::read_to_string(primary.join(format!(".ai/repo/project/issues/{id}.yaml"))).ok()?;
    let m = crate::metadata::yaml::parse_mapping(&text).ok()?;
    m.get("slug")?.as_str().map(str::to_string)
}

/// Every running process's command line and environment, one per line: `ps -Eww` on macOS,
/// `ps axeww` elsewhere. `None` when `ps` cannot be run, which the caller reads as "cannot
/// tell", never as "nobody".
pub fn process_listing() -> Option<String> {
    let mut cmd = std::process::Command::new("ps");
    if cfg!(target_os = "macos") {
        cmd.args(["-E", "-ww", "-ax", "-o", "command"]);
    } else {
        cmd.args(["axeww", "-o", "args"]);
    }
    let out = cmd.output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Did anything in `dir` change within `minutes`? `.git` is not read (git rewrites its own
/// files on a status). `None` when a directory cannot be read: a walk with a hole is no proof
/// of idleness.
pub fn changed_within(dir: &Path, minutes: u64) -> Option<bool> {
    let since = SystemTime::now() - Duration::from_secs(minutes.saturating_mul(60));
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).ok()? {
            let e = e.ok()?;
            if e.file_name() == ".git" {
                continue;
            }
            let m = fs::symlink_metadata(e.path()).ok()?;
            if m.modified().is_ok_and(|t| t > since) {
                return Some(true);
            }
            if m.is_dir() {
                stack.push(e.path());
            }
        }
    }
    Some(false)
}

/// Every cargo lock file in the worktree's build output: the `target` directories it holds,
/// at any depth outside `.git`.
pub fn build_locks(dir: &Path) -> Vec<PathBuf> {
    let mut locks = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), false)];
    while let Some((d, in_target)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name();
            if name == ".git" {
                continue;
            }
            let Ok(m) = fs::symlink_metadata(e.path()) else {
                continue;
            };
            if m.is_dir() {
                stack.push((e.path(), in_target || name == "target"));
            } else if in_target && name.to_str().is_some_and(|n| LOCK_FILES.contains(&n)) {
                locks.push(e.path());
            }
        }
    }
    locks.sort();
    locks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::unix::io::AsRawFd;

    #[test]
    fn the_derive_lock_names_a_branch_only_whole() {
        assert!(named_by_derive_lock("derive:feature/a 1", "feature/a"));
        assert!(!named_by_derive_lock("derive:feature/a-b 1", "feature/a"));
        assert!(!named_by_derive_lock("", "feature/a"));
        assert!(!named_by_derive_lock("garbage", "feature/a"));
    }

    #[test]
    fn a_pinned_build_dir_names_the_worktree_and_a_sibling_does_not() {
        let ps = "/bin/cargo test CARGO_TARGET_DIR=/w/a/target HOME=/h\n";
        assert!(named_in(ps, Path::new("/w/a")));
        assert!(named_in(ps, Path::new("/w/a/")));
        assert!(!named_in(ps, Path::new("/w/ab")));
    }

    #[test]
    fn a_path_spelled_through_a_symlink_names_the_worktree_it_resolves_to() {
        let t = tempfile::tempdir().unwrap();
        let real = t.path().join("real");
        fs::create_dir_all(real.join("wt/target")).unwrap();
        std::os::unix::fs::symlink(&real, t.path().join("link")).unwrap();
        let ps = format!(
            "sleep 9 CARGO_TARGET_DIR={}/link/wt/target/debug\n",
            t.path().display()
        );
        assert!(named_in(&ps, &real.join("wt")));
        assert!(!named_in(&ps, &real.join("other")));
    }

    #[test]
    fn a_fresh_tree_has_changed_and_an_aged_one_has_not() {
        let t = tempfile::tempdir().unwrap();
        fs::write(t.path().join("a"), b"x").unwrap();
        assert_eq!(changed_within(t.path(), 30), Some(true));
        let then = SystemTime::now() - Duration::from_secs(7200);
        File::open(t.path().join("a"))
            .unwrap()
            .set_modified(then)
            .unwrap();
        assert_eq!(changed_within(t.path(), 30), Some(false));
        // .git is not read
        fs::create_dir(t.path().join(".git")).unwrap();
        fs::write(t.path().join(".git/index"), b"x").unwrap();
        assert_eq!(changed_within(t.path(), 30), Some(false));
    }

    #[test]
    fn the_build_locks_are_found_under_target_only_and_can_be_held() {
        let t = tempfile::tempdir().unwrap();
        let tgt = t.path().join("apps/x/target/debug");
        fs::create_dir_all(&tgt).unwrap();
        fs::write(tgt.join(".cargo-lock"), b"").unwrap();
        fs::write(t.path().join(".cargo-lock"), b"").unwrap(); // not build output
        let locks = build_locks(t.path());
        assert_eq!(locks, vec![tgt.join(".cargo-lock")]);
        let held = crate::disk::lock_all(&locks).unwrap();
        // a second taker is refused while they are held
        let other = File::options()
            .read(true)
            .write(true)
            .open(&locks[0])
            .unwrap();
        // SAFETY: the test owns `other`
        let rc = unsafe { libc::flock(other.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        assert_ne!(rc, 0, "a held lock was taken twice");
        drop(held);
    }
}
