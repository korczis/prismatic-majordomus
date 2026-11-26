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
    git::try_run(
        primary,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            &upstream,
        ],
    )
    .ok()
    .filter(|out| out.status == Some(0))
    .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
    .filter(|name| !name.is_empty())
    .unwrap_or_else(|| trunk.to_string())
}

/// Would merging `rev` into `trunk` change nothing? `Some(true)` when the merge writes the
/// trunk's own tree, `Some(false)` when it writes another tree, conflicts, or cannot be
/// written at all (a revision git cannot merge, a git older than 2.38 without `merge-tree
/// --write-tree`): a "no" every time, never a "yes". `None` when the trunk itself cannot be
/// resolved or git cannot be run. `cwd` is any work tree of the repository.
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
    // exit 0 is a written merge; 1 a conflict, which means the branch holds something the
    // trunk does not, or a revision git cannot merge; anything else a git that cannot merge
    git::try_run(cwd, &["merge-tree", "--write-tree", trunk, rev])
        .ok()
        .map(|merged| {
            merged.status == Some(0)
                && String::from_utf8_lossy(&merged.stdout)
                    .lines()
                    .next()
                    .map(str::trim)
                    == Some(tree.as_str())
        })
}

/// Does every uncommitted change in this work tree already hold what `rev` holds at that
/// path, in the index and in the work tree? `Some(true)` for a clean work tree too.
/// `Some(false)` when any path differs (a staged version the trunk lacks included), when a
/// conflict is unresolved or an operation (merge, rebase, ...) is in progress, for a path
/// that is a symbolic link or a directory git reports whole (a nested repository), and for a
/// path the file system will not describe: none of those is ever judged equal. `None` when
/// git cannot read the work tree, `rev`, the index or a file's content.
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
    let trunk = listing(
        worktree,
        &["ls-tree", "-r", "-z", "--end-of-options", rev],
        2,
    );
    let index = listing(worktree, &["ls-files", "--stage", "-z"], 1);
    let (Some(blobs), Some(index)) = (trunk, index) else {
        return None;
    };
    let mut to_hash: Vec<(&str, &str)> = Vec::new();
    for (p, staged) in &paths {
        let held = blobs.get(p.as_str());
        // a staged version is uncommitted work too, and the removal would discard it
        if *staged
            && index
                .get(p.as_str())
                .is_some_and(|version| held != Some(version))
        {
            return Some(false);
        }
        let meta = std::fs::symlink_metadata(worktree.join(p));
        // a parent that became a file is a deletion too
        let absent = meta
            .as_ref()
            .err()
            .map(std::io::Error::kind)
            .is_some_and(|k| {
                k == std::io::ErrorKind::NotFound || k == std::io::ErrorKind::NotADirectory
            });
        match meta {
            _ if absent => {
                if held.is_some() {
                    return Some(false); // deleted here, present in the trunk
                }
            }
            Ok(m) if m.file_type().is_file() => {
                let Some((mode, blob)) = held else {
                    return Some(false); // present here, absent in the trunk
                };
                if *mode != file_mode(&m) {
                    return Some(false);
                }
                to_hash.push((p.as_str(), blob.as_str()));
            }
            // a symbolic link, a directory, or a path that cannot be described
            _ => return Some(false),
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
        // an answer that does not name every path proves nothing about the ones it skipped
        if got.len() != chunk.len() || chunk.iter().zip(got).any(|((_, want), got)| *want != got) {
            return Some(false);
        }
    }
    Some(true)
}

/// Every path `git status --porcelain -z` names, both sides of a rename or copy included,
/// each with whether the index holds a change there (the source of a rename is not in the
/// index). `None` when an entry is an unresolved conflict, or a path is not UTF-8.
fn changed_paths(status: &[u8]) -> Option<Vec<(String, bool)>> {
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
        let staged = !matches!(x, ' ' | '?');
        paths.push((String::from_utf8(entry[3..].to_vec()).ok()?, staged));
        if matches!(x, 'R' | 'C') || matches!(y, 'R' | 'C') {
            let from = entries.next()?;
            paths.push((String::from_utf8(from.to_vec()).ok()?, false));
        }
    }
    Some(paths)
}

/// A NUL-separated listing of `<three fields>\t<path>` entries (`ls-tree -r -z`: mode, type,
/// id; `ls-files --stage -z`: mode, id, stage), as path to (mode, id), the id being field
/// `id_at`. Submodules keep their gitlink mode, so a file at such a path never matches. A path
/// that is not UTF-8 is left out: it can equal no path [`changed_paths`] answers, which
/// refuses such paths itself.
fn listing(cwd: &Path, args: &[&str], id_at: usize) -> Option<BTreeMap<String, (String, String)>> {
    let out = git::try_run(cwd, args)
        .ok()
        .filter(|o| o.status == Some(0))?;
    Some(
        out.stdout
            .split(|b| *b == 0)
            .filter_map(|e| std::str::from_utf8(e).ok())
            .filter_map(|e| entry(e, id_at))
            .collect(),
    )
}

/// One entry of [`listing`] as (path, (mode, id)); `None` for anything else (the empty text
/// after the last NUL among them).
fn entry(text: &str, id_at: usize) -> Option<(String, (String, String))> {
    let (meta, path) = text.split_once('\t')?;
    let meta: Vec<&str> = meta.split(' ').collect();
    match (meta.len(), meta.first(), meta.get(id_at)) {
        (3, Some(mode), Some(id)) => Some((path.to_string(), (mode.to_string(), id.to_string()))),
        _ => None,
    }
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
    use std::path::PathBuf;

    /// Run git in `cwd` and answer whether it succeeded; never inherits a hook's repository.
    fn git_ok(cwd: &Path, args: &[&str]) -> bool {
        std::process::Command::new("git")
            .args(["-c", "user.email=t@t", "-c", "user.name=t"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .current_dir(cwd)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .expect("run git")
            .status
            .success()
    }

    fn git(cwd: &Path, args: &[&str]) {
        assert!(git_ok(cwd, args), "git {args:?} in {}", cwd.display());
    }

    fn write(dir: &Path, path: &str, text: &str) {
        std::fs::write(dir.join(path), text).expect("write");
    }

    /// A repository whose `main` holds `a.txt` ("a") and `dir/x.txt` ("x"), with a branch
    /// `work` at the same commit checked out.
    fn repo() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().canonicalize().expect("canonical").join("repo");
        std::fs::create_dir_all(dir.join("dir")).expect("mkdir");
        git(&dir, &["init", "-q", "-b", "main"]);
        write(&dir, "a.txt", "a\n");
        write(&dir, "dir/x.txt", "x\n");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "base"]);
        git(&dir, &["switch", "-qc", "work"]);
        (tmp, dir)
    }

    #[test]
    fn a_rename_names_both_sides_and_a_conflict_answers_nothing() {
        let status = b"R  new.txt\0old.txt\0?? fresh.txt\0 M seen.txt\0";
        assert_eq!(
            changed_paths(status),
            Some(vec![
                ("new.txt".into(), true),
                ("old.txt".into(), false),
                ("fresh.txt".into(), false),
                ("seen.txt".into(), false),
            ])
        );
        assert_eq!(changed_paths(b"UU both.txt\0"), None);
        assert_eq!(changed_paths(b"!! ignored/\0"), Some(vec![]));
    }

    #[test]
    fn a_status_entry_it_cannot_read_whole_answers_nothing() {
        assert_eq!(changed_paths(b"M\0"), None, "too short to name a path");
        assert_eq!(
            changed_paths(b" M \xff.txt\0"),
            None,
            "a path that is not UTF-8"
        );
        assert_eq!(
            changed_paths(b"R  new.txt\0"),
            None,
            "a rename without its source"
        );
        assert_eq!(
            changed_paths(b"R  new.txt\0\xff\0"),
            None,
            "a source not UTF-8"
        );
    }

    #[test]
    fn a_listing_entry_is_three_fields_and_a_path() {
        assert_eq!(
            entry("100644 blob abc\ta.txt", 2),
            Some(("a.txt".into(), ("100644".into(), "abc".into())))
        );
        assert_eq!(
            entry("100755 abc 0\tbin/run", 1),
            Some(("bin/run".into(), ("100755".into(), "abc".into())))
        );
        assert_eq!(entry("", 2), None, "the text after the last NUL");
        assert_eq!(entry("100644 abc\ta.txt", 2), None, "two fields");
    }

    #[test]
    fn the_trunk_is_judged_by_its_remote_tracking_branch_when_it_has_one() {
        let (tmp, dir) = repo();
        let origin = tmp
            .path()
            .canonicalize()
            .expect("canonical")
            .join("origin.git");
        git(tmp.path(), &["init", "-q", "--bare", "origin.git"]);
        git(
            &dir,
            &["remote", "add", "origin", &origin.to_string_lossy()],
        );
        git(&dir, &["push", "-q", "-u", "origin", "main"]);
        assert_eq!(trunk_reference(&dir, "main"), "origin/main");
        assert_eq!(trunk_reference(&dir, "work"), "work", "no upstream");
    }

    #[test]
    fn content_is_landed_only_when_the_merge_writes_the_trunks_own_tree() {
        let (_tmp, dir) = repo();
        // the same file changed on both sides: a conflict
        write(&dir, "a.txt", "work\n");
        git(&dir, &["commit", "-qam", "work"]);
        git(&dir, &["switch", "-q", "main"]);
        write(&dir, "a.txt", "main\n");
        git(&dir, &["commit", "-qam", "main"]);
        assert_eq!(content_in(&dir, "work", "main"), Some(false), "a conflict");
        // a file the trunk lacks: another tree
        git(&dir, &["switch", "-qc", "more", "main"]);
        write(&dir, "more.txt", "more\n");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "more"]);
        assert_eq!(
            content_in(&dir, "more", "main"),
            Some(false),
            "another tree"
        );
        assert_eq!(
            content_in(&dir, "main", "main"),
            Some(true),
            "the trunk itself"
        );
        assert_eq!(
            content_in(&dir, "nosuch", "main"),
            Some(false),
            "cannot merge"
        );
        assert_eq!(
            content_in(&dir, "more", "nosuch"),
            None,
            "no trunk to judge by"
        );
    }

    #[test]
    fn uncommitted_changes_are_judged_against_the_trunk_path_by_path() {
        let (tmp, dir) = repo();
        assert_eq!(uncommitted_in(&tmp.path().join("plain"), "main"), None);
        std::fs::create_dir(tmp.path().join("plain")).expect("mkdir");
        assert_eq!(uncommitted_in(&tmp.path().join("plain"), "main"), None);

        write(&dir, "fresh.txt", "fresh\n");
        assert_eq!(
            uncommitted_in(&dir, "main"),
            Some(false),
            "here, not in the trunk"
        );
        std::fs::remove_file(dir.join("fresh.txt")).expect("rm");

        // committed on the branch only, then deleted: the work tree is the trunk's there
        write(&dir, "branch.txt", "branch\n");
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-qm", "branch only"]);
        std::fs::remove_file(dir.join("branch.txt")).expect("rm");
        assert_eq!(uncommitted_in(&dir, "main"), Some(true), "absent in both");
        git(&dir, &["checkout", "-q", "--", "branch.txt"]);

        write(&dir, "a.txt", "changed\n");
        assert_eq!(
            uncommitted_in(&dir, "nosuch"),
            None,
            "no revision to judge by"
        );
        assert_eq!(uncommitted_in(&dir, "main"), Some(false), "another blob");
        git(&dir, &["checkout", "-q", "--", "a.txt"]);

        std::fs::remove_file(dir.join("a.txt")).expect("rm");
        assert_eq!(
            uncommitted_in(&dir, "main"),
            Some(false),
            "deleted here, held there"
        );
        git(&dir, &["checkout", "-q", "--", "a.txt"]);

        // staged, then deleted from the work tree: the staged version would be lost
        write(&dir, "new.txt", "new\n");
        git(&dir, &["add", "new.txt"]);
        std::fs::remove_file(dir.join("new.txt")).expect("rm");
        assert_eq!(
            uncommitted_in(&dir, "main"),
            Some(false),
            "a staged version"
        );
        git(&dir, &["rm", "-q", "--cached", "new.txt"]);
        assert_eq!(uncommitted_in(&dir, "main"), Some(true), "nothing is left");

        // a directory replaced by a file the status does not name: a deletion of dir/x.txt
        std::fs::remove_dir_all(dir.join("dir")).expect("rm");
        write(&dir, "dir", "a file now\n");
        std::fs::write(dir.join(".git/info/exclude"), "/dir\n").expect("exclude");
        assert_eq!(
            uncommitted_in(&dir, "main"),
            Some(false),
            "a parent became a file"
        );
        std::fs::remove_file(dir.join("dir")).expect("rm");
        git(&dir, &["checkout", "-q", "--", "dir"]);
        assert_eq!(uncommitted_in(&dir, "main"), Some(true));
    }

    #[cfg(unix)]
    #[test]
    fn a_mode_a_link_or_an_unreadable_file_is_never_judged_equal() {
        use std::os::unix::fs::PermissionsExt;
        let (_tmp, dir) = repo();
        let set = |mode: u32| {
            std::fs::set_permissions(dir.join("a.txt"), std::fs::Permissions::from_mode(mode))
                .expect("chmod")
        };
        set(0o755);
        assert_eq!(
            uncommitted_in(&dir, "main"),
            Some(false),
            "the trunk's is 100644"
        );
        set(0o644);
        std::os::unix::fs::symlink("a.txt", dir.join("link")).expect("symlink");
        assert_eq!(uncommitted_in(&dir, "main"), Some(false), "a symbolic link");
        std::fs::remove_file(dir.join("link")).expect("rm");

        // the trunk moves on; the work tree holds the trunk's content, unreadable
        git(&dir, &["switch", "-q", "main"]);
        write(&dir, "a.txt", "moved\n");
        git(&dir, &["commit", "-qam", "moved"]);
        git(&dir, &["switch", "-q", "work"]);
        write(&dir, "a.txt", "moved\n");
        assert_eq!(
            uncommitted_in(&dir, "main"),
            Some(true),
            "the trunk's content"
        );
        git(&dir, &["add", "a.txt"]);
        assert_eq!(
            uncommitted_in(&dir, "main"),
            Some(true),
            "staged, the trunk's"
        );
        set(0o000);
        let unreadable = std::fs::read(dir.join("a.txt")).is_err();
        let got = uncommitted_in(&dir, "main");
        set(0o644);
        if unreadable {
            assert_eq!(got, None, "git cannot read the file's content");
        }
    }

    #[test]
    fn a_conflict_left_without_an_operation_is_unfinished_work() {
        let (_tmp, dir) = repo();
        write(&dir, "a.txt", "stashed\n");
        git(&dir, &["stash", "-q"]);
        write(&dir, "a.txt", "committed\n");
        git(&dir, &["commit", "-qam", "committed"]);
        assert!(
            !git_ok(&dir, &["stash", "apply", "-q"]),
            "the apply conflicts"
        );
        assert!(super::super::state::operation_in_progress(&dir).is_none());
        assert_eq!(uncommitted_in(&dir, "main"), Some(false));
    }

    #[test]
    fn a_merge_in_progress_is_unfinished_work() {
        let (_tmp, dir) = repo();
        write(&dir, "a.txt", "work\n");
        git(&dir, &["commit", "-qam", "work"]);
        git(&dir, &["switch", "-q", "main"]);
        write(&dir, "a.txt", "main\n");
        git(&dir, &["commit", "-qam", "main"]);
        assert!(
            !git_ok(&dir, &["merge", "-q", "work"]),
            "the merge conflicts"
        );
        assert!(super::super::state::operation_in_progress(&dir).is_some());
        assert_eq!(uncommitted_in(&dir, "main"), Some(false));
    }
}
