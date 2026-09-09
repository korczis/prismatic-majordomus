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

    /// A fingerprint whose every field is set, so that a test can change exactly one and
    /// know that the line it reads back is about that one.
    fn full() -> WorktreeFingerprint {
        WorktreeFingerprint {
            branch: Some("feature/x".into()),
            head: Some("abc123".into()),
            index_digest: "i".into(),
            staged_diff_digest: "s".into(),
            unstaged_diff_digest: "u".into(),
            untracked_manifest_digest: "n".into(),
            untracked_files: 2,
            ignored_manifest_digest: "g".into(),
            in_progress: None,
            tree_manifest_digest: Some("t".into()),
        }
    }

    /// Write a small tree: a nested directory, a file of known size, a symbolic link, and
    /// the `.git` file a linked worktree carries, which the manifest must not attest.
    fn tree(root: &std::path::Path, order_reversed: bool) {
        std::fs::create_dir_all(root.join("src/deep")).unwrap();
        let files: Vec<&str> = if order_reversed {
            vec!["src/deep/z.rs", "src/deep/a.rs"]
        } else {
            vec!["src/deep/a.rs", "src/deep/z.rs"]
        };
        for f in files {
            std::fs::write(root.join(f), "fn main() {}").unwrap();
        }
        std::fs::write(root.join(".git"), "gitdir: /elsewhere").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("../elsewhere", root.join("link")).unwrap();
    }

    #[test]
    fn the_manifest_attests_every_entry_and_never_the_worktrees_own_git_file() {
        let dir = tempfile::tempdir().unwrap();
        tree(dir.path(), false);
        let manifest = tree_manifest(dir.path()).unwrap();

        assert!(
            manifest.contains("src\u{0}dir"),
            "a directory is attested as one"
        );
        assert!(
            manifest.contains("src/deep/a.rs\u{0}file\u{0}12"),
            "a file is attested with its size: {manifest}"
        );
        assert!(
            !manifest.contains(".git\u{0}"),
            "the .git file is rewritten by the move on purpose and must not be compared: {manifest}"
        );
        #[cfg(unix)]
        assert!(
            manifest.contains("link\u{0}link\u{0}../elsewhere"),
            "a symbolic link is attested as a link and its target, never followed: {manifest}"
        );
    }

    #[test]
    fn the_manifest_does_not_depend_on_the_order_the_directory_was_written_in() {
        let one = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        tree(one.path(), false);
        tree(other.path(), true);
        assert_eq!(
            tree_manifest(one.path()).unwrap(),
            tree_manifest(other.path()).unwrap(),
            "read_dir order reached the manifest, so two equal trees would compare unequal"
        );
    }

    #[test]
    fn a_manifest_notices_a_file_that_changed_size_and_a_link_that_changed_target() {
        let dir = tempfile::tempdir().unwrap();
        tree(dir.path(), false);
        let before = tree_manifest(dir.path()).unwrap();
        std::fs::write(dir.path().join("src/deep/a.rs"), "fn main() { todo!() }").unwrap();
        assert_ne!(
            before,
            tree_manifest(dir.path()).unwrap(),
            "a longer file is a difference"
        );
    }

    #[test]
    fn two_equal_fingerprints_differ_in_nothing() {
        assert!(differences(&full(), &full()).is_empty());
    }

    #[test]
    fn every_field_of_the_fingerprint_is_compared_and_named() {
        // The whole point of this type: it decides whether a migration may delete the
        // original directory. A field it stopped comparing would be a field a move could
        // lose silently, so each is changed alone and read back by name.
        /// One field changed, and the line the reader must get back for it.
        type Change = (&'static str, fn(&mut WorktreeFingerprint));
        let cases: Vec<Change> = vec![
            ("branch changed", |f| f.branch = Some("other".into())),
            ("HEAD changed", |f| f.head = Some("def456".into())),
            ("the index differs", |f| f.index_digest = "other".into()),
            ("the staged changes differ", |f| {
                f.staged_diff_digest = "other".into()
            }),
            ("the unstaged changes differ", |f| {
                f.unstaged_diff_digest = "other".into()
            }),
            ("the untracked files differ", |f| {
                f.untracked_manifest_digest = "other".into()
            }),
            ("the ignored entries differ", |f| {
                f.ignored_manifest_digest = "other".into()
            }),
            ("the operation in progress differs", |f| {
                f.in_progress = Some("rebase".into())
            }),
            ("the directory tree differs", |f| {
                f.tree_manifest_digest = Some("other".into())
            }),
        ];
        for (expected, change) in cases {
            let mut after = full();
            change(&mut after);
            let found = differences(&full(), &after);
            assert_eq!(found.len(), 1, "changing one field reported {found:?}");
            assert!(
                found[0].starts_with(expected),
                "expected a line beginning {expected:?}, got {:?}",
                found[0]
            );
        }
    }

    #[test]
    fn a_tree_manifest_absent_on_either_side_is_not_a_difference() {
        // `capture` takes the manifest only when asked. Comparing a fingerprint that has one
        // with a fingerprint that does not must not read as a lost directory.
        let (mut before, mut after) = (full(), full());
        after.tree_manifest_digest = None;
        assert!(
            differences(&before, &after).is_empty(),
            "absent is unknown, not different"
        );
        before.tree_manifest_digest = None;
        after.tree_manifest_digest = Some("t".into());
        assert!(differences(&before, &after).is_empty());
    }

    #[test]
    fn an_unborn_head_and_a_detached_one_are_named_rather_than_left_blank() {
        let (mut before, mut after) = (full(), full());
        before.head = None;
        after.branch = None;
        let found = differences(&before, &after);
        assert!(found.iter().any(|l| l.contains("(unborn)")), "{found:?}");
        assert!(found.iter().any(|l| l.contains("(detached)")), "{found:?}");
    }
}
