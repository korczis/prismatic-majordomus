//! What git is asked, and nothing else.
//!
//! Every fact the lifecycle rests on is measured here with one bounded subprocess, through
//! [`crate::worktree::git`] — the one runner the worktree subsystem already owns, which
//! passes arguments as arguments, clears `GIT_DIR`, and takes no optional lock. Nothing in
//! this file writes, fetches, or reaches the network; `gh`, when it is used at all, is
//! bounded and optional and its absence is an answer rather than a failure.
//!
//! The one definition that matters is [`unique_commits`]. "The branch is not on origin" is
//! the wrong question — a branch nobody pushed may be wholly contained in a pushed
//! integration branch, and a branch whose name is on origin may still carry a commit no
//! origin ref holds. The right question is whether a commit is reachable from **any** ref
//! under `refs/remotes/origin/`, and that is what this measures.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::worktree::git;

use super::model::Landing;

/// Commits reachable from `rev` and from no ref under `refs/remotes/origin/`.
///
/// `None` when git could not answer — an unknown revision, a repository with no commits —
/// which the classifier treats as "unproven", never as zero.
pub fn unique_commits(repo: &Path, rev: &str) -> Option<usize> {
    let out = git::try_run(
        repo,
        &[
            "rev-list",
            "--count",
            "--end-of-options",
            rev,
            "--not",
            "--remotes=origin",
        ],
    )
    .ok()?;
    if out.status != Some(0) {
        return None;
    }
    out.text().ok()?.trim().parse::<usize>().ok()
}

/// The first `limit` of those commits, short id and subject, so a person can see what is at
/// risk without running a second command.
pub fn unique_sample(repo: &Path, rev: &str, limit: usize) -> Vec<String> {
    let max = format!("--max-count={limit}");
    let Ok(out) = git::try_run(
        repo,
        &[
            "log",
            &max,
            "--format=%h %s",
            "--end-of-options",
            rev,
            "--not",
            "--remotes=origin",
        ],
    ) else {
        return Vec::new();
    };
    if out.status != Some(0) {
        return Vec::new();
    }
    out.text()
        .map(|t| {
            t.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// The committer date of a revision, in seconds since the Unix epoch.
///
/// The committer date, and not the modification time of anything on disk: a status sweep
/// rewrites a worktree's index, so an inventory that reads mtimes reports every worktree it
/// just looked at as freshly active. This repository has already been fooled by exactly
/// that, and a measurement that changes because it was taken is not a measurement.
pub fn committed_at(repo: &Path, rev: &str) -> Option<i64> {
    let out = git::try_run(
        repo,
        &["log", "-1", "--format=%ct", "--end-of-options", rev],
    )
    .ok()?;
    if out.status != Some(0) {
        return None;
    }
    out.text().ok()?.trim().parse::<i64>().ok()
}

/// Is `ancestor` reachable from `descendant`? `None` when git could not decide.
pub fn is_ancestor(repo: &Path, ancestor: &str, descendant: &str) -> Option<bool> {
    let out = git::try_run(
        repo,
        &[
            "merge-base",
            "--is-ancestor",
            "--end-of-options",
            ancestor,
            descendant,
        ],
    )
    .ok()?;
    match out.status {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

/// Would merging `head` into `base` conflict?
///
/// Proved here with `git merge-tree --write-tree`, in this clone, where this repository's
/// `merge=derived` driver is configured — which is the whole point. The forge cannot run
/// that driver, so it reports a conflict for every generated file and calls pull requests
/// unmergeable that merge cleanly in seconds locally. The forge's own answer is recorded
/// beside this one and believed by nobody.
///
/// `None` when git could not decide: a version without `merge-tree --write-tree`, an
/// unknown revision, or an unrelated history.
pub fn conflicts_with(repo: &Path, base: &str, head: &str) -> Option<bool> {
    let out = git::try_run(
        repo,
        &["merge-tree", "--write-tree", "--end-of-options", base, head],
    )
    .ok()?;
    match out.status {
        Some(0) => Some(false),
        // git exits 1 for a conflicted merge; anything else is "could not decide"
        Some(1) => Some(true),
        _ => None,
    }
}

/// Every merge on the trunk's first-parent history that names a branch, as a map from
/// branch name to what landed it.
///
/// The trunk's own merge commits are the offline record of what landed and when: a merge
/// made by the forge says `Merge pull request #123 from owner/branch`, and one made by hand
/// says `Merge branch 'branch'`. Both are read; neither needs the network, and the answer
/// does not change when a token expires.
pub fn landings(repo: &Path, trunk: &str, limit: usize) -> BTreeMap<String, Landing> {
    let mut found: BTreeMap<String, Landing> = BTreeMap::new();
    let max = format!("--max-count={limit}");
    let Ok(out) = git::try_run(
        repo,
        &[
            "log",
            "--first-parent",
            "--merges",
            &max,
            "--format=%H%x1f%P%x1f%s",
            "--end-of-options",
            trunk,
        ],
    ) else {
        return found;
    };
    if out.status != Some(0) {
        return found;
    }
    let Ok(text) = out.text() else {
        return found;
    };
    for line in text.lines() {
        let mut parts = line.split('\u{1f}');
        let (Some(commit), Some(parents), Some(subject)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let merged_head = parents.split_whitespace().nth(1).map(|s| s.to_string());
        let Some((branch, pull_request)) = parse_merge_subject(subject) else {
            continue;
        };
        // the first-parent walk is newest first; the newest landing of a branch wins
        found.entry(branch).or_insert(Landing {
            merge_commit: commit.to_string(),
            pull_request,
            merged_head,
            residue: Vec::new(),
        });
    }
    found
}

/// The branch a merge commit's subject names, and the pull request number when it carries
/// one.
///
/// The worked examples are `the_forge_and_hand_merge_subjects_both_name_a_branch` below.
pub fn parse_merge_subject(subject: &str) -> Option<(String, Option<u64>)> {
    if let Some(rest) = subject.strip_prefix("Merge pull request #") {
        let (number, rest) = rest.split_once(' ')?;
        let number = number.trim_end_matches(':').parse::<u64>().ok()?;
        let reference = rest.strip_prefix("from ")?.split_whitespace().next()?;
        // `owner/branch` where the branch itself may contain slashes: an owner segment is
        // present only when what follows still looks like a branch, so the first segment is
        // dropped only if the remainder is non-empty.
        let branch = match reference.split_once('/') {
            Some((_owner, branch)) if !branch.is_empty() && reference.matches('/').count() >= 1 => {
                branch
            }
            _ => reference,
        };
        return Some((branch.to_string(), Some(number)));
    }
    if let Some(rest) = subject.strip_prefix("Merge branch '") {
        let branch = rest.split('\'').next()?;
        if branch.is_empty() {
            return None;
        }
        return Some((branch.to_string(), None));
    }
    None
}

/// Annotated tags under `refs/tags/archive/` that contain `rev`, each with the reason its
/// message records.
///
/// An archive is a deliberate act with a reason written down. A `rescue/` tag is not one:
/// it preserves an object and records nothing about why, so work under a rescue tag is
/// still work nobody has decided about.
pub fn archive_tags(repo: &Path, rev: &str) -> Vec<String> {
    let Ok(out) = git::try_run(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname:short)%1f%(contents:subject)",
            &format!("--contains={rev}"),
            "refs/tags/archive/",
        ],
    ) else {
        return Vec::new();
    };
    if out.status != Some(0) {
        return Vec::new();
    }
    out.text()
        .map(|t| {
            t.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| match l.split_once('\u{1f}') {
                    Some((name, reason)) if !reason.trim().is_empty() => {
                        format!("{name}: {reason}")
                    }
                    Some((name, _)) => name.to_string(),
                    None => l.to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Read an RFC 3339 instant in UTC as seconds since the Unix epoch.
///
/// The crate links no date library on purpose, and the forge prints exactly one shape
/// (`2026-09-11T09:54:50Z`). Anything else is `None` rather than a guess.
///
/// The worked example is `the_instant_parser_round_trips_the_formatter` below.
pub fn parse_rfc3339_utc(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    let year: i64 = text.get(0..4)?.parse().ok()?;
    let month: i64 = text.get(5..7)?.parse().ok()?;
    let day: i64 = text.get(8..10)?.parse().ok()?;
    let hour: i64 = text.get(11..13)?.parse().ok()?;
    let minute: i64 = text.get(14..16)?.parse().ok()?;
    let second: i64 = text.get(17..19)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Howard Hinnant's `days_from_civil`: the inverse of the civil-from-days the peer board's
/// formatter uses, so the two round-trip.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Run a command with a bound, capturing its output.
///
/// Every wait in this repository carries a timeout, a completion predicate and a recovery
/// path (`project.every-wait-is-bounded`). This is the recovery path for the one command
/// here that can reach the network: the child is killed at the deadline and the caller gets
/// `None`, which reads as "the forge was not reached" and never as "there are no pull
/// requests".
pub fn run_bounded(program: &str, args: &[&str], cwd: &Path, bound: Duration) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut out = String::new();
                if let Some(mut stdout) = child.stdout.take() {
                    use std::io::Read;
                    stdout.read_to_string(&mut out).ok()?;
                }
                return Some(out);
            }
            Ok(None) => {
                if started.elapsed() >= bound {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_forge_and_hand_merge_subjects_both_name_a_branch() {
        assert_eq!(
            parse_merge_subject("Merge pull request #134 from korczis/feature/canonical-order"),
            Some(("feature/canonical-order".to_string(), Some(134)))
        );
        assert_eq!(
            parse_merge_subject("Merge branch 'fix/waits-are-bounded'"),
            Some(("fix/waits-are-bounded".to_string(), None))
        );
        assert_eq!(
            parse_merge_subject("Merge pull request #7 from fix/one"),
            Some(("fix/one".to_string(), Some(7)))
        );
        assert_eq!(parse_merge_subject("Merge remote-tracking branch"), None);
    }

    #[test]
    fn an_instant_that_is_not_the_forges_shape_is_refused() {
        assert_eq!(parse_rfc3339_utc("2026-08-29"), None);
        assert_eq!(parse_rfc3339_utc(""), None);
        assert_eq!(parse_rfc3339_utc("2026-13-01T00:00:00Z"), None);
    }

    #[test]
    fn a_merge_subject_that_names_nothing_is_not_a_landing() {
        assert_eq!(parse_merge_subject("chore: a plain commit"), None);
        assert_eq!(parse_merge_subject("Merge branch ''"), None);
        assert_eq!(parse_merge_subject("Merge pull request #x from a/b"), None);
    }

    #[test]
    fn a_branch_with_slashes_survives_the_owner_prefix() {
        assert_eq!(
            parse_merge_subject("Merge pull request #1 from korczis/feature/a/b/c"),
            Some(("feature/a/b/c".to_string(), Some(1)))
        );
    }

    #[test]
    fn the_instant_parser_round_trips_the_formatter() {
        for seconds in [0_i64, 1_000_000, 1_788_000_000, 2_000_000_000] {
            let formatted = crate::peers::rfc3339(
                std::time::UNIX_EPOCH + Duration::from_secs(seconds as u64),
            );
            assert_eq!(
                parse_rfc3339_utc(&formatted),
                Some(seconds),
                "{formatted} did not round-trip"
            );
        }
    }

    #[test]
    fn a_bounded_run_of_something_absent_is_none_not_a_hang() {
        let here = std::env::temp_dir();
        assert_eq!(
            run_bounded(
                "majordomus-no-such-program-exists",
                &[],
                &here,
                Duration::from_millis(200)
            ),
            None
        );
    }
}
