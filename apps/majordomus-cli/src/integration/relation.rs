//! What a pull request's head is to the current master, decided by git on this machine.
//!
//! The forge cannot answer this here: its merge cannot run the per-clone derived-file
//! driver (`merge=derived`), so it calls nearly every pull request conflicting over files a
//! generator rewrites anyway. `git merge-tree --write-tree` performs the real merge in the
//! object database — no work tree, no index, no ref moves — with this clone's drivers, and
//! `git check-attr merge` says which of the paths it touches are derived, from master's own
//! `.gitattributes`. Neither list is kept here.

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

/// Whether `commit` names a commit this clone has.
pub fn has_commit(root: &Path, commit: &str) -> bool {
    git(root, &["cat-file", "-e", &format!("{commit}^{{commit}}")])
        .map(|(ok, _)| ok)
        .unwrap_or(false)
}

/// The subset of `paths` that master's `.gitattributes` marks `merge=derived`, or why git
/// could not say. A failure is an error and never an empty set: an empty set reads as "no
/// path here is derived", which turns every projection into authored work.
pub fn derived_paths(
    root: &Path,
    master: &str,
    paths: &[String],
) -> Result<BTreeSet<String>, String> {
    if paths.is_empty() {
        return Ok(BTreeSet::new());
    }
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(root)
        .args(["check-attr", "--source", master, "-z", "--stdin", "merge"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("git check-attr could not run: {e}"))?;
    let mut input = Vec::new();
    for p in paths {
        input.extend_from_slice(p.as_bytes());
        input.push(0);
    }
    // Written from a thread of its own: git answers while it reads, and a long list would
    // otherwise fill the answer's pipe while this side is still writing the question.
    let writer = child.stdin.take().map(|mut stdin| {
        std::thread::spawn(move || {
            use std::io::Write;
            stdin.write_all(&input)
        })
    });
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git check-attr did not finish: {e}"))?;
    if let Some(writer) = writer {
        match writer.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(format!("git check-attr could not be given the paths: {e}")),
            Err(_) => return Err("git check-attr could not be given the paths".into()),
        }
    }
    if !out.status.success() {
        return Err(format!(
            "git check-attr --source {master} exited {}: {}",
            out.status
                .code()
                .map_or_else(|| "on a signal".to_string(), |c| c.to_string()),
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
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
mod tests {
    use super::*;

    /// Attributes git could not read are an error, never the empty set that would call every
    /// projection authored; an empty question is answered without asking git at all.
    #[test]
    fn unreadable_attributes_are_an_error_and_not_an_empty_set() {
        let dir = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            assert!(Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .unwrap()
                .status
                .success());
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "t"]);
        std::fs::write(dir.path().join(".gitattributes"), "gen/** merge=derived\n").unwrap();
        git(&["add", "-A"]);
        git(&["commit", "-qm", "base"]);
        let paths = vec!["gen/x.json".to_string(), "lib/a.sh".to_string()];
        let derived = derived_paths(dir.path(), "HEAD", &paths).unwrap();
        assert_eq!(derived, ["gen/x.json".to_string()].into());
        let err = derived_paths(dir.path(), "no-such-commit", &paths).unwrap_err();
        assert!(err.contains("check-attr"), "{err}");
        assert!(derived_paths(dir.path(), "no-such-commit", &[])
            .unwrap()
            .is_empty());
    }
}
