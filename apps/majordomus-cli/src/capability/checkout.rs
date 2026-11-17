//! The state of a checkout, as one fingerprint: what a
//! [`CachePolicy::Checkout`](super::CachePolicy::Checkout) entry is kept under.
//!
//! A capability that reads git or the files of the work tree cannot be keyed by the registry
//! fingerprint alone, because that covers what is declared and nothing else: a commit, a
//! staged change, an edited test or a new tag moves its answer and leaves the registry as it
//! was. So the key also carries this fingerprint, taken again on every call, over exactly
//! the state such a capability reads:
//!
//! - the git control files the live view already watches (`crate::live::Stamp`): HEAD, the
//!   staging index, the reflog of HEAD, a merge or rebase in progress, `packed-refs`;
//! - every directory under the common `refs/`, and `FETCH_HEAD`, `config` and
//!   `info/exclude`: a ref is created, moved or deleted by a rename inside its directory,
//!   which moves that directory's modification time, so a new tag or a fetched branch is a
//!   new fingerprint without a ref being read;
//! - the paths of the work tree that differ from the staging index (`git diff-files`, which
//!   compares without refreshing the index, so a touched file is named too) and the
//!   untracked, not ignored paths (`git ls-files --others --exclude-standard`), asked at once,
//!   and the size and modification time of each of those paths, so that a second edit of a
//!   file that was already modified is a new fingerprint too. What is staged needs no git:
//!   it is a function of the index file and HEAD, which the control files already cover.
//!
//! What it does not see is said where the policy is declared: an ignored file, another
//! process, the network, the clock. A capability that reads any of those is not a
//! `Checkout` capability. When git cannot answer, there is no fingerprint and the call is
//! not cached.
//!
//! Taking it costs two git processes run side by side and a few hundred `stat`s, a couple of
//! tens of milliseconds, which is what a capability that spawns dozens of git processes
//! saves by having it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use crate::live::Stamp;

/// The salt this fingerprint is hashed under, so that it never collides with another
/// fingerprint computed over some of the same files.
const SALT: &str = "capability/checkout-state";

/// Files of the common git directory, beside `refs/`, whose change can move what git
/// answers: a fetch that names commits without moving a ref, the configuration, and the
/// repository's own exclusions, which decide what is untracked.
const COMMON_FILES: [&str; 3] = ["FETCH_HEAD", "config", "info/exclude"];

/// What one checkout's fingerprint is taken over, resolved once per root: the git
/// directories do not move while a checkout exists.
#[derive(Debug)]
struct Watched {
    stamp: Stamp,
    common: PathBuf,
}

/// The checkouts this process has fingerprinted, by root. `None` is a root git could not
/// resolve, remembered so that it is not asked again.
static WATCHED: LazyLock<Mutex<HashMap<PathBuf, Option<std::sync::Arc<Watched>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn watched(root: &Path) -> Option<std::sync::Arc<Watched>> {
    let mut map = WATCHED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(known) = map.get(root) {
        return known.clone();
    }
    let resolved = Stamp::watching(root).and_then(|stamp| {
        let common = common_dir(root)?;
        Some(std::sync::Arc::new(Watched { stamp, common }))
    });
    map.insert(root.to_path_buf(), resolved.clone());
    resolved
}

fn common_dir(root: &Path) -> Option<PathBuf> {
    let out = crate::git::read_only(root)
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!text.is_empty()).then(|| {
        let path = PathBuf::from(text);
        if path.is_absolute() {
            path
        } else {
            root.join(path)
        }
    })
}

/// The fingerprint of the checkout at `root`, or `None` when git cannot give one.
pub(crate) fn fingerprint(root: &Path) -> Option<String> {
    let watched = watched(root)?;
    let mut files: Vec<String> = COMMON_FILES
        .iter()
        .map(|f| watched.common.join(f).to_string_lossy().into_owned())
        .collect();
    directories(&watched.common.join("refs"), &mut files);
    let ask = |args: &[&str]| {
        crate::git::read_only(root)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| o.stdout)
    };
    let (modified, untracked) = std::thread::scope(|scope| {
        let untracked = scope.spawn(|| ask(&["ls-files", "-z", "--others", "--exclude-standard"]));
        let modified = ask(&["diff-files", "-z", "--name-only"]);
        (modified, untracked.join().ok().flatten())
    });
    let (modified, untracked) = (modified?, untracked?);
    let mut changed = paths(&modified);
    changed.extend(paths(&untracked));
    let parts = [
        watched.stamp.take(),
        crate::environment::cache::fingerprint_of(root, &files, &[SALT]),
        crate::policy::sha256_hex(&String::from_utf8_lossy(&modified)),
        crate::policy::sha256_hex(&String::from_utf8_lossy(&untracked)),
        crate::environment::cache::fingerprint_of(root, &changed, &[SALT]),
    ];
    Some(crate::policy::sha256_hex(&parts.join("\n")))
}

/// Every directory at and under `dir`, absolute, into `out`.
fn directories(dir: &Path, out: &mut Vec<String>) {
    out.push(dir.to_string_lossy().into_owned());
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            directories(&entry.path(), out);
        }
    }
}

/// The paths of a NUL-separated git listing.
fn paths(listing: &[u8]) -> Vec<String> {
    listing
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn repository() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let root = dir.path();
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.email", "t@example.com"]);
        git(root, &["config", "user.name", "t"]);
        std::fs::write(root.join("a.md"), "a").expect("a file");
        std::fs::write(root.join(".gitignore"), "ignored/\n").expect("an ignore file");
        git(root, &["add", "-A"]);
        git(root, &["commit", "-qm", "one"]);
        dir
    }

    /// Nothing moved, nothing changed; and every kind of move a `Checkout` capability can
    /// observe is a new fingerprint.
    #[test]
    fn every_move_a_checkout_capability_reads_is_a_new_fingerprint() {
        let dir = repository();
        let root = dir.path();
        let first = fingerprint(root).expect("a work tree has a fingerprint");
        assert_eq!(fingerprint(root).as_ref(), Some(&first), "nothing moved");

        // an edit to a tracked file
        std::fs::write(root.join("a.md"), "b").expect("an edit");
        let edited = fingerprint(root).expect("a fingerprint");
        assert_ne!(edited, first, "an edited tracked file");

        // a second edit to the same, already modified file: status names it as before
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(root.join("a.md"), "bc").expect("a second edit");
        let again = fingerprint(root).expect("a fingerprint");
        assert_ne!(again, edited, "a second edit of a modified file");

        // an untracked file
        std::fs::write(root.join("new.md"), "n").expect("an untracked file");
        let untracked = fingerprint(root).expect("a fingerprint");
        assert_ne!(untracked, again, "a new untracked file");

        // a deleted tracked file
        std::fs::remove_file(root.join("a.md")).expect("a deletion");
        let deleted = fingerprint(root).expect("a fingerprint");
        assert_ne!(deleted, untracked, "a deleted tracked file");

        // staging
        git(root, &["add", "-A"]);
        let staged = fingerprint(root).expect("a fingerprint");
        assert_ne!(staged, deleted, "staging moves the index");

        // a commit
        git(root, &["commit", "-qm", "two"]);
        let committed = fingerprint(root).expect("a fingerprint");
        assert_ne!(committed, staged, "a commit");

        // a tag and a branch, which move no file of the work tree and not HEAD
        git(root, &["tag", "v1.0.0"]);
        let tagged = fingerprint(root).expect("a fingerprint");
        assert_ne!(tagged, committed, "a new tag");
        git(root, &["branch", "nested/side"]);
        let branched = fingerprint(root).expect("a fingerprint");
        assert_ne!(branched, tagged, "a new branch in a nested ref directory");
        git(root, &["commit", "-q", "--allow-empty", "-m", "three"]);
        let before_move = fingerprint(root).expect("a fingerprint");
        git(root, &["branch", "-f", "nested/side", "HEAD"]);
        assert_ne!(
            fingerprint(root).expect("a fingerprint"),
            before_move,
            "an existing branch moved"
        );
    }

    /// An ignored file is not part of the state: that is the boundary the policy declares.
    #[test]
    fn an_ignored_file_is_outside_the_fingerprint() {
        let dir = repository();
        let root = dir.path();
        let first = fingerprint(root).expect("a fingerprint");
        std::fs::create_dir_all(root.join("ignored")).expect("a directory");
        std::fs::write(root.join("ignored/x"), "x").expect("an ignored file");
        assert_eq!(fingerprint(root), Some(first));
    }

    /// A directory git knows nothing about has no fingerprint, so nothing of it is cached.
    #[test]
    fn a_directory_that_is_not_a_work_tree_has_no_fingerprint() {
        let plain = tempfile::tempdir().expect("a temporary directory");
        assert_eq!(fingerprint(plain.path()), None);
    }

    #[test]
    fn a_listing_is_read_path_by_path() {
        assert_eq!(paths(b"a b.md\0new file.md\0\0"), ["a b.md", "new file.md"]);
        assert!(paths(b"").is_empty());
    }
}
