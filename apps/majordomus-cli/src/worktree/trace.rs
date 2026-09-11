//! The work graph above the branch, derived from git and stored nowhere.
//!
//! [`state::issue_of`](super::state::issue_of) reads an issue id out of a branch name, and
//! until now that was the whole model: a branch knew which issue it served and nothing knew
//! which commits served it. This module continues the same edge upwards and downwards
//! without adding a second store of truth.
//!
//! ```text
//! issue  ──names──▶  branch  ──contains──▶  commit
//!   ▲                   │                      │
//!   └───────────────────┴──────────────────────┘
//!            the same edge, read backwards
//! ```
//!
//! # What is derived from what
//!
//! An issue's branches are the refs — local and remote-tracking — with a path component
//! that names it. A branch's commits are the commits it holds and the trunk did not:
//! `branch ^trunk` while the branch is open, and `branch ^merge^1` once a merge commit has
//! brought it in, where the merge commit is the earliest trunk commit descended from the
//! branch. A commit's issue is the issue whose branch set contains it. Nothing is written
//! down, because git already holds every one of those facts and a record that repeated one
//! would be a second truth to keep current.
//!
//! # What git cannot answer, and says so
//!
//! Three states are honest answers rather than failures, and each is reported by name:
//!
//! * [`Integration::Absorbed`] — the branch reached the trunk with no merge commit of its
//!   own (a fast-forward, or a rebase-and-merge). Its commits are indistinguishable from
//!   the trunk's, so none are claimed.
//! * [`Attribution::Unattributed`] — no ref naming an issue contains the commit. A branch
//!   deleted after its merge takes its own name with it; the head branch of a merged pull
//!   request survives on GitHub, which is why the pull-request half of this derivation is
//!   `scripts/traceability` and not this file — the executable makes no network call.
//! * [`Attribution::Ambiguous`] — two branches naming two different issues contain it.
//!
//! An unattributed commit is reported, never omitted: work with no execution contract is
//! exactly what a traceability report exists to make visible.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::error::Result;
use super::git;
use super::identity::RepositoryIdentity;
use super::state::{issue_ids, issue_of};

/// The most commits one branch claims. A branch with more than this is a history import,
/// not a piece of work, and the trace says it was cut rather than pretending it was not.
pub const MAX_COMMITS_PER_BRANCH: usize = 1000;

/// How many trunk commits a report attributes when the caller names no number.
pub const DEFAULT_REPORT_COMMITS: usize = 50;

/// The most trunk commits one report attributes.
pub const MAX_REPORT_COMMITS: usize = 2000;

/// The separator between the fields of one commit; a commit message may hold anything else.
const SEP: char = '\u{0}';

/// The pretty format every commit in this module is read with.
const FORMAT: &str = "--format=%H%x00%h%x00%an%x00%aI%x00%s";

/// One commit, exactly as git names it. Nothing here is stored anywhere: the whole record
/// is re-read from the object database on every call.
///
/// Both object names are kept. The full one is the identity, and the abbreviated one is
/// what this repository abbreviates to *today* — an abbreviation is only unique against
/// the object store that produced it, so it is carried as a rendering and never compared.
/// The date is the string git recorded and is not parsed here: nothing in this module
/// orders commits by date, git orders them by ancestry, and a parsed timestamp would
/// invite somebody to.
///
/// ```
/// use majordomus_cli::worktree::{parse_commit, CommitRef};
/// let line = "abc123\0abc\0A Person\02026-09-09T10:00:00+02:00\0feat: a thing";
/// let c: CommitRef = parse_commit(line).unwrap();
/// assert_eq!(c.id, "abc123");
/// assert_eq!(c.short, "abc", "the abbreviation is carried, never recomputed");
/// assert_eq!(c.subject, "feat: a thing");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommitRef {
    /// The full object name.
    pub id: String,
    /// The abbreviated object name, as this repository abbreviates it.
    pub short: String,
    /// The author, as the commit records them.
    pub author: String,
    /// The author date, ISO 8601 as the commit records it.
    pub date: String,
    /// The subject line.
    pub subject: String,
}

/// Parse one commit line of `FORMAT`. A line with fewer fields is not a commit.
///
/// ```
/// use majordomus_cli::worktree::parse_commit;
/// let c = parse_commit("abc123\0abc\0A Person\02026-09-09T10:00:00+02:00\0feat: a thing").unwrap();
/// assert_eq!(c.short, "abc");
/// assert_eq!(c.subject, "feat: a thing");
/// assert!(parse_commit("abc123").is_none());
/// ```
pub fn parse_commit(line: &str) -> Option<CommitRef> {
    let f: Vec<&str> = line.split(SEP).collect();
    if f.len() < 5 || f[0].is_empty() {
        return None;
    }
    Some(CommitRef {
        id: f[0].to_string(),
        short: f[1].to_string(),
        author: f[2].to_string(),
        date: f[3].to_string(),
        subject: f[4..].join(""),
    })
}

/// Every commit in the output of a `git log` run with [`FORMAT`].
fn parse_commits(text: &str) -> Vec<CommitRef> {
    text.lines().filter_map(parse_commit).collect()
}

/// How a branch stands to the trunk, which is what decides which commits are its own.
///
/// The four cases are not degrees of the same thing; they are four different derivations.
/// An open branch's commits are what the trunk lacks. A merged branch's are what its merge
/// commit brought in. An absorbed branch has none that can be told from the trunk's, and
/// saying so is the honest answer rather than claiming the trunk's recent commits. And with
/// no trunk there is nothing to measure against at all.
///
/// ```
/// use majordomus_cli::worktree::Integration;
/// assert_eq!(serde_json::to_string(&Integration::Absorbed).unwrap(), "\"absorbed\"");
/// let unknown: Integration = serde_json::from_str("\"unknown\"").unwrap();
/// assert_eq!(unknown, Integration::Unknown, "no trunk is a state, not a failure");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Integration {
    /// Not reachable from the trunk: its commits are the ones the trunk does not have.
    Open,
    /// Reachable from the trunk through a merge commit, which the trace names: its commits
    /// are the ones that merge brought in.
    Merged,
    /// Reachable from the trunk with no merge commit of its own — fast-forwarded, or
    /// rebased onto it. Its commits cannot be told from the trunk's and none are claimed.
    Absorbed,
    /// The trunk is unknown, so there is nothing to measure the branch against.
    Unknown,
}

/// One branch that names an issue, with the commits it holds.
///
/// A ref rather than a branch, and `remote` says which: a branch deleted locally after its
/// merge still exists as `origin/...`, and dropping it would lose the work it carried. The
/// commit list is derived from `integration` and is empty for good reasons as often as for
/// bad ones — an absorbed branch has nothing distinguishable to claim — so `note` carries
/// the sentence that stops an empty list from reading as a defect.
///
/// ```
/// use majordomus_cli::worktree::{BranchTrace, Integration};
/// let absorbed = BranchTrace {
///     name: "origin/feature/I1305-traceability".into(),
///     remote: true,
///     head: "abc123".into(),
///     integration: Integration::Absorbed,
///     merge_commit: None,
///     note: Some("fast-forwarded onto the trunk; its commits are the trunk's".into()),
///     commits: Vec::new(),
/// };
/// assert!(absorbed.remote, "only the remote still has this branch");
/// assert!(absorbed.commits.is_empty() && absorbed.note.is_some(), "empty, and explained");
/// let wire = serde_json::to_value(&absorbed).unwrap();
/// assert!(wire.get("merge_commit").is_none(), "an absorbed branch has no merge commit");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BranchTrace {
    /// The ref, short: `feature/I1305-traceability`, or `origin/feature/I1305-traceability`
    /// when only the remote still has it.
    pub name: String,
    /// True when the ref is a remote-tracking one and no local branch of the same name
    /// stands for it.
    pub remote: bool,
    /// The commit the ref points at.
    pub head: String,
    /// How it stands to the trunk.
    pub integration: Integration,
    /// The merge commit that brought it into the trunk, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_commit: Option<String>,
    /// Why the commit list is what it is, when it is worth a sentence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The commits this branch holds and the trunk did not, newest first, merges excluded.
    pub commits: Vec<CommitRef>,
}

/// Everything git knows about one issue's realisation.
///
/// Three fields exist to keep apart the ways of knowing nothing, and they are the whole
/// value of the type. `declared` separates "the project model has no such issue" from "it
/// has one and nothing has started it". `complete` separates "these are all the commits"
/// from "some of the work is absorbed into the trunk and cannot be told apart". And
/// `milestone` is left for a caller that has the index, because git does not hold that
/// edge and inventing it here would be a second source of truth.
///
/// ```
/// use majordomus_cli::worktree::IssueTrace;
/// let unstarted = IssueTrace {
///     issue: "I1305".into(),
///     declared: true,
///     milestone: None,
///     trunk: Some("master".into()),
///     branches: Vec::new(),
///     commits: 0,
///     complete: true,
/// };
/// // declared and unrealised: not an error, and not the same as an unknown id
/// assert!(unstarted.declared && unstarted.branches.is_empty());
/// assert!(unstarted.complete, "nothing was derived, and nothing was lost either");
/// let wire = serde_json::to_value(&unstarted).unwrap();
/// assert!(wire.get("milestone").is_none(), "git does not hold this edge");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IssueTrace {
    /// The issue id, as the project model spells it.
    pub issue: String,
    /// True when the project model declares this id. False is not an error and not an
    /// empty answer: it says the id was looked for and the model does not have it, which a
    /// caller must be able to tell apart from "declared, and nothing has realised it yet".
    /// A repository with no project model at all answers `false` for every id rather than
    /// refusing, because the branches naming an id are still derivable there.
    pub declared: bool,
    /// The milestone the canonical issue record names. Git does not hold this edge and
    /// nothing here derives it: the caller with the index fills it in, and it is `null` in
    /// a repository whose issue record does not name one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    /// The trunk every branch was measured against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trunk: Option<String>,
    /// The branches that name it, local first.
    pub branches: Vec<BranchTrace>,
    /// How many distinct commits the branches hold between them.
    pub commits: usize,
    /// True when every branch's commits could be derived. False when at least one reached
    /// the trunk without a merge commit, so part of the work is not distinguishable.
    pub complete: bool,
}

/// What is known about the contract one commit served.
///
/// Three answers, and two of them are admissions. `Unattributed` is reported and never
/// omitted, because a commit on the trunk that no issue's branch contains is exactly what
/// a traceability report exists to make visible — usually a branch deleted after its merge
/// took its own name with it. `Ambiguous` is the other admission: two issues' branches
/// contain the commit, and picking one of them would be a guess dressed as a fact.
///
/// ```
/// use majordomus_cli::worktree::Attribution;
/// let words: Vec<_> = [
///     Attribution::Attributed,
///     Attribution::Unattributed,
///     Attribution::Ambiguous,
/// ]
/// .iter()
/// .map(|a| serde_json::to_string(a).unwrap())
/// .collect();
/// assert_eq!(words, ["\"attributed\"", "\"unattributed\"", "\"ambiguous\""]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Attribution {
    /// Exactly one issue's branches contain it.
    Attributed,
    /// No branch naming an issue contains it.
    Unattributed,
    /// Branches naming more than one issue contain it.
    Ambiguous,
}

/// One commit and the contract it served, or the fact that none can be found.
///
/// `issue` and `issues` are not a duplication: the list is what was found, and the single
/// field is filled only when the list holds exactly one, so a consumer cannot read an
/// ambiguous verdict as an attribution by looking at the convenient field. `reason` is
/// mandatory for the same purpose as the remedy on a topology diagnostic — a report that
/// says a commit is unattributed without saying why sends the reader back to `git log`.
///
/// ```
/// use majordomus_cli::worktree::{parse_commit, Attribution, CommitAttribution};
/// let commit = parse_commit("abc123\0abc\0A Person\02026-09-09T10:00:00Z\0fix: a thing")
///     .unwrap();
/// let orphan = CommitAttribution {
///     commit,
///     attribution: Attribution::Unattributed,
///     issue: None,
///     milestone: None,
///     issues: Vec::new(),
///     branches: Vec::new(),
///     reason: "no ref naming an issue contains it".into(),
/// };
/// assert!(orphan.issue.is_none() && orphan.issues.is_empty());
/// assert!(!orphan.reason.is_empty(), "the verdict always carries its argument");
/// assert_eq!(orphan.commit.short, "abc");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommitAttribution {
    /// The commit.
    pub commit: CommitRef,
    /// What is known.
    pub attribution: Attribution,
    /// The issue, when exactly one claims it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
    /// The milestone that issue belongs to, filled by the caller that has the index.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestone: Option<String>,
    /// Every issue whose branches contain it: one when attributed, none when unattributed,
    /// more than one when ambiguous.
    pub issues: Vec<String>,
    /// The branches that contain it, by the names their refs carry.
    pub branches: Vec<String>,
    /// Why the verdict is what it is, in one line a person can act on.
    pub reason: String,
}

/// The whole traceability answer: every issue git can say something about, and every commit
/// of a stretch of the trunk with the contract it served or the fact that it has none.
///
/// Read in both directions at once, which is why one type holds both halves: `issues` is
/// the plan seen from the work, `commits` is the work seen from the plan, and a discrepancy
/// between them is the finding. `examined` is bounded — a report is about a stretch of the
/// trunk and never about the whole history — so every count in the tallies is a count over
/// that stretch and must not be read as a statement about the repository.
///
/// ```
/// use majordomus_cli::worktree::{TraceReport, TraceTallies};
/// let empty = TraceReport {
///     trunk: Some("master".into()),
///     examined: 0,
///     issues: Vec::new(),
///     without_branch: vec!["I1400".into()],
///     commits: Vec::new(),
///     tallies: TraceTallies { issues_declared: 1, ..Default::default() },
/// };
/// // an issue nobody has started is listed rather than counted as a problem
/// assert_eq!(empty.without_branch, ["I1400"]);
/// assert_eq!(empty.tallies.issues_with_branch, 0);
/// let wire = serde_json::to_value(&empty).unwrap();
/// assert_eq!(wire["trunk"], "master");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TraceReport {
    /// The trunk every branch and commit was measured against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trunk: Option<String>,
    /// How many trunk commits were attributed.
    pub examined: usize,
    /// The issues that at least one branch names, in id order.
    pub issues: Vec<IssueTrace>,
    /// The issue ids the project model declares that no ref names. Not a fault: an issue
    /// nobody has started has no branch yet.
    pub without_branch: Vec<String>,
    /// The examined trunk commits, newest first.
    pub commits: Vec<CommitAttribution>,
    /// The counts, so a caller need not add them up.
    pub tallies: TraceTallies,
}

/// The counts of one report, added up once so that no surface adds them up differently.
///
/// Every count is over that one report's own scope, and the scope is bounded: the three
/// attribution counts are over the examined stretch of the trunk, so they sum to
/// `examined` and say nothing about the commits before it. `issues_with_branch` is a subset
/// of `issues_declared`, because the ids looked for in ref names are exactly the declared
/// ones — a branch naming something the project model does not have is not an issue.
///
/// ```
/// use majordomus_cli::worktree::TraceTallies;
/// let t = TraceTallies {
///     issues_declared: 12,
///     issues_with_branch: 9,
///     attributed: 40,
///     unattributed: 8,
///     ambiguous: 2,
///     ..Default::default()
/// };
/// assert_eq!(
///     t.attributed + t.unattributed + t.ambiguous,
///     50,
///     "every examined commit falls in exactly one of the three"
/// );
/// assert!(t.issues_with_branch <= t.issues_declared);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TraceTallies {
    /// Issue ids the project model declares.
    pub issues_declared: usize,
    /// Of those, the ones at least one ref names.
    pub issues_with_branch: usize,
    /// Branches naming an issue.
    pub branches: usize,
    /// Distinct commits those branches claim.
    pub branch_commits: usize,
    /// Examined trunk commits with exactly one issue.
    pub attributed: usize,
    /// Examined trunk commits no branch naming an issue contains.
    pub unattributed: usize,
    /// Examined trunk commits more than one issue claims.
    pub ambiguous: usize,
}

/// One ref that names an issue, as `for-each-ref` reported it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueRef {
    /// The ref, short.
    pub name: String,
    /// The commit it points at.
    pub head: String,
    /// True when it is a remote-tracking ref standing in for a branch this clone has no
    /// local copy of.
    pub remote: bool,
    /// The issue its name provably contains.
    pub issue: String,
}

/// The reader. Opened from anywhere inside the repository; every question is answered by
/// running git again, because the history changes outside this process.
///
/// It holds no cache and no index, and that is deliberate: the refs and the object database
/// are the store, another process is committing to them while this one runs, and a cached
/// answer would be a claim about a repository that no longer exists. What it does hold is
/// the two things that are not git's — the work tree to run in and the issue ids to look
/// for — so that every question is asked against one repository and one plan.
///
/// ```no_run
/// use majordomus_cli::worktree::Tracer;
/// use std::path::Path;
///
/// let tracer = Tracer::open(Path::new("/a/foo")).unwrap();
/// let report = tracer.report(50).unwrap();
/// assert_eq!(report.trunk.as_deref(), tracer.trunk(), "one trunk for the whole answer");
/// assert_eq!(report.examined, report.commits.len());
/// ```
#[derive(Debug, Clone)]
pub struct Tracer {
    root: PathBuf,
    trunk: Option<String>,
    issues: Vec<String>,
}

impl Tracer {
    /// Open the tracer for the repository holding `start`. Git runs in the work tree the
    /// caller named: every ref and every commit of the repository is visible from any of
    /// them, so the answer does not depend on which one it is, while reading the project
    /// model from somewhere else would answer about a plan this checkout does not have.
    /// The trunk is the repository's, discovered the way the topology discovers it.
    ///
    /// A repository with no project model opens successfully with no issue ids: the branch
    /// half of the derivation still works there, and refusing would make traceability a
    /// feature only this repository has.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::Tracer;
    /// use std::path::Path;
    ///
    /// // any work tree of the repository; the refs are the same from all of them
    /// let tracer = Tracer::open(Path::new("/a/foo-wt/feature/x")).unwrap();
    /// assert!(tracer.issues().iter().all(|id| !id.is_empty()));
    /// ```
    pub fn open(start: &Path) -> Result<Self> {
        let identity = RepositoryIdentity::discover(start)?;
        let root = identity.current_worktree().path.clone();
        let trunk = identity.trunk().branch.clone();
        // the file names under .ai/repo/project/issues/, of this work tree; a caller that
        // has already parsed the project model replaces them with `with_issues`
        let issues = issue_ids(&root);
        Ok(Tracer {
            root,
            trunk,
            issues,
        })
    }

    /// The issue ids to look for in branch names, from a caller that has already read the
    /// project model — the index, which parsed every issue record, rather than a second
    /// directory listing that could disagree with it.
    ///
    /// It replaces the ids rather than adding to them, which is the point: two lists of
    /// declared issues is the defect this exists to remove, and merging them would keep an
    /// id the project model has dropped.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::Tracer;
    /// use std::path::Path;
    ///
    /// let tracer = Tracer::open(Path::new("/a/foo"))
    ///     .unwrap()
    ///     .with_issues(vec!["I1305".into(), "I1400".into()]);
    /// assert_eq!(tracer.issues(), ["I1305", "I1400"], "the caller's list, not a listing");
    /// ```
    pub fn with_issues(mut self, issues: Vec<String>) -> Self {
        self.issues = issues;
        self
    }

    /// The trunk every measurement is against, when one was discovered.
    pub fn trunk(&self) -> Option<&str> {
        self.trunk.as_deref()
    }

    /// The issue ids the project model declares.
    pub fn issues(&self) -> &[String] {
        &self.issues
    }

    /// Every ref — local branch first, then remote-tracking — whose name provably contains
    /// one of `ids`. A remote-tracking ref whose local counterpart is already in the list is
    /// dropped: it is the same branch seen twice, and counting its commits twice would
    /// inflate every tally that follows.
    fn issue_refs(&self, ids: &[String]) -> Result<Vec<IssueRef>> {
        let out = git::run(
            &self.root,
            &[
                "for-each-ref",
                "refs/heads",
                "refs/remotes",
                "--format=%(refname:short)%00%(objectname)%00%(refname)",
            ],
        )?;
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        Ok(parse_issue_refs(&text, ids))
    }

    /// The commits one ref holds and the trunk does not, with how it reached the trunk.
    fn branch_trace(&self, r: &IssueRef) -> Result<BranchTrace> {
        let Some(trunk) = self.trunk.as_deref() else {
            return Ok(BranchTrace {
                name: r.name.clone(),
                remote: r.remote,
                head: r.head.clone(),
                integration: Integration::Unknown,
                merge_commit: None,
                note: Some(
                    "the trunk could not be determined, so the branch has nothing to be \
                     measured against"
                        .into(),
                ),
                commits: Vec::new(),
            });
        };
        // The trunk as a ref this repository actually has: the local branch, or the
        // remote-tracking one in a clone that never checked it out.
        let trunk_ref = if git::rev_exists(&self.root, trunk)? {
            trunk.to_string()
        } else if git::rev_exists(&self.root, &format!("origin/{trunk}"))? {
            format!("origin/{trunk}")
        } else {
            return Ok(BranchTrace {
                name: r.name.clone(),
                remote: r.remote,
                head: r.head.clone(),
                integration: Integration::Unknown,
                merge_commit: None,
                note: Some(format!(
                    "the trunk `{trunk}` does not resolve in this clone, so the branch has \
                     nothing to be measured against"
                )),
                commits: Vec::new(),
            });
        };

        let merged = crate::git::is_ancestor(&self.root, &r.head, &trunk_ref).unwrap_or(false);
        if !merged {
            let commits = self.log(&[r.name.clone(), format!("^{trunk_ref}")])?;
            return Ok(cut(BranchTrace {
                name: r.name.clone(),
                remote: r.remote,
                head: r.head.clone(),
                integration: Integration::Open,
                merge_commit: None,
                note: None,
                commits,
            }));
        }

        // Merged. The earliest trunk commit descended from the branch is the commit that
        // brought it in; `--ancestry-path` is what makes "descended from" the question
        // rather than "committed after".
        let merge = self.integration_commit(&r.head, &trunk_ref)?;
        let Some((merge, first_parent)) = merge else {
            return Ok(BranchTrace {
                name: r.name.clone(),
                remote: r.remote,
                head: r.head.clone(),
                integration: Integration::Absorbed,
                merge_commit: None,
                note: Some(
                    "the branch reached the trunk with no merge commit of its own — \
                     fast-forwarded, or rebased onto it — so its commits cannot be told \
                     from the trunk's and none are claimed"
                        .into(),
                ),
                commits: Vec::new(),
            });
        };
        let commits = self.log(&[r.name.clone(), format!("^{first_parent}")])?;
        Ok(cut(BranchTrace {
            name: r.name.clone(),
            remote: r.remote,
            head: r.head.clone(),
            integration: Integration::Merged,
            merge_commit: Some(merge),
            note: None,
            commits,
        }))
    }

    /// The merge commit that brought `head` into `trunk_ref`, with its first parent — the
    /// trunk as it stood before the merge, which is what the branch's commits are measured
    /// against. `None` when the branch was absorbed without a merge commit.
    fn integration_commit(&self, head: &str, trunk_ref: &str) -> Result<Option<(String, String)>> {
        // Two runs on purpose. `--ancestry-path` is a history-simplification option, and
        // `--parents` under simplification prints *rewritten* parents — which would name a
        // commit inside the filtered set rather than the trunk as it stood. The candidate
        // is found under simplification; its parents are read back without it.
        let out = git::run(
            &self.root,
            &[
                "rev-list",
                "--ancestry-path",
                "--reverse",
                &format!("{head}..{trunk_ref}"),
            ],
        )?;
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let Some(candidate) = text.lines().map(str::trim).find(|l| !l.is_empty()) else {
            return Ok(None);
        };
        let parents = git::run(&self.root, &["rev-list", "--parents", "-n", "1", candidate])?;
        Ok(first_merge(&String::from_utf8_lossy(&parents.stdout)))
    }

    /// `git log` over a revision specification, in [`FORMAT`], merges excluded.
    fn log(&self, spec: &[String]) -> Result<Vec<CommitRef>> {
        let mut args: Vec<String> = vec![
            "log".into(),
            "--no-merges".into(),
            FORMAT.into(),
            format!("--max-count={}", MAX_COMMITS_PER_BRANCH + 1),
        ];
        args.extend(spec.iter().cloned());
        let out = git::run(&self.root, &args)?;
        Ok(parse_commits(&String::from_utf8_lossy(&out.stdout)))
    }

    /// Every issue at least one ref names, in id order, each with its branches and commits.
    ///
    /// Only the issues something names. An id the project model declares and no ref
    /// mentions is absent from this list rather than present and empty — the report puts
    /// those in `without_branch`, where they read as work not started instead of as work
    /// with no commits.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::Tracer;
    /// use std::path::Path;
    ///
    /// let traces = Tracer::open(Path::new("/a/foo")).unwrap().traces().unwrap();
    /// assert!(
    ///     traces.iter().all(|t| !t.branches.is_empty()),
    ///     "an issue is here because something names it"
    /// );
    /// ```
    pub fn traces(&self) -> Result<Vec<IssueTrace>> {
        let refs = self.issue_refs(&self.issues)?;
        let mut by_issue: BTreeMap<String, Vec<BranchTrace>> = BTreeMap::new();
        for r in &refs {
            by_issue
                .entry(r.issue.clone())
                .or_default()
                .push(self.branch_trace(r)?);
        }
        Ok(by_issue
            .into_iter()
            .map(|(issue, branches)| assemble(issue, self.trunk.clone(), branches))
            .collect())
    }

    /// One issue: its branches and their commits. An id no ref names is a trace with no
    /// branches, not an error — an issue nobody has started yet is a legitimate answer.
    ///
    /// Unlike [`Self::traces`], this asks about an id the caller names, so it also answers
    /// for one the project model does not declare: `declared` is then false, which is a
    /// different fact from an empty branch list and has to be readable separately.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::Tracer;
    /// use std::path::Path;
    ///
    /// let tracer = Tracer::open(Path::new("/a/foo")).unwrap();
    /// let made_up = tracer.trace("I9999").unwrap();
    /// assert_eq!(made_up.issue, "I9999");
    /// assert!(!made_up.declared, "asked for, and the project model has no such issue");
    /// ```
    pub fn trace(&self, issue: &str) -> Result<IssueTrace> {
        let ids = vec![issue.to_string()];
        let refs = self.issue_refs(&ids)?;
        let mut branches = Vec::new();
        for r in &refs {
            branches.push(self.branch_trace(r)?);
        }
        let mut t = assemble(issue.to_string(), self.trunk.clone(), branches);
        t.declared = self.issues.iter().any(|i| i == issue);
        Ok(t)
    }

    /// Resolve a revision to the commit it names, with the fields a report shows. `None`
    /// when nothing in this repository answers to it.
    ///
    /// Any revision git understands — a branch, a tag, an abbreviation, `HEAD~3` — peeled
    /// to a commit, so a tag pointing at a tag object still answers with the commit. An
    /// unknown revision is `None` and not an error: a caller tracing a commit id somebody
    /// pasted is asking whether this repository has it, and that is the answer.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::Tracer;
    /// use std::path::Path;
    ///
    /// let tracer = Tracer::open(Path::new("/a/foo")).unwrap();
    /// let head = tracer.commit("HEAD").unwrap().expect("a checkout has a HEAD commit");
    /// assert!(head.id.starts_with(&head.short), "the abbreviation is a prefix of the id");
    /// assert!(tracer.commit("no-such-revision").unwrap().is_none());
    /// ```
    pub fn commit(&self, rev: &str) -> Result<Option<CommitRef>> {
        let out = git::try_run(
            &self.root,
            &["log", "--no-walk", FORMAT, &format!("{rev}^{{commit}}")],
        )?;
        if out.status != Some(0) {
            return Ok(None);
        }
        Ok(parse_commits(&String::from_utf8_lossy(&out.stdout))
            .into_iter()
            .next())
    }

    /// The newest `limit` commits of the trunk, merges excluded: the work a report
    /// attributes. Merge commits are integration events rather than work, and each is
    /// already named as the `merge_commit` of the branch it brought in.
    ///
    /// `limit` is capped at [`MAX_REPORT_COMMITS`], because a report is about a stretch of
    /// recent trunk and attributing an entire history is a different job. An unknown trunk,
    /// or a trunk that exists only on the remote and not locally, answers with an empty
    /// list rather than an error — there is nothing to attribute, which is not a failure.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::Tracer;
    /// use std::path::Path;
    ///
    /// let tracer = Tracer::open(Path::new("/a/foo")).unwrap();
    /// let recent = tracer.trunk_commits(10).unwrap();
    /// assert!(recent.len() <= 10, "never more than was asked for");
    /// // and never more than MAX_REPORT_COMMITS, whatever was asked for
    /// assert!(tracer.trunk_commits(usize::MAX).unwrap().len() <= 2000);
    /// ```
    pub fn trunk_commits(&self, limit: usize) -> Result<Vec<CommitRef>> {
        let Some(trunk) = self.trunk.as_deref() else {
            return Ok(Vec::new());
        };
        let trunk_ref = if git::rev_exists(&self.root, trunk)? {
            trunk.to_string()
        } else if git::rev_exists(&self.root, &format!("origin/{trunk}"))? {
            format!("origin/{trunk}")
        } else {
            return Ok(Vec::new());
        };
        let out = git::run(
            &self.root,
            &[
                "log",
                "--no-merges",
                FORMAT,
                &format!("--max-count={}", limit.min(MAX_REPORT_COMMITS)),
                &trunk_ref,
            ],
        )?;
        Ok(parse_commits(&String::from_utf8_lossy(&out.stdout)))
    }

    /// The whole answer: every issue with a branch, every issue without one, and the newest
    /// `limit` trunk commits each attributed to the issue whose branches contain it or
    /// reported as having none.
    ///
    /// The tallies are computed here, once, from the same values the report carries, so a
    /// surface never has to add up the arrays and never reaches a different number. Every
    /// commit examined lands in exactly one of the three attribution counts, which is what
    /// makes the summary a partition of `examined` rather than three overlapping filters.
    ///
    /// ```no_run
    /// use majordomus_cli::worktree::Tracer;
    /// use std::path::Path;
    ///
    /// let report = Tracer::open(Path::new("/a/foo")).unwrap().report(50).unwrap();
    /// let t = &report.tallies;
    /// assert_eq!(t.attributed + t.unattributed + t.ambiguous, report.examined);
    /// assert!(report.examined <= 50, "no more than was asked for");
    /// ```
    pub fn report(&self, limit: usize) -> Result<TraceReport> {
        let issues = self.traces()?;
        let named: BTreeSet<&str> = issues.iter().map(|t| t.issue.as_str()).collect();
        let without_branch: Vec<String> = self
            .issues
            .iter()
            .filter(|id| !named.contains(id.as_str()))
            .cloned()
            .collect();
        let commits = attribute(&self.trunk_commits(limit)?, &issues);
        let mut tallies = TraceTallies {
            issues_declared: self.issues.len(),
            issues_with_branch: issues.len(),
            branches: issues.iter().map(|t| t.branches.len()).sum(),
            branch_commits: distinct_commits(&issues),
            ..TraceTallies::default()
        };
        for c in &commits {
            match c.attribution {
                Attribution::Attributed => tallies.attributed += 1,
                Attribution::Unattributed => tallies.unattributed += 1,
                Attribution::Ambiguous => tallies.ambiguous += 1,
            }
        }
        Ok(TraceReport {
            trunk: self.trunk.clone(),
            examined: commits.len(),
            issues,
            without_branch,
            commits,
            tallies,
        })
    }
}

/// The refs of `text` that name one of `ids`, local branches first, each remote-tracking
/// ref dropped when a local branch already stands for the same name.
///
/// ```
/// use majordomus_cli::worktree::parse_issue_refs;
/// let ids = vec!["I1305".to_string()];
/// let text = "feature/I1305-x\0aaa\0refs/heads/feature/I1305-x\n\
///             origin/feature/I1305-x\0aaa\0refs/remotes/origin/feature/I1305-x\n\
///             origin/feature/I1305-y\0bbb\0refs/remotes/origin/feature/I1305-y\n\
///             master\0ccc\0refs/heads/master\n";
/// let refs = parse_issue_refs(text, &ids);
/// assert_eq!(refs.len(), 2);
/// assert_eq!(refs[0].name, "feature/I1305-x");
/// assert!(!refs[0].remote);
/// assert_eq!(refs[1].name, "origin/feature/I1305-y");
/// assert!(refs[1].remote);
/// ```
pub fn parse_issue_refs(text: &str, ids: &[String]) -> Vec<IssueRef> {
    parse_refs(text, ids)
}

/// The typed half of [`parse_issue_refs`].
fn parse_refs(text: &str, ids: &[String]) -> Vec<IssueRef> {
    let mut local: Vec<IssueRef> = Vec::new();
    let mut remote: Vec<IssueRef> = Vec::new();
    let mut local_names: BTreeSet<String> = BTreeSet::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(SEP).collect();
        if f.len() < 3 || f[0].is_empty() {
            continue;
        }
        let (name, head, full) = (f[0], f[1], f[2]);
        // `origin/HEAD` is a symbolic pointer at another ref, never a branch of its own
        if name.ends_with("/HEAD") {
            continue;
        }
        let is_remote = full.starts_with("refs/remotes/");
        if !is_remote {
            local_names.insert(name.to_string());
        }
        let Some(issue) = issue_of(name, ids) else {
            continue;
        };
        let r = IssueRef {
            name: name.to_string(),
            head: head.to_string(),
            remote: is_remote,
            issue,
        };
        if is_remote {
            remote.push(r);
        } else {
            local.push(r);
        }
    }
    // a remote-tracking ref is the same branch as its local counterpart; keep one
    remote.retain(|r| {
        !r.name
            .split_once('/')
            .is_some_and(|(_, rest)| local_names.contains(rest))
    });
    local.sort_by(|a, b| a.name.cmp(&b.name));
    remote.sort_by(|a, b| a.name.cmp(&b.name));
    local.extend(remote);
    local
}

/// The first merge commit of a `rev-list --parents --reverse --ancestry-path` listing, with
/// its first parent. A listing whose first entry has one parent is a branch that reached the
/// trunk without a merge of its own.
///
/// ```
/// use majordomus_cli::worktree::first_merge;
/// assert_eq!(first_merge("m p1 p2\nx m\n"), Some(("m".into(), "p1".into())));
/// assert!(first_merge("ff p\n").is_none());
/// assert!(first_merge("").is_none());
/// ```
pub fn first_merge(text: &str) -> Option<(String, String)> {
    let line = text.lines().find(|l| !l.trim().is_empty())?;
    let mut fields = line.split_whitespace();
    let commit = fields.next()?.to_string();
    let first_parent = fields.next()?.to_string();
    // one parent only: not a merge
    fields.next()?;
    Some((commit, first_parent))
}

/// Cut a branch's commit list at [`MAX_COMMITS_PER_BRANCH`] and say so when it was cut.
fn cut(mut b: BranchTrace) -> BranchTrace {
    if b.commits.len() > MAX_COMMITS_PER_BRANCH {
        b.commits.truncate(MAX_COMMITS_PER_BRANCH);
        b.note = Some(format!(
            "more than {MAX_COMMITS_PER_BRANCH} commits; the list is cut at that and the \
             branch is a history import rather than a piece of work"
        ));
    }
    b
}

/// Assemble one issue's trace from its branches.
fn assemble(issue: String, trunk: Option<String>, branches: Vec<BranchTrace>) -> IssueTrace {
    let declared = true;
    let distinct: BTreeSet<&str> = branches
        .iter()
        .flat_map(|b| b.commits.iter().map(|c| c.id.as_str()))
        .collect();
    let complete = branches
        .iter()
        .all(|b| b.integration != Integration::Absorbed && b.integration != Integration::Unknown);
    IssueTrace {
        issue,
        declared,
        milestone: None,
        trunk,
        commits: distinct.len(),
        complete,
        branches,
    }
}

/// How many distinct commits a set of traces claims between them.
fn distinct_commits(traces: &[IssueTrace]) -> usize {
    traces
        .iter()
        .flat_map(|t| t.branches.iter())
        .flat_map(|b| b.commits.iter().map(|c| c.id.as_str()))
        .collect::<BTreeSet<&str>>()
        .len()
}

/// Attribute every commit to the issue whose branches contain it, or report that none does.
///
/// This is the reverse of the same edge and reads nothing else: the branch sets were derived
/// from git a moment ago, and a commit is attributed because a branch that names an issue
/// holds it, never because a record said so.
pub fn attribute(commits: &[CommitRef], traces: &[IssueTrace]) -> Vec<CommitAttribution> {
    let mut owners: BTreeMap<&str, BTreeSet<(&str, &str)>> = BTreeMap::new();
    // A commit belongs to the issue whose branch carries it — and a merge commit belongs to
    // the issue whose branch it merged, though no branch contains it. The merge commit is
    // the one a reader is most likely to hold: it is what lands on the trunk and what a
    // release note names. Leaving it unattributed answered "which outcome did this serve?"
    // with silence for exactly the commit the question is usually asked about.
    for t in traces {
        for b in &t.branches {
            for c in &b.commits {
                owners
                    .entry(c.id.as_str())
                    .or_default()
                    .insert((t.issue.as_str(), b.name.as_str()));
            }
            if let Some(m) = b.merge_commit.as_deref() {
                owners
                    .entry(m)
                    .or_default()
                    .insert((t.issue.as_str(), b.name.as_str()));
            }
        }
    }
    let milestone_of = |issue: &str| {
        traces
            .iter()
            .find(|t| t.issue == issue)
            .and_then(|t| t.milestone.clone())
    };
    commits
        .iter()
        .map(|c| {
            let found = owners.get(c.id.as_str());
            let issues: Vec<String> = found
                .map(|s| {
                    s.iter()
                        .map(|(i, _)| (*i).to_string())
                        .collect::<BTreeSet<String>>()
                        .into_iter()
                        .collect()
                })
                .unwrap_or_default();
            let branches: Vec<String> = found
                .map(|s| s.iter().map(|(_, b)| (*b).to_string()).collect())
                .unwrap_or_default();
            let (attribution, issue, reason) = match issues.len() {
                0 => (
                    Attribution::Unattributed,
                    None,
                    "no ref naming an issue contains it; either it was committed without an \
                     execution contract, or the branch that carried it has been deleted — a \
                     deleted head branch survives on GitHub, which `scripts/traceability` \
                     reads and this executable does not"
                        .to_string(),
                ),
                1 => (
                    Attribution::Attributed,
                    Some(issues[0].clone()),
                    format!("{} contains it", branches.join(", ")),
                ),
                _ => (
                    Attribution::Ambiguous,
                    None,
                    format!(
                        "branches naming {} issues contain it ({}); the branch-name edge \
                         cannot decide which contract it served",
                        issues.len(),
                        branches.join(", ")
                    ),
                ),
            };
            let milestone = issue.as_deref().and_then(milestone_of);
            CommitAttribution {
                commit: c.clone(),
                attribution,
                issue,
                milestone,
                issues,
                branches,
                reason,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(id: &str) -> CommitRef {
        CommitRef {
            id: id.into(),
            short: id[..3.min(id.len())].into(),
            author: "A".into(),
            date: "2026-09-09T00:00:00+00:00".into(),
            subject: format!("work {id}"),
        }
    }

    fn trace(issue: &str, milestone: Option<&str>, branch: &str, ids: &[&str]) -> IssueTrace {
        let mut t = assemble(
            issue.into(),
            Some("master".into()),
            vec![BranchTrace {
                name: branch.into(),
                remote: false,
                head: ids.last().copied().unwrap_or("x").into(),
                integration: Integration::Open,
                merge_commit: None,
                note: None,
                commits: ids.iter().map(|i| commit(i)).collect(),
            }],
        );
        t.milestone = milestone.map(str::to_string);
        t
    }

    #[test]
    fn a_commit_no_issue_branch_holds_is_reported_not_omitted() {
        let traces = vec![trace(
            "I1305",
            Some("work-graph"),
            "feature/I1305-x",
            &["aaa"],
        )];
        let out = attribute(&[commit("aaa"), commit("zzz")], &traces);
        assert_eq!(out.len(), 2, "every examined commit is in the answer");
        assert_eq!(out[0].attribution, Attribution::Attributed);
        assert_eq!(out[0].issue.as_deref(), Some("I1305"));
        assert_eq!(out[0].milestone.as_deref(), Some("work-graph"));
        assert_eq!(out[1].attribution, Attribution::Unattributed);
        assert!(out[1].issue.is_none());
        assert!(out[1].reason.contains("no ref naming an issue"));
    }

    #[test]
    fn two_issues_claiming_one_commit_is_ambiguous_not_a_guess() {
        let traces = vec![
            trace("I1305", None, "feature/I1305-x", &["aaa"]),
            trace("I1306", None, "feature/I1306-y", &["aaa"]),
        ];
        let out = attribute(&[commit("aaa")], &traces);
        assert_eq!(out[0].attribution, Attribution::Ambiguous);
        assert!(out[0].issue.is_none());
        assert_eq!(out[0].issues, vec!["I1305", "I1306"]);
    }

    #[test]
    fn a_remote_ref_whose_local_branch_exists_is_the_same_branch_once() {
        let ids = vec!["I1305".to_string()];
        let text = "feature/I1305-x\u{0}aaa\u{0}refs/heads/feature/I1305-x\n\
                    origin/feature/I1305-x\u{0}aaa\u{0}refs/remotes/origin/feature/I1305-x\n";
        let refs = parse_refs(text, &ids);
        assert_eq!(refs.len(), 1);
        assert!(!refs[0].remote);
    }

    #[test]
    fn a_branch_only_the_remote_still_has_is_kept_and_marked() {
        let ids = vec!["I1305".to_string()];
        let text = "origin/feature/I1305-x\u{0}aaa\u{0}refs/remotes/origin/feature/I1305-x\n\
                    origin/HEAD\u{0}aaa\u{0}refs/remotes/origin/HEAD\n";
        let refs = parse_refs(text, &ids);
        assert_eq!(refs.len(), 1);
        assert!(refs[0].remote);
        assert_eq!(refs[0].issue, "I1305");
    }

    #[test]
    fn an_absorbed_branch_claims_nothing_rather_than_claiming_the_trunk() {
        assert!(first_merge("ff parent-only\n").is_none());
        let t = assemble(
            "I1305".into(),
            Some("master".into()),
            vec![BranchTrace {
                name: "feature/I1305-x".into(),
                remote: false,
                head: "aaa".into(),
                integration: Integration::Absorbed,
                merge_commit: None,
                note: Some("absorbed".into()),
                commits: Vec::new(),
            }],
        );
        assert_eq!(t.commits, 0);
        assert!(!t.complete, "an absorbed branch makes the trace incomplete");
    }

    #[test]
    fn one_commit_on_two_branches_of_one_issue_is_counted_once() {
        let t = assemble(
            "I1305".into(),
            Some("master".into()),
            vec![
                BranchTrace {
                    name: "feature/I1305-x".into(),
                    remote: false,
                    head: "aaa".into(),
                    integration: Integration::Open,
                    merge_commit: None,
                    note: None,
                    commits: vec![commit("aaa")],
                },
                BranchTrace {
                    name: "origin/feature/I1305-z".into(),
                    remote: true,
                    head: "aaa".into(),
                    integration: Integration::Open,
                    merge_commit: None,
                    note: None,
                    commits: vec![commit("aaa")],
                },
            ],
        );
        assert_eq!(t.commits, 1);
        assert!(t.complete);
    }

    #[test]
    fn a_subject_holding_the_separator_cannot_break_a_commit_line() {
        // git never writes a NUL into a subject; the parser still keeps the whole tail
        let c = parse_commit("a\u{0}b\u{0}c\u{0}d\u{0}sub\u{0}ject").unwrap();
        assert_eq!(c.subject, "subject");
    }
}
