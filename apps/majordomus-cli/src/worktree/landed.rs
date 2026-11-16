//! Whether a worktree's work is already in the trunk, by what git can prove offline.
//!
//! `git branch --merged` answers one question: is the branch's tip an ancestor of the trunk.
//! A branch squash-merged on the forge, carried in a train, or cherry-picked is never an
//! ancestor, and a worktree whose only uncommitted changes are records that landed through
//! another branch is never clean, so neither is offered by `worktree cleanup`. On 2026-11-16
//! that left 205 worktrees (about 44 GiB) on this machine whose every byte was already on
//! the trunk; they were found with the two readings below and removed by hand.
//!
//! Both readings are exact, never heuristic:
//!
//! - [`content_in`]: merging the branch into the trunk would change nothing. `git merge-tree
//!   --write-tree <trunk> <rev>` writes the merged tree without touching any work tree or
//!   ref; when that tree is the trunk's own tree, the branch adds nothing. A branch with one
//!   line the trunk lacks writes a different tree, and a conflict is a "no".
//! - [`uncommitted_in`]: every path `git status` reports holds, in the work tree, exactly
//!   what the trunk holds at that path (the same blob and mode, or absent in both).
//!
//! Neither fetches, and neither writes: `merge-tree --write-tree` adds the merged tree's
//! objects to the object store, as any `git merge` would, and moves no ref.
//!
//! ```
//! use majordomus_cli::worktree::landed::{content_in, uncommitted_in};
//! let dir = tempfile::tempdir().unwrap();
//! let run = |args: &[&str]| {
//!     let ok = std::process::Command::new("git")
//!         .args(["-c", "user.email=t@t", "-c", "user.name=t"])
//!         .args(args)
//!         .current_dir(dir.path())
//!         .env_remove("GIT_DIR")
//!         .env_remove("GIT_WORK_TREE")
//!         .status()
//!         .unwrap()
//!         .success();
//!     assert!(ok, "git {args:?}");
//! };
//! run(&["init", "-q", "-b", "main"]);
//! run(&["commit", "-q", "--allow-empty", "-m", "base"]);
//! run(&["switch", "-q", "-c", "squashed"]);
//! std::fs::write(dir.path().join("a.txt"), "a\n").unwrap();
//! run(&["add", "a.txt"]);
//! run(&["commit", "-q", "-m", "one"]);
//! run(&["switch", "-q", "main"]);
//! run(&["merge", "-q", "--squash", "squashed"]);
//! run(&["commit", "-q", "-m", "squash"]);
//! // not an ancestor of main, and still entirely in it
//! assert_eq!(content_in(dir.path(), "squashed", "main"), Some(true));
//! run(&["switch", "-q", "squashed"]);
//! std::fs::write(dir.path().join("b.txt"), "b\n").unwrap();
//! assert_eq!(uncommitted_in(dir.path(), "main"), Some(false), "b.txt is not in main");
//! std::fs::remove_file(dir.path().join("b.txt")).unwrap();
//! assert_eq!(uncommitted_in(dir.path(), "main"), Some(true), "nothing uncommitted is left");
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use super::git;

/// The ref the trunk is judged by: its remote-tracking branch when it has one (what every
/// other checkout and the forge see), the local trunk otherwise.
///
/// ```
/// use majordomus_cli::worktree::landed::trunk_reference;
/// let dir = tempfile::tempdir().unwrap();
/// // no repository, no upstream: the name given is the answer
/// assert_eq!(trunk_reference(dir.path(), "main"), "main");
/// ```
pub fn trunk_reference(primary: &Path, trunk: &str) -> String {
    let upstream = format!("{trunk}@{{upstream}}");
    if let Ok(out) = git::try_run(
        primary,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            &upstream,
        ],
    ) {
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status == Some(0) && !name.is_empty() {
            return name;
        }
    }
    trunk.to_string()
}

/// Would merging `rev` into `trunk` change nothing? `Some(true)` when the merge writes the
/// trunk's own tree, `Some(false)` when it writes another tree or conflicts, `None` when git
/// cannot say (an unknown revision, or a git older than 2.38 without `merge-tree
/// --write-tree`). `cwd` is any work tree of the repository.
///
/// ```
/// use majordomus_cli::worktree::landed::content_in;
/// let dir = tempfile::tempdir().unwrap();
/// // not a repository: git cannot say, and that is not a "yes"
/// assert_eq!(content_in(dir.path(), "HEAD", "main"), None);
/// ```
pub fn content_in(cwd: &Path, rev: &str, trunk: &str) -> Option<bool> {
    let tree = format!("{trunk}^{{tree}}");
    let tree = git::try_run(
        cwd,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &tree,
        ],
    )
    .ok()
    .filter(|o| o.status == Some(0))?;
    let tree = String::from_utf8_lossy(&tree.stdout).trim().to_string();
    let merged = git::try_run(cwd, &["merge-tree", "--write-tree", trunk, rev]).ok()?;
    match merged.status {
        Some(0) => {
            let written = String::from_utf8_lossy(&merged.stdout);
            Some(written.lines().next().map(str::trim) == Some(tree.as_str()))
        }
        // a conflict: the branch holds something the trunk does not, whatever else it holds
        Some(1) => Some(false),
        _ => None,
    }
}

/// Does every uncommitted change in this work tree already hold what `rev` holds at that
/// path? `Some(true)` for a clean work tree too. `Some(false)` when any path differs, when a
/// conflict is unresolved or an operation (merge, rebase, ...) is in progress, and for a
/// path that is a symbolic link or a directory git reports whole (a nested repository):
/// those are never judged equal. `None` when git or the file system cannot be read.
///
/// ```
/// use majordomus_cli::worktree::landed::uncommitted_in;
/// let dir = tempfile::tempdir().unwrap();
/// assert_eq!(uncommitted_in(dir.path(), "main"), None, "not a repository");
/// ```
pub fn uncommitted_in(worktree: &Path, rev: &str) -> Option<bool> {
    if super::state::operation_in_progress(worktree).is_some() {
        return Some(false);
    }
    let status = git::status_porcelain_z(worktree, "all").ok()?;
    let Some(paths) = changed_paths(&status) else {
        return Some(false); // an unresolved conflict is work nobody has finished
    };
    if paths.is_empty() {
        return Some(true);
    }
    let blobs = blobs_of(worktree, rev)?;
    let mut to_hash: Vec<(&str, &str)> = Vec::new();
    for p in &paths {
        match std::fs::symlink_metadata(worktree.join(p)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if blobs.contains_key(p.as_str()) {
                    return Some(false); // deleted here, present in the trunk
                }
            }
            Err(_) => return None,
            Ok(m) if m.file_type().is_file() => {
                let Some((mode, blob)) = blobs.get(p.as_str()) else {
                    return Some(false); // present here, absent in the trunk
                };
                if *mode != file_mode(&m) {
                    return Some(false);
                }
                to_hash.push((p.as_str(), blob.as_str()));
            }
            Ok(_) => return Some(false),
        }
    }
    // `hash-object` applies the path's filters (eol, clean) as `git add` would
    for chunk in to_hash.chunks(256) {
        let mut args: Vec<&str> = vec!["hash-object", "--"];
        args.extend(chunk.iter().map(|(p, _)| *p));
        let out = git::try_run(worktree, &args)
            .ok()
            .filter(|o| o.status == Some(0))?;
        let got = String::from_utf8_lossy(&out.stdout);
        let got: Vec<&str> = got.lines().collect();
        if got.len() != chunk.len() {
            return None;
        }
        if chunk.iter().zip(got).any(|((_, want), got)| *want != got) {
            return Some(false);
        }
    }
    Some(true)
}

/// Every path `git status --porcelain -z` names, both sides of a rename or copy included.
/// `None` when an entry is an unresolved conflict, or a path is not UTF-8.
fn changed_paths(status: &[u8]) -> Option<Vec<String>> {
    let mut paths = Vec::new();
    let mut entries = status.split(|b| *b == 0).filter(|e| !e.is_empty());
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            return None;
        }
        let (x, y) = (entry[0] as char, entry[1] as char);
        match (x, y) {
            ('!', '!') => continue,
            ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D') => return None,
            _ => {}
        }
        paths.push(String::from_utf8(entry[3..].to_vec()).ok()?);
        if matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C') {
            let from = entries.next()?;
            paths.push(String::from_utf8(from.to_vec()).ok()?);
        }
    }
    Some(paths)
}

/// Every blob of `rev`'s tree: path to (mode, object id). Submodules keep their gitlink
/// mode, so a file at such a path never matches.
fn blobs_of(cwd: &Path, rev: &str) -> Option<BTreeMap<String, (String, String)>> {
    let out = git::try_run(cwd, &["ls-tree", "-r", "-z", "--end-of-options", rev])
        .ok()
        .filter(|o| o.status == Some(0))?;
    let mut blobs = BTreeMap::new();
    for entry in out.stdout.split(|b| *b == 0).filter(|e| !e.is_empty()) {
        let entry = std::str::from_utf8(entry).ok()?;
        let (meta, path) = entry.split_once('\t')?;
        let mut meta = meta.split(' ');
        let (mode, _kind, id) = (meta.next()?, meta.next()?, meta.next()?);
        blobs.insert(path.to_string(), (mode.to_string(), id.to_string()));
    }
    Some(blobs)
}

/// The mode git would record for a regular file.
#[cfg(unix)]
fn file_mode(m: &std::fs::Metadata) -> &'static str {
    use std::os::unix::fs::PermissionsExt;
    if m.permissions().mode() & 0o111 != 0 {
        "100755"
    } else {
        "100644"
    }
}

#[cfg(not(unix))]
fn file_mode(_m: &std::fs::Metadata) -> &'static str {
    "100644"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rename_names_both_sides_and_a_conflict_answers_nothing() {
        let status = b"R  new.txt\0old.txt\0?? fresh.txt\0";
        assert_eq!(
            changed_paths(status),
            Some(vec!["new.txt".into(), "old.txt".into(), "fresh.txt".into()])
        );
        assert_eq!(changed_paths(b"UU both.txt\0"), None);
        assert_eq!(changed_paths(b"!! ignored/\0"), Some(vec![]));
    }
}
