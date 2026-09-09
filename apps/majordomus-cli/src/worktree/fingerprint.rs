//! The proof that a move lost nothing.
//!
//! A migration takes one of these before a worktree moves and one after, and refuses to
//! call the move a success unless they are equal. It is strong enough to notice a lost
//! untracked file, a dropped staged change, a modified tracked file that reverted, a branch
//! that switched or a HEAD that moved; it is deliberately not taken on any hot path, because
//! it hashes the content of every modified and untracked file. Migration is exceptional and
//! data safety is worth more there than a few milliseconds.
//!
//! Ignored content (build output, the checkout's own `.ai/local/` state) is attested by
//! presence and size rather than by content: it moves with the directory as everything else
//! does, and hashing a `target/` tree to prove a rename would cost minutes for nothing.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::error::{Result, WorktreeError};
use super::git;

/// What a work tree held at one moment, reduced to digests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WorktreeFingerprint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The branch, or none when detached.
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The commit.
    pub head: Option<String>,
    /// A digest of the index: every tracked path with its mode, blob and stage.
    pub index_digest: String,
    /// A digest of the staged diff against HEAD.
    pub staged_diff_digest: String,
    /// A digest of the unstaged diff against the index.
    pub unstaged_diff_digest: String,
    /// A digest of every untracked file: path, kind, size and content.
    pub untracked_manifest_digest: String,
    /// How many untracked files the manifest holds.
    pub untracked_files: usize,
    /// A digest of every ignored entry: path, kind and size, never content.
    pub ignored_manifest_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// The operation in progress, if any.
    pub in_progress: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// A digest of every entry of the directory tree, ignored included: path, kind, size
    /// and link target. Taken only for a move made by copying, where the rename guarantee
    /// does not hold.
    pub tree_manifest_digest: Option<String>,
}

fn hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn hash_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|e| WorktreeError::io(path, &e))?;
    Ok(hex(&bytes))
}

/// Take a fingerprint of `worktree`. `whole_tree` also walks the directory for the tree
/// manifest, which a copy-based move needs and a rename does not.
pub fn capture(worktree: &Path, whole_tree: bool) -> Result<WorktreeFingerprint> {
    let branch = git::branch_of(worktree)?;
    let head = git::head_of(worktree)?;
    let index = git::run(worktree, &["ls-files", "--stage", "-z"])?.stdout;
    let staged = git::run(
        worktree,
        &[
            "diff",
            "--cached",
            "--binary",
            "--no-ext-diff",
            "--no-color",
        ],
    )?
    .stdout;
    let unstaged = git::run(
        worktree,
        &["diff", "--binary", "--no-ext-diff", "--no-color"],
    )?
    .stdout;

    let untracked_list = git::run(
        worktree,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?
    .stdout;
    let mut untracked_manifest = String::new();
    let mut untracked_files = 0usize;
    for raw in untracked_list.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        let rel = String::from_utf8_lossy(raw).to_string();
        let path = worktree.join(&rel);
        let meta = std::fs::symlink_metadata(&path).map_err(|e| WorktreeError::io(&path, &e))?;
        let line = if meta.file_type().is_symlink() {
            let target = std::fs::read_link(&path).map_err(|e| WorktreeError::io(&path, &e))?;
            format!("{rel}\0link\0{}\n", target.display())
        } else if meta.is_file() {
            format!("{rel}\0file\0{}\0{}\n", meta.len(), hash_file(&path)?)
        } else {
            format!("{rel}\0other\n")
        };
        untracked_manifest.push_str(&line);
        untracked_files += 1;
    }

    let ignored_list = git::run(
        worktree,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "-z",
        ],
    )?
    .stdout;
    let mut ignored_manifest = String::new();
    for raw in ignored_list.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        let rel = String::from_utf8_lossy(raw).to_string();
        let path = worktree.join(&rel);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.is_file() => {
                ignored_manifest.push_str(&format!("{rel}\0file\0{}\n", meta.len()))
            }
            Ok(meta) if meta.file_type().is_symlink() => {
                ignored_manifest.push_str(&format!("{rel}\0link\n"))
            }
            Ok(_) => ignored_manifest.push_str(&format!("{rel}\0dir\n")),
            Err(_) => ignored_manifest.push_str(&format!("{rel}\0gone\n")),
        }
    }

    let tree_manifest_digest = if whole_tree {
        Some(hex(tree_manifest(worktree)?.as_bytes()))
    } else {
        None
    };

    Ok(WorktreeFingerprint {
        branch,
        head,
        index_digest: hex(&index),
        staged_diff_digest: hex(&staged),
        unstaged_diff_digest: hex(&unstaged),
        untracked_manifest_digest: hex(untracked_manifest.as_bytes()),
        untracked_files,
        ignored_manifest_digest: hex(ignored_manifest.as_bytes()),
        in_progress: super::state::operation_in_progress(worktree),
        tree_manifest_digest,
    })
}

/// Every entry below `root`, relative, sorted, with kind, size and link target. The `.git`
/// file of a linked worktree is excluded: it is rewritten by the move on purpose.
pub fn tree_manifest(root: &Path) -> Result<String> {
    let mut entries: Vec<String> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let read = std::fs::read_dir(&dir).map_err(|e| WorktreeError::io(&dir, &e))?;
        for entry in read {
            let entry = entry.map_err(|e| WorktreeError::io(&dir, &e))?;
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            if rel == ".git" {
                continue;
            }
            let meta =
                std::fs::symlink_metadata(&path).map_err(|e| WorktreeError::io(&path, &e))?;
            if meta.file_type().is_symlink() {
                let target = std::fs::read_link(&path).map_err(|e| WorktreeError::io(&path, &e))?;
                entries.push(format!("{rel}\0link\0{}", target.display()));
            } else if meta.is_dir() {
                entries.push(format!("{rel}\0dir"));
                stack.push(path);
            } else {
                entries.push(format!("{rel}\0file\0{}", meta.len()));
            }
        }
    }
    entries.sort();
    Ok(entries.join("\n"))
}

/// Everything that differs between two fingerprints, one line each. Empty means equal.
///
/// ```
/// use majordomus_cli::worktree::fingerprint::{differences, WorktreeFingerprint};
/// let a = WorktreeFingerprint {
///     branch: Some("x".into()), head: Some("abc".into()), index_digest: "i".into(),
///     staged_diff_digest: "s".into(), unstaged_diff_digest: "u".into(),
///     untracked_manifest_digest: "m".into(), untracked_files: 2, ignored_manifest_digest: "g".into(),
///     in_progress: None, tree_manifest_digest: None,
/// };
/// let mut b = a.clone();
/// assert!(differences(&a, &b).is_empty());
/// b.untracked_files = 1; b.untracked_manifest_digest = "n".into();
/// let d = differences(&a, &b);
/// assert_eq!(d.len(), 1);
/// assert!(d[0].contains("untracked"));
/// ```
pub fn differences(before: &WorktreeFingerprint, after: &WorktreeFingerprint) -> Vec<String> {
    let mut out = Vec::new();
    if before.branch != after.branch {
        out.push(format!(
            "branch changed from {} to {}",
            before.branch.as_deref().unwrap_or("(detached)"),
            after.branch.as_deref().unwrap_or("(detached)")
        ));
    }
    if before.head != after.head {
        out.push(format!(
            "HEAD changed from {} to {}",
            before.head.as_deref().unwrap_or("(unborn)"),
            after.head.as_deref().unwrap_or("(unborn)")
        ));
    }
    if before.index_digest != after.index_digest {
        out.push("the index differs".into());
    }
    if before.staged_diff_digest != after.staged_diff_digest {
        out.push("the staged changes differ".into());
    }
    if before.unstaged_diff_digest != after.unstaged_diff_digest {
        out.push("the unstaged changes differ".into());
    }
    if before.untracked_manifest_digest != after.untracked_manifest_digest {
        out.push(format!(
            "the untracked files differ ({} before, {} after)",
            before.untracked_files, after.untracked_files
        ));
    }
    if before.ignored_manifest_digest != after.ignored_manifest_digest {
        out.push("the ignored entries differ".into());
    }
    if before.in_progress != after.in_progress {
        out.push("the operation in progress differs".into());
    }
    if let (Some(a), Some(b)) = (&before.tree_manifest_digest, &after.tree_manifest_digest) {
        if a != b {
            out.push("the directory tree differs".into());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory tree, for the manifest: the half of a fingerprint that needs no git.
    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, content) in files {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        dir
    }

    #[test]
    fn the_manifest_notices_a_file_that_changed_size_and_one_that_went_missing() {
        // the property a migration depends on: a tree that lost something cannot produce
        // the manifest of the tree that had it
        let dir = tree(&[("a.txt", "one"), ("deep/b.txt", "two")]);
        let before = tree_manifest(dir.path()).unwrap();

        std::fs::write(dir.path().join("a.txt"), "one and more").unwrap();
        assert_ne!(
            tree_manifest(dir.path()).unwrap(),
            before,
            "size is attested"
        );

        std::fs::write(dir.path().join("a.txt"), "one").unwrap();
        assert_eq!(tree_manifest(dir.path()).unwrap(), before, "and only that");

        std::fs::remove_file(dir.path().join("deep/b.txt")).unwrap();
        assert_ne!(tree_manifest(dir.path()).unwrap(), before);
    }

    #[test]
    fn the_manifest_is_the_same_whatever_order_the_filesystem_hands_entries_back_in() {
        // two trees with the same content built in opposite orders: a manifest that
        // depended on readdir order would call a faithful copy a difference
        let one = tree(&[("a", "1"), ("b", "2"), ("c/d", "3")]);
        let two = tree(&[("c/d", "3"), ("b", "2"), ("a", "1")]);
        assert_eq!(
            tree_manifest(one.path()).unwrap(),
            tree_manifest(two.path()).unwrap()
        );
    }

    #[test]
    fn the_git_file_of_a_linked_worktree_is_left_out_because_the_move_rewrites_it() {
        let dir = tree(&[("a", "1")]);
        let before = tree_manifest(dir.path()).unwrap();
        std::fs::write(dir.path().join(".git"), "gitdir: /somewhere/else").unwrap();
        assert_eq!(
            tree_manifest(dir.path()).unwrap(),
            before,
            "a move rewrites .git on purpose, so attesting it would fail every migration"
        );
    }

    #[test]
    fn two_equal_fingerprints_differ_in_nothing_and_each_change_is_named() {
        let base = WorktreeFingerprint {
            branch: Some("feature/x".into()),
            head: Some("abc123".into()),
            index_digest: "i".into(),
            staged_diff_digest: "s".into(),
            unstaged_diff_digest: "u".into(),
            untracked_manifest_digest: "n".into(),
            untracked_files: 2,
            ignored_manifest_digest: "g".into(),
            in_progress: None,
            tree_manifest_digest: None,
        };
        assert!(differences(&base, &base).is_empty(), "equal is equal");

        let mut moved = base.clone();
        moved.branch = Some("feature/y".into());
        let found = differences(&base, &moved);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("feature/x") && found[0].contains("feature/y"));

        // the one a migration exists to catch: work that was there and is not
        let mut lost = base.clone();
        lost.untracked_manifest_digest = "different".into();
        lost.untracked_files = 1;
        assert!(
            !differences(&base, &lost).is_empty(),
            "a lost untracked file is a difference, or the guarantee is worthless"
        );
    }
}
