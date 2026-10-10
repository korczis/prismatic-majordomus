//! The closing readings: what a task leaves behind that was not there when it started.
//!
//! A person ending a session asks the same things every time — is any debt added, are
//! there more branches and worktrees than at the start, are there more open pull requests —
//! and a worker answers from what it remembers doing, which is the one source that cannot
//! see what another worker did in the same hour and has every reason to round up. On
//! 2026-10-10 that question was put to one session seven times and answered seven times from
//! recollection; the eighth answer, measured, differed from all of them.
//!
//! So the three things asked are read here, each from the record that already holds it:
//!
//! - **recorded debt** from the ratchet baselines under `.ai/repo/` — every
//!   `*-baseline.txt` is a list of known violations a gate tolerates, one a line, so the
//!   number of entries at the commit the task started at and in the working tree now are
//!   two facts git has;
//! - **branches and worktrees** from git's own reflogs, which carry the moment each was
//!   created: the first line of `logs/refs/heads/<branch>` and of a linked worktree's
//!   `logs/HEAD`;
//! - **the backlog** from the last recorded forge observation, whose pull requests carry
//!   the moment each was opened.
//!
//! Nothing here judges. A reading is taken or it is an `Err` with the reason, and
//! [`super::done`] is where a reading becomes an answer — where an unread one is `unknown`
//! and never a pass.
//!
//! # What these readings are not
//!
//! They are the repository's, not one worker's. A branch created by another session since
//! this task started is counted, because nothing in git says who created a branch, and the
//! question a person asks at the end — did this get bigger while we worked — is about the
//! repository. And "created since and still here" is not "more than at the start": what was
//! removed in the meantime leaves no record, so the readings name what was added and
//! remains and claim nothing about the difference of two totals.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::integration::forge::ForgeObservation;

/// The directory a repository's debt baselines live in, repository-relative.
pub(crate) const DEBT_DIRECTORY: &str = ".ai/repo";

/// What makes a file there a debt baseline. Discovered by this suffix and never listed: a
/// baseline added tomorrow is read tomorrow.
pub(crate) const DEBT_SUFFIX: &str = "-baseline.txt";

/// One baseline's entries, at the commit the task started at and in the working tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DebtReading {
    /// The baseline, repository-relative.
    pub path: String,
    /// Its entries at the start; `None` when the file did not exist there.
    pub before: Option<usize>,
    /// Its entries now.
    pub after: usize,
}

impl DebtReading {
    /// How many entries it gained. A baseline that did not exist at the start gained
    /// nothing: the debt it lists was there unrecorded, and recording it is not adding it.
    pub(crate) fn grown_by(&self) -> usize {
        match self.before {
            Some(before) => self.after.saturating_sub(before),
            None => 0,
        }
    }
}

/// What was created since the task started and is still there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Accumulated {
    /// The linked worktrees registered since, by path. A set: each is there once, and its
    /// order is the paths' own.
    pub worktrees: BTreeSet<String>,
    /// The local branches created since, by name.
    pub branches: BTreeSet<String>,
    /// Every linked worktree registered now.
    pub worktrees_now: usize,
    /// Every local branch now.
    pub branches_now: usize,
    /// Branches and worktrees whose creation git has no record of, and which are therefore
    /// counted on neither side.
    pub undated: usize,
}

/// The pull requests opened since the task started and still open when the forge was last
/// observed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Backlog {
    /// When the forge was observed, RFC 3339.
    pub observed_at: String,
    /// Every pull request open then.
    pub open: usize,
    /// The ones opened since the task started, by number.
    pub opened_since: BTreeSet<u64>,
}

/// Everything the closing questions are answered from. Each is its own `Result`: one
/// reading that could not be taken does not take the others with it.
#[derive(Debug, Clone)]
pub(crate) struct Closing {
    /// The task's starting moment the other two are measured from, RFC 3339.
    pub since: String,
    /// Every debt baseline, sorted by path.
    pub debt: Result<Vec<DebtReading>, String>,
    /// Branches and worktrees.
    pub accumulated: Result<Accumulated, String>,
    /// Pull requests.
    pub backlog: Result<Backlog, String>,
}

/// Take every closing reading for a task that started at commit `base`, at `since`.
pub(crate) fn read(root: &Path, base: &str, since: &str) -> Closing {
    let started = crate::peers::parse_rfc3339(since)
        .ok_or_else(|| format!("the task's starting moment '{since}' is not an RFC 3339 UTC time"));
    Closing {
        since: since.to_string(),
        debt: debt(root, base),
        accumulated: started.clone().and_then(|at| accumulated(root, at)),
        backlog: started.and_then(|at| {
            let observation = crate::integration::load_observation(root)?;
            backlog(observation.as_ref(), at)
        }),
    }
}

/// How many entries a baseline's text holds: every line that is neither blank nor a comment.
pub(crate) fn entries(text: &str) -> usize {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .count()
}

/// Every debt baseline of the working tree, with its entries at `base` and now.
pub(crate) fn debt(root: &Path, base: &str) -> Result<Vec<DebtReading>, String> {
    if base.is_empty() || base == "NONE" {
        return Err("the task records no commit it started at".into());
    }
    let known = crate::git::read_only(root)
        .args(["cat-file", "-e", &format!("{base}^{{commit}}")])
        .output()
        .map_err(|e| format!("git cat-file: {e}"))?;
    if !known.status.success() {
        return Err(format!(
            "the commit the task started at ({base}) is not in this repository"
        ));
    }
    let directory = root.join(DEBT_DIRECTORY);
    let names: BTreeSet<String> = match std::fs::read_dir(&directory) {
        Ok(listing) => listing
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name.ends_with(DEBT_SUFFIX))
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeSet::new(),
        Err(e) => return Err(format!("{}: {e}", directory.display())),
    };
    let paths: Vec<String> = names
        .iter()
        .map(|name| format!("{DEBT_DIRECTORY}/{name}"))
        .collect();
    let then = blobs_at(root, base, &paths)?;
    let mut readings = Vec::with_capacity(paths.len());
    for (path, before) in paths.into_iter().zip(then) {
        let now = std::fs::read_to_string(root.join(&path)).map_err(|e| format!("{path}: {e}"))?;
        readings.push(DebtReading {
            before: before.as_deref().map(entries),
            after: entries(&now),
            path,
        });
    }
    Ok(readings)
}

/// The text of each of `paths` at `commit`, in order; `None` for a path that commit lacks.
///
/// One `git cat-file --batch` for all of them: this is read on every `majordomus check`, and
/// a process per baseline would be paid there sixteen times over in this repository.
fn blobs_at(root: &Path, commit: &str, paths: &[String]) -> Result<Vec<Option<String>>, String> {
    use std::io::Write;
    use std::process::Stdio;

    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let mut child = crate::git::read_only(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("git cat-file: {e}"))?;
    let asked: String = paths
        .iter()
        .map(|path| format!("{commit}:{path}\n"))
        .collect();
    // the request is a few hundred bytes and is written whole before anything is read; the
    // pipe is closed by the drop, which is what ends the batch
    child
        .stdin
        .take()
        .ok_or("git cat-file: no standard input")?
        .write_all(asked.as_bytes())
        .map_err(|e| format!("git cat-file: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git cat-file: {e}"))?;
    parse_batch(&out.stdout, paths.len())
        .ok_or_else(|| "git cat-file --batch answered something that is not a batch".to_string())
}

/// The bodies of a `git cat-file --batch` answer: `<oid> <type> <size>\n<body>\n` for an
/// object that exists, one line ending in ` missing` for one that does not.
pub(crate) fn parse_batch(mut answer: &[u8], expected: usize) -> Option<Vec<Option<String>>> {
    let mut bodies = Vec::with_capacity(expected);
    for _ in 0..expected {
        let end = answer.iter().position(|b| *b == b'\n')?;
        let header = std::str::from_utf8(&answer[..end]).ok()?;
        answer = &answer[end + 1..];
        if header.ends_with(" missing") {
            bodies.push(None);
            continue;
        }
        let size: usize = header.rsplit(' ').next()?.parse().ok()?;
        let body = answer.get(..size)?;
        bodies.push(Some(String::from_utf8_lossy(body).into_owned()));
        // the body is followed by one newline of the protocol's own
        answer = answer.get(size + 1..)?;
    }
    Some(bodies)
}

/// The moment a reflog's first line records, in seconds since the epoch.
///
/// A reflog line is `<old> <new> <name> <email> <seconds> <zone>\t<message>`; the name may
/// hold spaces, so the two fields are taken from the end of what precedes the tab.
pub(crate) fn first_recorded(reflog: &str) -> Option<u64> {
    let line = reflog.lines().next()?;
    let head = line.split('\t').next()?;
    let mut fields = head.split_whitespace().rev();
    let _zone = fields.next()?;
    fields.next()?.parse().ok()
}

/// What git answers to `args` in `root`, or the reason it did not: one reading of "run git and
/// refuse a failure" for every question asked of it here.
fn git_text(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = crate::git::read_only(root)
        .args(args)
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "git {} did not answer in {}",
            args.join(" "),
            root.display()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Git's directory shared by every worktree of the repository `root` is a checkout of.
fn common_directory(root: &Path) -> Result<PathBuf, String> {
    let text = git_text(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    Ok(PathBuf::from(text.trim()))
}

/// The branches and linked worktrees created at or after `started` that still exist.
pub(crate) fn accumulated(root: &Path, started: u64) -> Result<Accumulated, String> {
    let common = common_directory(root)?;
    let listed = git_text(
        root,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )?;
    let mut found = Accumulated::default();
    for branch in listed.lines().filter(|name| !name.is_empty()) {
        found.branches_now += 1;
        let log = common.join("logs/refs/heads").join(branch);
        match std::fs::read_to_string(&log)
            .ok()
            .as_deref()
            .and_then(first_recorded)
        {
            Some(created) if created >= started => {
                found.branches.insert(branch.to_string());
            }
            Some(_) => {}
            None => found.undated += 1,
        }
    }
    // A linked worktree is a directory under <common>/worktrees/; its `gitdir` names the
    // checkout and its own HEAD reflog begins when it was added.
    let registered = match std::fs::read_dir(common.join("worktrees")) {
        Ok(listing) => listing.filter_map(|entry| entry.ok()).collect::<Vec<_>>(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(format!("{}: {e}", common.join("worktrees").display())),
    };
    for entry in registered {
        let admin = entry.path();
        let Ok(gitdir) = std::fs::read_to_string(admin.join("gitdir")) else {
            continue;
        };
        found.worktrees_now += 1;
        let checkout = gitdir.trim().trim_end_matches("/.git").to_string();
        let log = admin.join("logs/HEAD");
        match std::fs::read_to_string(&log)
            .ok()
            .as_deref()
            .and_then(first_recorded)
        {
            Some(created) if created >= started => {
                found.worktrees.insert(checkout);
            }
            Some(_) => {}
            None => found.undated += 1,
        }
    }
    Ok(found)
}

/// The pull requests of `observation` opened at or after `started`.
///
/// An observation taken before the task started cannot have seen what the task's hours
/// brought, so it is an `Err` and not an empty answer.
pub(crate) fn backlog(
    observation: Option<&ForgeObservation>,
    started: u64,
) -> Result<Backlog, String> {
    let Some(observation) = observation else {
        return Err("the forge has never been observed from this checkout".into());
    };
    let observed = crate::peers::parse_rfc3339(&observation.observed_at).ok_or_else(|| {
        format!(
            "the observation's moment '{}' could not be read",
            observation.observed_at
        )
    })?;
    if observed < started {
        return Err(format!(
            "the forge was last observed at {}, before this task started",
            observation.observed_at
        ));
    }
    let opened_since: BTreeSet<u64> = observation
        .pull_requests
        .iter()
        .filter(|pull| {
            crate::peers::parse_rfc3339(&pull.created_at).is_some_and(|at| at >= started)
        })
        .map(|pull| pull.number)
        .collect();
    Ok(Backlog {
        observed_at: observation.observed_at.clone(),
        open: observation.pull_requests.len(),
        opened_since,
    })
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .current_dir(dir)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .args([
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A set's members in its own order, to compare with what a test expects.
    fn names(set: &BTreeSet<String>) -> Vec<&str> {
        set.iter().map(String::as_str).collect()
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A repository with one debt baseline of two entries, committed; its root and that commit.
    fn repository() -> (tempfile::TempDir, PathBuf, String) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        git(&root, &["init", "-q", "-b", "master"]);
        write(
            &root.join(".ai/repo/doc-baseline.txt"),
            "# known debt, one a line\nREADME.md\tjust a\n\nREADME.md\tjust b\n",
        );
        write(&root.join(".ai/repo/policy.yaml"), "not: a baseline\n");
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "base"]);
        let base = git(&root, &["rev-parse", "HEAD"]);
        (dir, root, base)
    }

    #[test]
    fn an_entry_is_a_line_that_says_something() {
        assert_eq!(entries(""), 0);
        assert_eq!(entries("# a comment\n\n   \n"), 0);
        assert_eq!(entries("# header\na\tb\n  c\n# trailing\n"), 2);
    }

    #[test]
    fn a_baseline_is_read_at_the_start_and_now() {
        let (_dir, root, base) = repository();
        assert_eq!(
            debt(&root, &base).unwrap(),
            vec![DebtReading {
                path: ".ai/repo/doc-baseline.txt".into(),
                before: Some(2),
                after: 2
            }],
            "only a file with the suffix is a baseline"
        );
        // an entry added and not yet committed is debt already: the working tree is read
        write(
            &root.join(".ai/repo/doc-baseline.txt"),
            "# known debt, one a line\nREADME.md\tjust a\nREADME.md\tjust b\nREADME.md\tjust c\n",
        );
        let grown = debt(&root, &base).unwrap();
        assert_eq!(
            (grown[0].before, grown[0].after, grown[0].grown_by()),
            (Some(2), 3, 1)
        );
        // and one that shrank gained nothing
        write(
            &root.join(".ai/repo/doc-baseline.txt"),
            "README.md\tjust a\n",
        );
        assert_eq!(debt(&root, &base).unwrap()[0].grown_by(), 0);
    }

    #[test]
    fn a_baseline_recorded_for_the_first_time_is_not_growth() {
        let (_dir, root, base) = repository();
        write(&root.join(".ai/repo/new-gate-baseline.txt"), "a\nb\nc\n");
        let readings = debt(&root, &base).unwrap();
        let new = readings
            .iter()
            .find(|r| r.path.ends_with("new-gate-baseline.txt"))
            .unwrap();
        assert_eq!((new.before, new.after, new.grown_by()), (None, 3, 0));
    }

    #[test]
    fn a_start_that_cannot_be_read_is_an_error_and_not_an_empty_answer() {
        let (_dir, root, _) = repository();
        assert!(debt(&root, "").unwrap_err().contains("records no commit"));
        assert!(debt(&root, "NONE").is_err());
        let unknown = "0123456789012345678901234567890123456789";
        assert!(debt(&root, unknown)
            .unwrap_err()
            .contains("is not in this repository"));
    }

    #[test]
    fn a_repository_with_no_baseline_has_none() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "master"]);
        git(dir.path(), &["commit", "-q", "--allow-empty", "-m", "base"]);
        let base = git(dir.path(), &["rev-parse", "HEAD"]);
        assert_eq!(debt(dir.path(), &base).unwrap(), Vec::new());
    }

    #[test]
    fn a_batch_answer_is_bodies_and_absences_in_the_order_asked() {
        let answer = b"1111 blob 4\na\nb\n\nHEAD:nope missing\n2222 blob 0\n\n";
        assert_eq!(
            parse_batch(answer, 3).unwrap(),
            vec![Some("a\nb\n".to_string()), None, Some(String::new())]
        );
        // an answer shorter than what was asked, or one that is not a batch, is not read
        assert!(parse_batch(answer, 4).is_none());
        assert!(parse_batch(b"1111 blob 99\nshort\n", 1).is_none());
        assert!(parse_batch(b"no newline", 1).is_none());
        assert_eq!(parse_batch(b"", 0).unwrap(), Vec::new());
    }

    #[test]
    fn a_directory_that_is_no_repository_is_an_error_for_every_reading_of_git() {
        let dir = tempfile::tempdir().unwrap();
        let nowhere = dir.path();
        assert!(git_text(nowhere, &["rev-parse", "HEAD"])
            .unwrap_err()
            .contains("did not answer"));
        assert!(common_directory(nowhere).is_err());
        assert!(accumulated(nowhere, 0).is_err());
        // the debt reading meets the missing commit before anything else
        assert!(debt(nowhere, "0123456789012345678901234567890123456789").is_err());
    }

    #[test]
    fn a_debt_directory_that_cannot_be_listed_is_an_error_and_not_no_debt() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-q", "-b", "master"]);
        git(root, &["commit", "-q", "--allow-empty", "-m", "base"]);
        let base = git(root, &["rev-parse", "HEAD"]);
        // where the baselines' directory should be, there is a file
        write(&root.join(".ai/repo"), "not a directory\n");
        let refused = debt(root, &base).unwrap_err();
        assert!(refused.contains(".ai/repo"), "{refused}");
    }

    #[test]
    fn a_worktree_registry_that_cannot_be_read_or_names_nothing_is_handled() {
        let (_dir, root, _) = repository();
        // a registration with no `gitdir` names no checkout: it is skipped, not counted
        std::fs::create_dir_all(root.join(".git/worktrees/half-made")).unwrap();
        let found = accumulated(&root, 0).unwrap();
        assert_eq!((found.worktrees_now, found.worktrees.len()), (0, 0));
        // and a registry that is not a directory is an error, never "no worktrees"
        std::fs::remove_dir_all(root.join(".git/worktrees")).unwrap();
        std::fs::write(root.join(".git/worktrees"), "not a directory\n").unwrap();
        let refused = accumulated(&root, 0).unwrap_err();
        assert!(refused.contains("worktrees"), "{refused}");
    }

    #[test]
    fn the_first_line_of_a_reflog_is_when_it_began() {
        let line = "0000000000000000000000000000000000000000 1111111111111111111111111111111111111111 \
                    A Person With Spaces <a@example.com> 1788000000 +0200\tbranch: Created from HEAD\n\
                    1111 2222 A <a@example.com> 1788000999 +0200\tcommit: later\n";
        assert_eq!(first_recorded(line), Some(1_788_000_000));
        assert_eq!(first_recorded(""), None);
        assert_eq!(first_recorded("not a reflog line\n"), None);
    }

    #[test]
    fn what_was_created_since_the_start_and_remains_is_named() {
        let (_dir, root, _) = repository();
        let trunk_created = first_recorded(
            &std::fs::read_to_string(root.join(".git/logs/refs/heads/master")).unwrap(),
        )
        .unwrap();
        git(&root, &["branch", "feature/kept"]);
        let wt = root.parent().unwrap().join("wt");
        git(
            &root,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feature/held",
                wt.to_str().unwrap(),
            ],
        );

        // measured from before anything existed, everything is new
        let all = accumulated(&root, 0).unwrap();
        assert_eq!(
            names(&all.branches),
            ["feature/held", "feature/kept", "master"]
        );
        assert_eq!(names(&all.worktrees), [wt.to_str().unwrap()]);
        assert_eq!(
            (all.branches_now, all.worktrees_now, all.undated),
            (3, 1, 0)
        );

        // measured from after everything existed, nothing is — and the totals are the same
        let none = accumulated(&root, trunk_created + 86_400).unwrap();
        assert!(none.branches.is_empty() && none.worktrees.is_empty());
        assert_eq!((none.branches_now, none.worktrees_now), (3, 1));

        // what is removed again is no longer there to name
        git(&root, &["worktree", "remove", wt.to_str().unwrap()]);
        git(&root, &["branch", "-D", "feature/held", "feature/kept"]);
        let after = accumulated(&root, 0).unwrap();
        assert_eq!(names(&after.branches), ["master"]);
        assert!(after.worktrees.is_empty());
    }

    #[test]
    fn a_branch_git_has_no_creation_record_for_is_counted_on_neither_side() {
        let (_dir, root, _) = repository();
        git(&root, &["branch", "feature/old"]);
        std::fs::remove_file(root.join(".git/logs/refs/heads/feature/old")).unwrap();
        let found = accumulated(&root, 0).unwrap();
        assert_eq!(names(&found.branches), ["master"]);
        assert_eq!((found.branches_now, found.undated), (2, 1));
    }

    fn observation(observed_at: &str, opened: &[(u64, &str)]) -> ForgeObservation {
        let pulls: Vec<serde_json::Value> = opened
            .iter()
            .map(|(number, created_at)| {
                serde_json::json!({
                    "number": number, "title": "t", "body": "", "author": "a",
                    "author_association": "OWNER", "draft": false, "auto_merge": false,
                    "head_ref": format!("feature/{number}"), "head_sha": "0".repeat(40),
                    "base_ref": "master", "cross_repository": false,
                    "created_at": created_at, "updated_at": created_at,
                    "labels": [], "checks": [], "latest_reviews": [], "review_requests": [],
                    "review_decision": "", "cross_references": "whole",
                })
            })
            .collect();
        serde_json::from_value(serde_json::json!({
            "schema": crate::integration::forge::OBSERVATION_SCHEMA,
            "repository": "o/r", "base": "master", "base_sha": "0".repeat(40),
            "observed_at": observed_at, "required_checks": null, "review_policy": null,
            "up_to_date_required": null, "merge_methods": ["merge"],
            "pull_requests": pulls, "resolved": {}, "delete_branch_on_merge": null,
        }))
        .unwrap()
    }

    #[test]
    fn the_backlog_names_what_was_opened_since_the_start() {
        let started = crate::peers::parse_rfc3339("2026-10-10T12:00:00Z").unwrap();
        let seen = observation(
            "2026-10-10T15:00:00Z",
            &[
                (7, "2026-10-09T08:00:00Z"),
                (9, "2026-10-10T13:00:00Z"),
                (8, "2026-10-10T12:00:00Z"),
            ],
        );
        let read = backlog(Some(&seen), started).unwrap();
        assert_eq!(read.open, 3);
        assert_eq!(
            read.opened_since.iter().copied().collect::<Vec<_>>(),
            [8, 9]
        );
        assert_eq!(read.observed_at, "2026-10-10T15:00:00Z");
    }

    #[test]
    fn an_observation_that_cannot_answer_is_an_error() {
        let started = crate::peers::parse_rfc3339("2026-10-10T12:00:00Z").unwrap();
        assert!(backlog(None, started)
            .unwrap_err()
            .contains("never been observed"));
        let stale = observation("2026-10-10T11:59:59Z", &[]);
        assert!(backlog(Some(&stale), started)
            .unwrap_err()
            .contains("before this task started"));
        let unread = observation("yesterday", &[]);
        assert!(backlog(Some(&unread), started)
            .unwrap_err()
            .contains("could not be read"));
    }

    #[test]
    fn every_reading_is_taken_on_its_own() {
        let (_dir, root, base) = repository();
        // a starting moment that is not a time loses the two readings measured from it and
        // keeps the one measured from the commit
        let read = read(&root, &base, "this morning");
        assert!(read.debt.is_ok());
        assert!(read.accumulated.unwrap_err().contains("not an RFC 3339"));
        assert!(read.backlog.is_err());
        // with a real moment, a checkout that never observed the forge says exactly that
        let read = super::read(&root, &base, "2026-10-10T12:00:00Z");
        assert_eq!(read.since, "2026-10-10T12:00:00Z");
        assert!(read.accumulated.is_ok());
        assert!(read.backlog.unwrap_err().contains("never been observed"));
    }
}
