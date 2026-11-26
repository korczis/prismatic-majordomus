//! Supersede, don't rewrite: a published range replayed onto a new branch with new messages.
//!
//! A pushed commit whose message must change cannot be fixed in place. The forge holds the
//! branch, CI runs hold its head, and a protected branch refuses the force push anyway. The
//! repair is a new branch that carries the same trees with the reworded messages, and a pull
//! request that declares `Supersedes #N`. The integration engine then holds the old pull
//! request `waiting_for_dependency` and `prs cleanup --apply` closes it once the successor
//! lands (docs/INTEGRATION.md, "Supersession markers").
//!
//! This module plans that replay and carries it out. The plan, [`plan`], reads git and writes
//! nothing. It lists every first-parent commit of `<base>..<head>` with the message it will
//! carry. It judges each message against the repository's commit policy, with that commit's
//! own paths. It judges every non-merge commit the successor will carry and cannot reword,
//! one that arrives through a kept second parent, because the forge judges those too. And it
//! composes the successor's body. [`apply`] replays a plan that passed with `git commit-tree`:
//!
//! - same tree, same author, same author date;
//! - each new first parent the replayed one, every other parent unchanged;
//! - the new branch created atomically;
//! - no existing ref moved.
//!
//! The remote half (push, open the successor, take the old one out of auto-merge) stays with
//! the person and `gh`, because no machine surface of this executable mutates a forge outside
//! the integration executor.
//!
//! ```
//! use majordomus_cli::commit::reword::{successor_body, without_successor_lines};
//! let body = successor_body(7, Some("https://example.org/pr/7"), "Why.\n\nSuperseded by #9\n");
//! assert!(body.starts_with("Supersedes #7\n"));
//! assert!(!body.contains("Superseded by #9"));
//! assert_eq!(without_successor_lines("a\nSuperseded by #3\nb"), "a\nb");
//! ```

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::policy::CommitPolicy;
use super::verdict::{judge, CommitSubject};
use crate::model::Diagnostic;

/// What to replay and how: the range, the new messages, and what the successor supersedes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
/// ```
/// use majordomus_cli::commit::reword::RewordInput;
/// let i: RewordInput = serde_json::from_str(
///     r#"{"range": "origin/master..HEAD", "messages": {"HEAD~1": "Fix: [X] Y"}}"#).unwrap();
/// assert!(i.trunk.is_none() && i.supersedes.is_none());
/// ```
pub struct RewordInput {
    /// `<base>..<head>`: the commits after `base` on `head`'s first-parent line are replayed,
    /// and `base` stays the base.
    pub range: String,
    /// The new message of each commit to reword, by any revision git resolves to a commit of
    /// the range. A commit not named keeps its message, and is judged all the same.
    #[serde(default)]
    pub messages: BTreeMap<String, String>,
    /// The trunk the forge compares the pull request with, for the commits a merge brings in
    /// from elsewhere; `base` when absent. Name it (`origin/master`) when the branch merges
    /// the trunk in, so that the trunk's own commits are not judged as the branch's.
    #[serde(default)]
    pub trunk: Option<String>,
    /// The pull request the successor supersedes.
    #[serde(default)]
    pub supersedes: Option<u64>,
    /// Its address, for the successor's body.
    #[serde(default)]
    pub url: Option<String>,
    /// Its body, carried into the successor's.
    #[serde(default)]
    pub body: Option<String>,
}

/// One commit of the replay.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RewordCommit {
    /// The commit replayed.
    pub commit: String,
    /// Its parents, first parent first.
    pub parents: Vec<String>,
    /// Whether it gets a new message.
    pub reworded: bool,
    /// The message it will carry, as git will store it.
    pub message: String,
    /// Whether that message passes the commit policy.
    pub passed: bool,
    /// What the judge found in it.
    pub findings: Vec<Diagnostic>,
}

/// A commit the successor carries unchanged through a kept parent, and whose message fails.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UnrewordableCommit {
    /// The commit.
    pub commit: String,
    /// Its subject.
    pub subject: String,
    /// What the judge found in it.
    pub findings: Vec<Diagnostic>,
}

/// The replay, planned: every commit with its message and verdict, and the successor's body.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RewordPlan {
    /// The base, resolved.
    pub base: String,
    /// The head, resolved.
    pub head: String,
    /// The first-parent commits of the range, oldest first.
    pub commits: Vec<RewordCommit>,
    /// Failing commits that only a kept second parent carries, which no rewording reaches.
    pub unrewordable: Vec<UnrewordableCommit>,
    /// Why the plan cannot be applied, one sentence each; empty when it can.
    pub problems: Vec<String>,
    /// Whether the plan can be applied: no problem, every message passing.
    pub passed: bool,
    /// The successor pull request's body, when the plan names what it supersedes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub successor_body: Option<String>,
}

/// Run git in `root`; its standard output when it succeeds.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run git in `root` with `input` on standard input and `env` set; its output or its error.
fn git_with(
    root: &Path,
    args: &[&str],
    input: &str,
    env: &[(&str, &str)],
) -> Result<String, String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("git could not be run: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.as_bytes())
            .map_err(|e| format!("git {args:?}: {e}"))?;
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("git {args:?}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// The full name of the commit a revision names, or `None` when git cannot name exactly one.
fn commit_of(root: &Path, rev: &str) -> Option<String> {
    git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ],
    )
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
}

/// A message as `git commit` would store it: comment lines and trailing whitespace removed,
/// blank lines collapsed. What is judged is then byte for byte what is stored.
fn stored(root: &Path, message: &str) -> Result<String, String> {
    git_with(root, &["stripspace", "--strip-comments"], message, &[])
}

/// A commit's message exactly as stored: everything after the first blank line of the raw
/// object. `git log --format=%B` adds a line of its own and would not round-trip.
fn raw_message(root: &Path, commit: &str) -> String {
    git(root, &["cat-file", "commit", commit])
        .and_then(|raw| raw.split_once("\n\n").map(|(_, m)| m.to_string()))
        .unwrap_or_default()
}

/// The paths one commit changes against its first parent.
fn paths_of(root: &Path, commit: &str) -> Vec<String> {
    git(
        root,
        &[
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "-m",
            "--first-parent",
            commit,
        ],
    )
    .unwrap_or_default()
    .lines()
    .filter(|l| !l.is_empty())
    .map(str::to_string)
    .collect()
}

/// Lines a successor's body must not inherit: a `Superseded by #N` the old body said about
/// itself is not true of its successor.
pub fn without_successor_lines(body: &str) -> String {
    body.lines()
        .filter(|l| {
            !l.trim_start()
                .to_ascii_lowercase()
                .starts_with("superseded by #")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The successor's body. The `Supersedes #N` marker comes first, on a line of its own, so
/// that nothing of the old body (an open fence, a quote) can turn it into someone else's
/// words. The old body follows whole, without the lines that described its own successor.
pub fn successor_body(number: u64, url: Option<&str>, old: &str) -> String {
    let link = url.map(|u| format!(" ({u})")).unwrap_or_default();
    let mut out = format!(
        "Supersedes #{number}\n\nThis pull request carries the commits of #{number}{link} with reworded \
         messages and the same trees: nothing but the messages differs. #{number} was not \
         force-pushed; it stays open until this one lands, and `majordomus prs cleanup --apply` \
         then closes it.\n"
    );
    let old = without_successor_lines(old);
    if !old.trim().is_empty() {
        out.push_str(&format!("\n## The description of #{number}\n\n"));
        out.push_str(old.trim_end());
        out.push('\n');
    }
    out
}

/// Plan the replay of `input.range`. Reads git; writes nothing.
pub fn plan(
    root: &Path,
    input: &RewordInput,
    policy: &CommitPolicy,
    scopes: Option<Vec<String>>,
    records: &[String],
) -> Result<RewordPlan, String> {
    let (base_rev, head_rev) = input
        .range
        .split_once("..")
        .filter(|(b, h)| !b.is_empty() && !h.is_empty() && !h.starts_with('.'))
        .ok_or_else(|| format!("'{}' is not `<base>..<head>`", input.range))?;
    let base = commit_of(root, base_rev)
        .ok_or_else(|| format!("git does not name one commit by '{base_rev}'"))?;
    let head = commit_of(root, head_rev)
        .ok_or_else(|| format!("git does not name one commit by '{head_rev}'"))?;
    let mut problems = Vec::new();
    let ancestor = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["merge-base", "--is-ancestor", &base, &head])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ancestor {
        problems.push(format!("{base_rev} is not an ancestor of {head_rev}"));
    }
    let listing = git(
        root,
        &[
            "rev-list",
            "--first-parent",
            "--reverse",
            "--parents",
            &format!("{base}..{head}"),
        ],
    )
    .ok_or_else(|| format!("git cannot list {}", input.range))?;
    let line: Vec<Vec<String>> = listing
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.split(' ').map(str::to_string).collect())
        .collect();
    if line.is_empty() {
        problems.push(format!("{} holds no commit", input.range));
    }
    let on_line: Vec<&String> = line.iter().map(|c| &c[0]).collect();

    let mut replacements: BTreeMap<String, String> = BTreeMap::new();
    for (rev, text) in &input.messages {
        match commit_of(root, rev) {
            Some(c) if on_line.contains(&&c) => {
                replacements.insert(c, text.clone());
            }
            Some(c) => problems.push(format!(
                "{rev} ({}) is not a first-parent commit of {}",
                &c[..12.min(c.len())],
                input.range
            )),
            None => problems.push(format!(
                "git does not name one commit by '{rev}' (unknown, or an ambiguous short name)"
            )),
        }
    }

    let judged = |text: &str, paths: Option<Vec<String>>| {
        judge(
            &CommitSubject {
                text: text.to_string(),
                scopes: scopes.clone(),
                paths,
                records: records.to_vec(),
            },
            policy,
        )
    };

    let mut commits = Vec::new();
    for entry in &line {
        let commit = &entry[0];
        let parents: Vec<String> = entry[1..].to_vec();
        let reworded = replacements.contains_key(commit);
        let message = match replacements.get(commit) {
            Some(text) => stored(root, text)?,
            None => raw_message(root, commit),
        };
        let paths = (parents.len() == 1).then(|| paths_of(root, commit));
        let verdict = judged(&message, paths);
        commits.push(RewordCommit {
            commit: commit.clone(),
            parents,
            reworded,
            message,
            passed: verdict.passed,
            findings: verdict.findings,
        });
    }

    // What a kept second parent brings in is carried unchanged; the forge judges it all the
    // same, so a failing message there is a successor that fails again.
    let trunk = match &input.trunk {
        Some(t) => {
            commit_of(root, t).ok_or_else(|| format!("git does not name one commit by '{t}'"))?
        }
        None => base.clone(),
    };
    let mut unrewordable = Vec::new();
    for c in git(
        root,
        &["rev-list", "--no-merges", &format!("{trunk}..{head}")],
    )
    .unwrap_or_default()
    .lines()
    .filter(|l| !l.is_empty() && !on_line.iter().any(|o| o.as_str() == *l))
    {
        let text = raw_message(root, c);
        let verdict = judged(&text, None);
        if !verdict.passed {
            unrewordable.push(UnrewordableCommit {
                commit: c.to_string(),
                subject: text.lines().next().unwrap_or_default().to_string(),
                findings: verdict.findings,
            });
        }
    }
    if !unrewordable.is_empty() {
        problems.push(format!(
            "{} commit(s) reach the branch through a merge's other parent and fail the policy; \
             rewording the first-parent line cannot change them (name --trunk if they are the trunk's)",
            unrewordable.len()
        ));
    }

    let passed = problems.is_empty() && commits.iter().all(|c| c.passed);
    let successor_body = input.supersedes.map(|n| {
        successor_body(
            n,
            input.url.as_deref(),
            input.body.as_deref().unwrap_or_default(),
        )
    });
    Ok(RewordPlan {
        base,
        head,
        commits,
        unrewordable,
        problems,
        passed,
        successor_body,
    })
}

/// What [`apply`] made.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Applied {
    /// The new branch.
    pub branch: String,
    /// Its head.
    pub head: String,
    /// Old commit to new commit, in replay order.
    pub replayed: Vec<(String, String)>,
    /// What the replay could not carry over, by name (a signature, a merged tag's header).
    pub dropped: Vec<String>,
}

/// Replay a plan that passed onto the new branch `branch`. Refuses a plan that did not pass,
/// a branch that exists, and a replay whose trees differ from the original's; moves no ref
/// that exists.
pub fn apply(root: &Path, plan: &RewordPlan, branch: &str) -> Result<Applied, String> {
    if !plan.passed {
        return Err("the plan does not pass; nothing was written".into());
    }
    let refname = format!("refs/heads/{branch}");
    if git(root, &["check-ref-format", &refname]).is_none() {
        return Err(format!("'{branch}' is not a branch name git accepts"));
    }
    if commit_of(root, &refname).is_some() {
        return Err(format!(
            "the branch {branch} exists; a replay never moves a ref"
        ));
    }
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    let mut replayed = Vec::new();
    let mut dropped = Vec::new();
    for c in &plan.commits {
        let raw = git(root, &["cat-file", "commit", &c.commit]).unwrap_or_default();
        let headers = raw.split("\n\n").next().unwrap_or_default();
        for (key, what) in [
            ("gpgsig", "signature"),
            ("gpgsig-sha256", "signature"),
            ("mergetag", "merged tag header"),
            ("encoding", "encoding header"),
        ] {
            if headers.lines().any(|l| l.starts_with(&format!("{key} "))) {
                dropped.push(format!(
                    "{} of {}",
                    what,
                    &c.commit[..12.min(c.commit.len())]
                ));
            }
        }
        let tree = git(root, &["rev-parse", &format!("{}^{{tree}}", c.commit)])
            .map(|s| s.trim().to_string())
            .ok_or_else(|| format!("git cannot read the tree of {}", c.commit))?;
        let who = git(
            root,
            &[
                "log",
                "-1",
                "--format=%an%x00%ae%x00%ad",
                "--date=raw",
                &c.commit,
            ],
        )
        .unwrap_or_default();
        let mut fields = who.trim_end_matches('\n').split('\0');
        let (name, email, date) = (
            fields.next().unwrap_or_default().to_string(),
            fields.next().unwrap_or_default().to_string(),
            fields.next().unwrap_or_default().to_string(),
        );
        let mut args: Vec<String> = vec!["commit-tree".into(), tree.clone()];
        for p in &c.parents {
            args.push("-p".into());
            args.push(map.get(p).cloned().unwrap_or_else(|| p.clone()));
        }
        args.push("-F".into());
        args.push("-".into());
        let argv: Vec<&str> = args.iter().map(String::as_str).collect();
        let new = git_with(
            root,
            &argv,
            &c.message,
            &[
                ("GIT_AUTHOR_NAME", name.as_str()),
                ("GIT_AUTHOR_EMAIL", email.as_str()),
                ("GIT_AUTHOR_DATE", date.as_str()),
            ],
        )?
        .trim()
        .to_string();
        let new_tree = git(root, &["rev-parse", &format!("{new}^{{tree}}")])
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if new_tree != tree {
            return Err(format!(
                "the replay of {} carries another tree; nothing was written",
                c.commit
            ));
        }
        map.insert(c.commit.clone(), new.clone());
        replayed.push((c.commit.clone(), new));
    }
    let new_head = replayed
        .last()
        .map(|(_, n)| n.clone())
        .ok_or("the plan replays no commit")?;
    git_with(
        root,
        &[
            "update-ref",
            &refname,
            &new_head,
            "0000000000000000000000000000000000000000",
        ],
        "",
        &[],
    )
    .map_err(|e| {
        format!("{e}; the branch {branch} appeared while the replay ran, and was left as it is")
    })?;
    Ok(Applied {
        branch: branch.to_string(),
        head: new_head,
        replayed,
        dropped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Repo(tempfile::TempDir);

    impl Repo {
        fn new() -> Repo {
            let r = Repo(tempfile::tempdir().expect("a temporary directory"));
            r.git(&["init", "-q", "-b", "trunk"]);
            // the replay commits as whoever this clone says, and signs as its config says
            r.git(&["config", "user.name", "Committer"]);
            r.git(&["config", "user.email", "c@example.org"]);
            r.git(&["config", "commit.gpgsign", "false"]);
            r
        }
        fn path(&self) -> &Path {
            self.0.path()
        }
        fn git(&self, args: &[&str]) -> String {
            let out = Command::new("git")
                .arg("-C")
                .arg(self.path())
                .args([
                    "-c",
                    "user.name=Author",
                    "-c",
                    "user.email=a@example.org",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(args)
                .output()
                .expect("git runs");
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        }
        fn commit(&self, file: &str, message: &str) -> String {
            std::fs::write(self.path().join(file), message).unwrap();
            self.git(&["add", "-A"]);
            self.git(&["commit", "-q", "-m", message]);
            self.git(&["rev-parse", "HEAD"])
        }
    }

    fn keyword() -> CommitPolicy {
        serde_json::from_value(serde_json::json!({
            "grammar": {"style": "keyword", "words": [{"word": "Fix", "kind": "fix"}, {"word": "Codechange", "kind": "refactor"}]}
        }))
        .unwrap()
    }

    #[test]
    fn a_reworded_range_keeps_every_tree_and_moves_no_ref() {
        let r = Repo::new();
        let base = r.commit("a", "Fix: [A] The base");
        let bad = r.commit("b", "Test: [B] A test");
        r.git(&["checkout", "-q", "-b", "side", &base]);
        r.commit("t", "Fix: [T] On the trunk side");
        r.git(&["checkout", "-q", "trunk"]);
        r.git(&[
            "merge",
            "-q",
            "--no-ff",
            "-m",
            "Merge branch 'side'",
            "side",
        ]);
        let last = r.commit("c", "Fix: [C] After the merge");
        let range = format!("{base}..{last}");
        let mut input = RewordInput {
            range: range.clone(),
            supersedes: Some(7),
            url: Some("https://example.org/7".into()),
            body: Some("The why.\n\nSuperseded by #3\n".into()),
            ..RewordInput::default()
        };
        // unreworded, the bad message refuses the plan, and nothing is written
        let p = plan(r.path(), &input, &keyword(), None, &[]).unwrap();
        assert!(!p.passed);
        assert!(apply(r.path(), &p, "reworded").is_err());
        // the new message is stored as git would store it, comment and trailing space gone
        input.messages.insert(
            bad.clone(),
            "Codechange: [B] A test  \n# a comment\n".into(),
        );
        let p = plan(r.path(), &input, &keyword(), None, &[]).unwrap();
        assert!(p.passed, "{:?} {:?}", p.problems, p.commits);
        assert_eq!(
            p.commits.len(),
            3,
            "first-parent: the reworded one, the merge, the last"
        );
        assert_eq!(p.commits[0].message, "Codechange: [B] A test\n");
        let body = p.successor_body.clone().unwrap();
        assert!(body.starts_with("Supersedes #7\n") && body.contains("https://example.org/7"));
        assert!(body.contains("The why.") && !body.contains("Superseded by #3"));

        let old_head = r.git(&["rev-parse", "trunk"]);
        let done = apply(r.path(), &p, "reworded").unwrap();
        assert_eq!(
            r.git(&["rev-parse", "trunk"]),
            old_head,
            "the old ref did not move"
        );
        assert_eq!(
            r.git(&["rev-parse", "reworded^{tree}"]),
            r.git(&["rev-parse", "trunk^{tree}"])
        );
        assert_eq!(
            r.git(&["log", "-1", "--format=%s", "reworded~2"]),
            "Codechange: [B] A test"
        );
        assert_eq!(
            r.git(&["log", "-1", "--format=%an <%ae>", "reworded~2"]),
            "Author <a@example.org>"
        );
        assert_eq!(
            r.git(&["log", "-1", "--format=%ad", "--date=raw", "reworded~2"]),
            r.git(&["log", "-1", "--format=%ad", "--date=raw", &bad])
        );
        // the merge kept its other parent, so the trunk side is still the same commit
        assert_eq!(
            r.git(&["rev-parse", "reworded~1^2"]),
            r.git(&["rev-parse", "side"])
        );
        assert_eq!(done.replayed.len(), 3);
        // an untouched message is carried byte for byte
        assert_eq!(
            r.git(&["cat-file", "-p", "reworded"])
                .split_once("\n\n")
                .unwrap()
                .1,
            r.git(&["cat-file", "-p", &last])
                .split_once("\n\n")
                .unwrap()
                .1
        );
        assert!(done.dropped.is_empty());
        // and a second apply refuses, because the branch now exists
        assert!(apply(r.path(), &p, "reworded")
            .unwrap_err()
            .contains("exists"));
    }

    #[test]
    fn a_failing_commit_behind_a_kept_parent_is_named_and_refuses_the_plan() {
        let r = Repo::new();
        let base = r.commit("a", "Fix: [A] The base");
        r.git(&["checkout", "-q", "-b", "side"]);
        r.commit("s", "Test: [S] Not a word");
        r.git(&["checkout", "-q", "trunk"]);
        r.git(&[
            "merge",
            "-q",
            "--no-ff",
            "-m",
            "Merge branch 'side'",
            "side",
        ]);
        let head = r.git(&["rev-parse", "HEAD"]);
        let input = RewordInput {
            range: format!("{base}..{head}"),
            ..RewordInput::default()
        };
        let p = plan(r.path(), &input, &keyword(), None, &[]).unwrap();
        assert!(!p.passed);
        assert_eq!(p.unrewordable.len(), 1);
        assert_eq!(p.unrewordable[0].subject, "Test: [S] Not a word");
        // naming the trunk that already holds it takes it out of the branch's own set
        let input = RewordInput {
            trunk: Some(head.clone()),
            ..input
        };
        let p = plan(r.path(), &input, &keyword(), None, &[]).unwrap();
        assert!(p.unrewordable.is_empty() && p.passed, "{:?}", p.problems);
    }

    #[test]
    fn a_revision_outside_the_range_and_a_malformed_range_are_refused() {
        let r = Repo::new();
        let base = r.commit("a", "Fix: [A] The base");
        let one = r.commit("b", "Fix: [B] One");
        let input = RewordInput {
            range: format!("{one}..{one}"),
            messages: BTreeMap::from([(base.clone(), "Fix: [A] Again".to_string())]),
            ..RewordInput::default()
        };
        let p = plan(r.path(), &input, &keyword(), None, &[]).unwrap();
        assert!(!p.passed);
        assert!(
            p.problems.iter().any(|m| m.contains("holds no commit")),
            "{:?}",
            p.problems
        );
        assert!(
            p.problems
                .iter()
                .any(|m| m.contains("is not a first-parent commit")),
            "{:?}",
            p.problems
        );
        let unknown = RewordInput {
            range: format!("{base}..{one}"),
            messages: BTreeMap::from([("no-such-rev".to_string(), "Fix: x".to_string())]),
            ..RewordInput::default()
        };
        let p = plan(r.path(), &unknown, &keyword(), None, &[]).unwrap();
        assert!(
            p.problems.iter().any(|m| m.contains("no-such-rev")),
            "{:?}",
            p.problems
        );
        let backwards = RewordInput {
            range: format!("{one}..{base}"),
            ..RewordInput::default()
        };
        let p = plan(r.path(), &backwards, &keyword(), None, &[]).unwrap();
        assert!(
            p.problems.iter().any(|m| m.contains("is not an ancestor")),
            "{:?}",
            p.problems
        );
        for bad in ["HEAD", "..HEAD", "a...b"] {
            let i = RewordInput {
                range: bad.into(),
                ..RewordInput::default()
            };
            assert!(plan(r.path(), &i, &keyword(), None, &[]).is_err(), "{bad}");
        }
        assert!(plan(
            r.path(),
            &RewordInput {
                range: "nope..HEAD".into(),
                ..RewordInput::default()
            },
            &keyword(),
            None,
            &[]
        )
        .is_err());
        assert!(plan(
            r.path(),
            &RewordInput {
                range: format!("{base}..nope"),
                ..RewordInput::default()
            },
            &keyword(),
            None,
            &[]
        )
        .is_err());
        let trunk = RewordInput {
            range: format!("{base}..{one}"),
            trunk: Some("nope".into()),
            ..RewordInput::default()
        };
        assert!(plan(r.path(), &trunk, &keyword(), None, &[]).is_err());
    }

    #[test]
    fn apply_refuses_a_name_git_does_not_accept() {
        let r = Repo::new();
        let base = r.commit("a", "Fix: [A] The base");
        let one = r.commit("b", "Fix: [B] One");
        let p = plan(
            r.path(),
            &RewordInput {
                range: format!("{base}..{one}"),
                ..RewordInput::default()
            },
            &keyword(),
            None,
            &[],
        )
        .unwrap();
        assert!(p.passed);
        assert!(apply(r.path(), &p, "bad..name")
            .unwrap_err()
            .contains("not a branch name"));
        assert!(
            p.successor_body.is_none(),
            "no successor body unless the plan supersedes something"
        );
    }

    #[test]
    fn a_body_with_nothing_of_its_own_is_only_the_marker_and_the_sentence() {
        let b = successor_body(3, None, "  \n");
        assert!(b.starts_with("Supersedes #3\n"));
        assert!(!b.contains("## The description"));
        assert!(!b.contains("()"));
    }
}
