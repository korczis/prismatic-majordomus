//! What a pull request's head is to the current master, decided by git on this machine.
//!
//! The forge cannot answer this here: its merge cannot run the per-clone derived-file
//! driver (`merge=derived`), so it calls nearly every pull request conflicting over files a
//! generator rewrites anyway. `git merge-tree --write-tree` performs the real merge in the
//! object database — no work tree, no index, no ref moves — with this clone's drivers, and
//! `git check-attr merge` says which of the paths it touches are derived, from master's own
//! `.gitattributes`. Neither list is kept here. Where the merge would conflict or change only
//! derived output, `git cherry` says whether every commit of the head is on master already as
//! an equal patch ([`patches_upstream`]).

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use super::model::RelationToMaster;

fn git(root: &Path, args: &[&str]) -> Result<(bool, String), String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("git could not run: {e}"))?;
    Ok((
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    ))
}

/// The pull requests whose mirrored head (`refs/majordomus/prs/<n>`) contains `head`, each
/// with the commit its mirror names. Empty when git cannot say: an inferred dependency is
/// evidence, and its absence decides nothing.
pub fn containing(root: &Path, head: &str) -> Vec<(u64, String)> {
    git(
        root,
        &[
            "for-each-ref",
            "--contains",
            head,
            "--format=%(objectname) %(refname)",
            super::forge::PR_REF_PREFIX,
        ],
    )
    .ok()
    .filter(|(ok, _)| *ok)
    .map(|(_, out)| {
        out.lines()
            .filter_map(|l| {
                // git prints `<sha> <ref>` for refs under the prefix it was given; a name that
                // is not a number after it is no mirror of a pull request
                l.split_once(' ').and_then(|(sha, name)| {
                    name.strip_prefix(super::forge::PR_REF_PREFIX)
                        .and_then(|n| n.parse().ok())
                        .map(|n| (n, sha.to_string()))
                })
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Whether `commit` names a commit this clone has.
pub fn has_commit(root: &Path, commit: &str) -> bool {
    git(root, &["cat-file", "-e", &format!("{commit}^{{commit}}")])
        .map(|(ok, _)| ok)
        .unwrap_or(false)
}

/// The subset of `paths` that master's `.gitattributes` marks `merge=derived`, or why git
/// could not say. An unread attribute is never "not derived": that answer would call every
/// derived path authored, and a relation decided on it would be kept forever.
pub fn derived_paths(
    root: &Path,
    master: &str,
    paths: &[String],
) -> Result<BTreeSet<String>, String> {
    if paths.is_empty() {
        return Ok(BTreeSet::new());
    }
    let mut input = Vec::new();
    for p in paths {
        input.extend_from_slice(p.as_bytes());
        input.push(0);
    }
    let out = Command::new("git")
        .current_dir(root)
        .args(["check-attr", "--source", master, "-z", "--stdin", "merge"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            // written from another thread while this one reads: git answers as it reads, and
            // with enough paths its stdout pipe fills while a write from here would still be
            // blocked on its stdin — each waiting on the other. A write git refused shows in
            // its exit status, which is read below; stdin closes when the writer ends.
            let stdin = child.stdin.take();
            let writer = std::thread::spawn(move || stdin.map(|mut s| s.write_all(&input)));
            let out = child.wait_with_output();
            let _ = writer.join();
            out
        })
        .map_err(|e| format!("git check-attr could not run: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git check-attr --source {master} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    // -z output: path NUL attribute NUL value NUL, repeated
    let text = String::from_utf8_lossy(&out.stdout);
    let fields: Vec<&str> = text.split('\0').collect();
    Ok(fields
        .chunks(3)
        .filter(|c| c.len() == 3 && c[2] == "derived")
        .map(|c| c[0].to_string())
        .collect())
}

/// How many commits `head` has that `master` lacks, when every one of them is on master
/// already as an equal patch: `git cherry` marks each `-`. `None` for a partial match, for no
/// commit at all, when git fails, and for a head that carries a merge commit of its own in
/// that range — `git cherry` skips merges, and a merge's own resolution has no patch to
/// compare, so a match of the rest proves nothing about it.
///
/// It is asked only where it can change the answer: when the merge would conflict, or would
/// change only derived output. Where a clean merge changes authored paths, master lacks
/// something the head carries (a landed change master reverted since, say), and equal patches
/// do not make that redundant.
pub fn patches_upstream(root: &Path, master: &str, head: &str) -> Option<u64> {
    let range = format!("{master}..{head}");
    // one chain, one failure: a range git cannot count, a merge of the head's own in it, and
    // a cherry git cannot answer all prove nothing
    git(root, &["rev-list", "--count", "--min-parents=2", &range])
        .ok()
        .filter(|(ok, merges)| *ok && merges.trim() == "0")
        .and_then(|_| git(root, &["cherry", master, head]).ok())
        .filter(|(ok, _)| *ok)
        .and_then(|(_, out)| {
            let marks: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
            (!marks.is_empty() && marks.iter().all(|l| l.starts_with("- ")))
                .then_some(marks.len() as u64)
        })
}

/// What `head` is to `master`, with the authored paths it changes.
///
/// ```text
/// use crate::integration::{relation_to_master, RelationToMaster};
/// let r = relation_to_master(std::path::Path::new("."), "origin/master", "origin/master");
/// assert_eq!(r, RelationToMaster::Contained);
/// ```text
pub fn relation_to_master(root: &Path, master: &str, head: &str) -> RelationToMaster {
    if !has_commit(root, head) {
        return RelationToMaster::Unknown {
            reason: format!(
                "the head {head} is not in this clone; majordomus prs refresh fetches it"
            ),
        };
    }
    match git(root, &["merge-base", "--is-ancestor", head, master]) {
        Ok((true, _)) => return RelationToMaster::Contained,
        Ok(_) => {}
        Err(reason) => return RelationToMaster::Unknown { reason },
    }
    let (clean, out) = match git(
        root,
        &[
            "merge-tree",
            "--write-tree",
            "--name-only",
            "--no-messages",
            master,
            head,
        ],
    ) {
        Ok(v) => v,
        Err(reason) => return RelationToMaster::Unknown { reason },
    };
    let mut lines = out.lines();
    let Some(tree) = lines.next().map(str::trim).filter(|t| !t.is_empty()) else {
        return RelationToMaster::Unknown {
            reason: "git merge-tree printed no tree".into(),
        };
    };
    let conflicted: Vec<String> = if clean {
        Vec::new()
    } else {
        lines
            .take_while(|l| !l.is_empty())
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    };
    let changed: Vec<String> = match git(root, &["diff", "--name-only", "-z", master, tree]) {
        Ok((true, s)) => s
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect(),
        Ok((false, _)) => {
            return RelationToMaster::Unknown {
                reason: "git diff of the merge result failed".into(),
            }
        }
        Err(reason) => return RelationToMaster::Unknown { reason },
    };
    let mut all: Vec<String> = changed.clone();
    all.extend(conflicted.iter().cloned());
    let derived = match derived_paths(root, master, &all) {
        Ok(d) => d,
        Err(reason) => return RelationToMaster::Unknown { reason },
    };
    // a conflict on a derived path is the regeneration's to resolve, not a person's
    let authored_conflicts: Vec<String> = conflicted
        .iter()
        .filter(|p| !derived.contains(*p))
        .cloned()
        .collect();
    if !authored_conflicts.is_empty() {
        // a change that landed by a cherry-pick and that master then moved past conflicts
        // with master; its patches say it landed
        if let Some(commits) = patches_upstream(root, master, head) {
            return RelationToMaster::PatchIdsUpstream { commits };
        }
        return RelationToMaster::Conflicting {
            paths: authored_conflicts,
        };
    }
    let authored: Vec<String> = changed
        .iter()
        .filter(|p| !derived.contains(*p))
        .cloned()
        .collect();
    if changed.is_empty() && conflicted.is_empty() {
        return RelationToMaster::Superseded;
    }
    if authored.is_empty() {
        if let Some(commits) = patches_upstream(root, master, head) {
            return RelationToMaster::PatchIdsUpstream { commits };
        }
        let paths: BTreeSet<String> = changed.into_iter().chain(conflicted).collect();
        return RelationToMaster::DerivedOnly {
            paths: paths.into_iter().collect(),
        };
    }
    match git(root, &["merge-base", "--is-ancestor", master, head]) {
        Ok((true, _)) => RelationToMaster::UpToDate { authored },
        Ok(_) => {
            let behind = git(root, &["rev-list", "--count", &format!("{head}..{master}")])
                .ok()
                .and_then(|(_, s)| s.trim().parse().ok())
                .unwrap_or(0);
            RelationToMaster::Behind { behind, authored }
        }
        Err(reason) => RelationToMaster::Unknown { reason },
    }
}

#[cfg(test)]
mod patch_tests {
    //! Equal patches on master, against real repositories.

    use super::*;

    fn g(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.email=t@example.com", "-c", "user.name=t"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn write(dir: &Path, file: &str, text: &str) {
        std::fs::write(dir.join(file), text).unwrap();
    }

    /// master: base, P (a.txt and the derived gen.json), D (gen.json regenerated); the head
    /// branches from base and carries P cherry-picked, so every one of its commits is on
    /// master as an equal patch while its merge still conflicts on the derived file alone.
    fn repo() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        g(r, &["init", "-q", "-b", "master"]);
        write(r, ".gitattributes", "gen.json merge=derived\n");
        write(r, "a.txt", "a\n");
        write(r, "gen.json", "v0\n");
        g(r, &["add", "-A"]);
        g(r, &["commit", "-q", "-m", "base"]);
        let base = g(r, &["rev-parse", "HEAD"]);
        write(r, "a.txt", "a2\n");
        write(r, "gen.json", "v1\n");
        g(r, &["commit", "-q", "-am", "P"]);
        let p = g(r, &["rev-parse", "HEAD"]);
        write(r, "gen.json", "v2\n");
        g(r, &["commit", "-q", "-am", "D"]);
        g(r, &["checkout", "-q", "-b", "topic", &base]);
        // -x: a commit of its own with an equal patch, never the same commit recreated
        g(r, &["cherry-pick", "-x", &p]);
        g(r, &["checkout", "-q", "master"]);
        (dir, base)
    }

    #[test]
    fn a_head_whose_patches_landed_and_whose_merge_touches_only_derived_output_landed() {
        let (dir, _) = repo();
        assert_eq!(patches_upstream(dir.path(), "master", "topic"), Some(1));
        assert_eq!(
            relation_to_master(dir.path(), "master", "topic"),
            RelationToMaster::PatchIdsUpstream { commits: 1 }
        );
    }

    #[test]
    fn a_patch_master_lacks_a_merge_of_its_own_or_an_unknown_range_prove_nothing() {
        let (dir, base) = repo();
        let r = dir.path();
        assert_eq!(
            patches_upstream(r, "master", "no-such-ref"),
            None,
            "git refuses"
        );
        assert_eq!(
            patches_upstream(r, "master", "master"),
            None,
            "no commit at all"
        );
        g(r, &["checkout", "-q", "topic"]);
        write(r, "b.txt", "b\n");
        g(r, &["add", "b.txt"]);
        g(r, &["commit", "-q", "-m", "new"]);
        assert_eq!(
            patches_upstream(r, "master", "topic"),
            None,
            "a patch master lacks"
        );
        g(r, &["checkout", "-q", "-b", "side", &base]);
        write(r, "c.txt", "c\n");
        g(r, &["add", "c.txt"]);
        g(r, &["commit", "-q", "-m", "side"]);
        g(r, &["checkout", "-q", "topic"]);
        g(r, &["merge", "-q", "--no-ff", "side", "-m", "merge side"]);
        assert_eq!(
            patches_upstream(r, "master", "topic"),
            None,
            "a merge of its own"
        );
    }
}

#[cfg(test)]
mod containing_tests {
    use super::*;

    #[test]
    fn the_mirrors_whose_head_contains_a_commit_are_named_with_their_tips() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        let g = |args: &[&str]| {
            let out = Command::new("git")
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .arg("-C")
                .arg(r)
                .args(["-c", "user.email=t@example.com", "-c", "user.name=t"])
                .args(args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?}: {out:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        g(&["init", "-q", "-b", "master"]);
        g(&["commit", "-q", "--allow-empty", "-m", "one"]);
        let one = g(&["rev-parse", "HEAD"]);
        g(&["update-ref", "refs/majordomus/prs/1", &one]);
        g(&["commit", "-q", "--allow-empty", "-m", "two"]);
        let two = g(&["rev-parse", "HEAD"]);
        g(&["update-ref", "refs/majordomus/prs/2", &two]);
        g(&["update-ref", "refs/majordomus/prs/not-a-number", &two]);
        let found = containing(r, &one);
        assert_eq!(found, [(1, one.clone()), (2, two.clone())]);
        assert_eq!(containing(r, &two), [(2, two)]);
        assert!(containing(r, "no-such-commit").is_empty(), "git refuses");
    }
}
