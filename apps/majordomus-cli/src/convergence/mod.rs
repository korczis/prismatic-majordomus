//! Where the work of this repository is being held, and whether any of it is held
//! somewhere it can be lost.
//!
//! # The problem this exists for
//!
//! `project.a-worker-that-stops-leaves-its-work-behind` is written down as advisory, and
//! says why: *"nothing can see the uncommitted half except somebody looking, which is
//! exactly why the responsibility is written down rather than gated."* That premise was
//! true when the rule was written and is not true now. [`crate::worktree`] already counts
//! the uncommitted work of every registered worktree, and git answers in one call which
//! commits of this repository have never reached a remote. What was missing was not a
//! measurement but a **verdict** over the repository: one word that says whether every
//! unit of work is somewhere another worker could find it.
//!
//! # What a holding is
//!
//! A holding is anything that can hold work: a worktree (its uncommitted files), a branch
//! (its commits), a stash (its diff). Each is given a [`Disposition`] from the evidence,
//! and a holding whose disposition is [`Disposition::LocalOnly`] or
//! [`Disposition::Uncommitted`] is **at risk**: it exists on one disk, it is invisible to
//! every other worker, and a worker that stops is not a rollback.
//!
//! # What this is not
//!
//! It is not a second reading of the topology — [`crate::worktree::WorktreeService`] is
//! the only one, and this composes its answer. It is not a pull-request inventory: a pull
//! request lives on a forge, reaching one is a network call, and the completion invariant
//! already asks whether the trunk reaches a commit (`integrated`). This answers the half
//! that is decidable offline, from this repository alone, in one pass.
//!
//! ```
//! use majordomus_cli::convergence::Disposition;
//!
//! // the two dispositions that mean "this exists on one disk only"
//! assert!(Disposition::LocalOnly.at_risk() && Disposition::Uncommitted.at_risk());
//! // and the two that mean somebody else could pick it up
//! assert!(!Disposition::Integrated.at_risk() && !Disposition::Published.at_risk());
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::worktree::{git, Detail, Result, WorktreeService};

/// The schema this module answers under.
pub const SCHEMA: &str = "convergence/v1";

// ---------------------------------------------------------------- vocabulary

/// What kind of thing is holding the work.
///
/// ```
/// use majordomus_cli::convergence::HoldingKind;
///
/// // the word a report and a persisted record use; renaming a variant must not rename it
/// assert_eq!(HoldingKind::Worktree.as_str(), "worktree");
/// assert_eq!(serde_json::to_string(&HoldingKind::Stash).unwrap(), "\"stash\"");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HoldingKind {
    /// A registered work tree, holding files that are not committed.
    Worktree,
    /// A local branch, holding commits.
    Branch,
    /// An entry of `git stash`.
    Stash,
}

impl HoldingKind {
    /// The word this kind is reported under — the same word its serialisation carries.
    ///
    /// ```
    /// use majordomus_cli::convergence::HoldingKind;
    ///
    /// for kind in [HoldingKind::Worktree, HoldingKind::Branch, HoldingKind::Stash] {
    ///     let json = serde_json::to_string(&kind).unwrap();
    ///     assert_eq!(json, format!("\"{}\"", kind.as_str()));
    /// }
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            HoldingKind::Worktree => "worktree",
            HoldingKind::Branch => "branch",
            HoldingKind::Stash => "stash",
        }
    }
}

/// Where the work a holding carries can be reached from.
///
/// The order is the order of safety: everything above [`Disposition::LocalOnly`] is
/// reachable by somebody who is not the worker that made it.
///
/// ```
/// use majordomus_cli::convergence::Disposition;
///
/// // declared in the order of safety, so the derived order says which is worse
/// assert!(Disposition::Integrated < Disposition::Published);
/// assert!(Disposition::Published < Disposition::LocalOnly);
/// assert_eq!(serde_json::to_string(&Disposition::LocalOnly).unwrap(), "\"local_only\"");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    /// The trunk reaches it: it is in the repository's own history.
    Integrated,
    /// A remote reaches it: another checkout can fetch it, and it survives this disk.
    Published,
    /// Committed here and nowhere else. One disk away from being lost.
    LocalOnly,
    /// Not committed at all: files in a work tree, or a stash entry.
    Uncommitted,
}

impl Disposition {
    /// The word this disposition is reported under — the same word its serialisation
    /// carries, so a tally keyed by it and a holding carrying it agree.
    ///
    /// ```
    /// use majordomus_cli::convergence::Disposition;
    ///
    /// assert_eq!(Disposition::LocalOnly.as_str(), "local_only");
    /// let json = serde_json::to_string(&Disposition::Uncommitted).unwrap();
    /// assert_eq!(json, format!("\"{}\"", Disposition::Uncommitted.as_str()));
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            Disposition::Integrated => "integrated",
            Disposition::Published => "published",
            Disposition::LocalOnly => "local_only",
            Disposition::Uncommitted => "uncommitted",
        }
    }

    /// This holding's work exists on one disk only: the verdict refuses over exactly these.
    ///
    /// ```
    /// use majordomus_cli::convergence::Disposition;
    ///
    /// let risky: Vec<_> = [
    ///     Disposition::Integrated,
    ///     Disposition::Published,
    ///     Disposition::LocalOnly,
    ///     Disposition::Uncommitted,
    /// ]
    /// .into_iter()
    /// .filter(Disposition::at_risk)
    /// .collect();
    /// assert_eq!(risky, [Disposition::LocalOnly, Disposition::Uncommitted]);
    /// ```
    pub fn at_risk(&self) -> bool {
        matches!(self, Disposition::LocalOnly | Disposition::Uncommitted)
    }
}

// ---------------------------------------------------------------- the documents

/// One thing holding work, and what is known about where that work can be reached from.
///
/// ```
/// use majordomus_cli::convergence::{Disposition, Holding, HoldingKind};
///
/// let branch = Holding {
///     kind: HoldingKind::Branch,
///     identity: "feature/x".into(),
///     disposition: Disposition::LocalOnly,
///     evidence: "abc123 is on no remote-tracking ref of this checkout".into(),
///     remedy: "git push -u origin feature/x".into(),
///     at_risk: true,
/// };
/// // an at-risk holding always carries the command that would move it out of danger
/// assert!(branch.at_risk && !branch.remedy.is_empty());
/// // and a reachable one carries none, so the field is left out of the document
/// let safe = Holding { disposition: Disposition::Published, remedy: String::new(), at_risk: false, ..branch };
/// assert!(!serde_json::to_string(&safe).unwrap().contains("remedy"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Holding {
    /// What kind of thing holds it.
    pub kind: HoldingKind,
    /// How it is named: a branch name, an absolute worktree path, a stash ref.
    pub identity: String,
    /// Where the work can be reached from.
    pub disposition: Disposition,
    /// What was actually read — never a restatement of the disposition.
    pub evidence: String,
    /// The command that would move it out of danger. Empty when it is not at risk.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub remedy: String,
    /// This holding's work exists on one disk only.
    pub at_risk: bool,
}

/// The repository's convergence: every holding, and the one verdict over them.
///
/// ```
/// use majordomus_cli::convergence::{ConvergenceReport, SCHEMA};
///
/// let empty = ConvergenceReport {
///     schema: SCHEMA.into(),
///     repository: "/tmp/r".into(),
///     trunk: Some("master".into()),
///     holdings: vec![],
///     tallies: Default::default(),
///     at_risk: 0,
///     converged: true,
/// };
/// // the verdict is the one word a gate reads; the count is what a person reads next to it
/// assert!(empty.converged && empty.at_risk == 0);
/// assert_eq!(serde_json::to_value(&empty).unwrap()["schema"], "convergence/v1");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ConvergenceReport {
    /// [`SCHEMA`].
    pub schema: String,
    /// The primary checkout this was measured from.
    pub repository: String,
    /// The trunk the containment was decided against, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trunk: Option<String>,
    /// Every holding, at-risk ones first, then by kind and identity.
    pub holdings: Vec<Holding>,
    /// How many holdings carry each disposition, by its word.
    pub tallies: BTreeMap<String, usize>,
    /// The holdings that exist on one disk only.
    pub at_risk: usize,
    /// Nothing is held where it can be lost.
    pub converged: bool,
}

impl ConvergenceReport {
    /// The one line a person or a gate reads first, stating the count it refuses over.
    ///
    /// ```
    /// use majordomus_cli::convergence::{ConvergenceReport, SCHEMA};
    ///
    /// let mut report = ConvergenceReport {
    ///     schema: SCHEMA.into(),
    ///     repository: "/tmp/r".into(),
    ///     trunk: None,
    ///     holdings: vec![],
    ///     tallies: Default::default(),
    ///     at_risk: 0,
    ///     converged: true,
    /// };
    /// assert!(report.summary().starts_with("converged"));
    /// report.at_risk = 2;
    /// report.converged = false;
    /// assert!(report.summary().starts_with("not converged: 2 of"));
    /// ```
    pub fn summary(&self) -> String {
        if self.converged {
            return format!(
                "converged: {} holding(s), every one reachable by somebody else",
                self.holdings.len()
            );
        }
        format!(
            "not converged: {} of {} holding(s) exist on this disk only",
            self.at_risk,
            self.holdings.len()
        )
    }
}

/// One collection, one order, declared once: at-risk holdings first (the group), then by
/// kind in the order a worker meets them (a work tree, its branch, a stash), then by
/// identity. Every surface that lists holdings shows this order, because none of them sorts.
impl crate::order::Ordered for Holding {
    fn order_key(&self) -> crate::order::OrderKey<'_> {
        let group = if self.at_risk { "at-risk" } else { "reachable" };
        let rank = match self.kind {
            HoldingKind::Worktree => 1,
            HoldingKind::Branch => 2,
            HoldingKind::Stash => 3,
        };
        crate::order::OrderKey::grouped(group, &self.identity, &self.identity).ranked(rank)
    }
}

// ---------------------------------------------------------------- the measurement

/// Measure the repository holding `root`.
///
/// One topology read (the same service the hooks and the Cockpit ask), one `git rev-list`
/// for the commits no remote reaches, one `git stash list`. Nothing here writes, and
/// nothing here reaches the network: a remote-tracking ref is what this checkout last
/// fetched, which is exactly the question — whether the work left this disk, not whether
/// the remote still has it.
///
/// ```
/// use std::process::Command;
/// use majordomus_cli::convergence::{report, Disposition};
///
/// let dir = std::env::temp_dir().join(format!("mj-convergence-doc-{}", std::process::id()));
/// let _ = std::fs::remove_dir_all(&dir);
/// std::fs::create_dir_all(&dir).unwrap();
/// let git = |args: &[&str]| {
///     let ok = Command::new("git").current_dir(&dir).args(args).status().unwrap().success();
///     assert!(ok, "git {args:?}");
/// };
/// git(&["init", "-q", "-b", "master"]);
/// let commit = |msg: &str| git(&["-c", "user.email=d@example.com", "-c", "user.name=doc",
///                                "commit", "-q", "--allow-empty", "-m", msg]);
/// commit("base");
/// git(&["checkout", "-q", "-b", "feature/doc"]);
/// commit("work nobody else has");
/// git(&["checkout", "-q", "master"]);
///
/// // a commit on a branch the trunk does not reach and no remote has seen is on one disk
/// let verdict = report(&dir).unwrap();
/// assert!(!verdict.converged);
/// assert!(verdict.holdings.iter().any(|h| h.disposition == Disposition::LocalOnly));
///
/// // and a file never committed is the other half
/// std::fs::write(dir.join("draft.txt"), "half written").unwrap();
/// let verdict = report(&dir).unwrap();
/// assert!(verdict.holdings.iter().any(|h| h.disposition == Disposition::Uncommitted));
/// std::fs::remove_dir_all(&dir).unwrap();
/// ```
pub fn report(root: &Path) -> Result<ConvergenceReport> {
    let topology = WorktreeService::open(root).and_then(|s| s.topology(Detail::Full))?;
    let primary = topology.repository.primary_worktree.clone();
    let unreachable = commits_no_remote_reaches(Path::new(&primary))?;
    let stashes = stash_entries(Path::new(&primary))?;

    let mut holdings = Vec::new();

    for worktree in &topology.worktrees {
        let Some(dirty) = worktree.dirty.as_ref() else {
            continue;
        };
        if dirty.clean {
            continue;
        }
        holdings.push(Holding {
            kind: HoldingKind::Worktree,
            identity: worktree.path.clone(),
            disposition: Disposition::Uncommitted,
            evidence: format!("{} — {}", worktree.label, dirty.summary()),
            remedy: format!(
                "git -C {} add -A && git -C {} commit -m 'checkpoint: unreviewed'",
                worktree.path, worktree.path
            ),
            at_risk: true,
        });
    }

    for branch in &topology.branches {
        let (disposition, evidence) = if branch.merged_into_trunk == Some(true) {
            (
                Disposition::Integrated,
                format!("the trunk reaches {}", short(&branch.head)),
            )
        } else if unreachable.contains(&branch.head) {
            (
                Disposition::LocalOnly,
                format!(
                    "{} is on no remote-tracking ref of this checkout",
                    short(&branch.head)
                ),
            )
        } else {
            (
                Disposition::Published,
                format!("a remote-tracking ref reaches {}", short(&branch.head)),
            )
        };
        let at_risk = disposition.at_risk();
        holdings.push(Holding {
            kind: HoldingKind::Branch,
            identity: branch.name.clone(),
            disposition,
            evidence,
            remedy: if at_risk {
                format!("git push -u origin {}", branch.name)
            } else {
                String::new()
            },
            at_risk,
        });
    }

    for (reference, subject) in stashes {
        holdings.push(Holding {
            kind: HoldingKind::Stash,
            identity: reference.clone(),
            disposition: Disposition::Uncommitted,
            evidence: subject,
            remedy: format!("git stash show -p {reference}"),
            at_risk: true,
        });
    }

    crate::order::canonical(&mut holdings);

    let mut tallies: BTreeMap<String, usize> = BTreeMap::new();
    for holding in &holdings {
        *tallies
            .entry(holding.disposition.as_str().to_string())
            .or_default() += 1;
    }
    let at_risk = holdings.iter().filter(|h| h.at_risk).count();

    Ok(ConvergenceReport {
        schema: SCHEMA.to_string(),
        repository: primary,
        trunk: topology.trunk.branch.clone(),
        holdings,
        tallies,
        at_risk,
        converged: at_risk == 0,
    })
}

/// The commits that are on a local branch and on no remote-tracking ref: the one reading
/// [`crate::worktree::state::commits_no_remote_reaches`] takes, shared with the
/// reconciliation so that "published" means one thing in both.
fn commits_no_remote_reaches(primary: &Path) -> Result<BTreeSet<String>> {
    crate::worktree::state::commits_no_remote_reaches(primary)
}

/// Every stash entry: its ref and the subject git records for it.
fn stash_entries(primary: &Path) -> Result<Vec<(String, String)>> {
    text_of(primary, &["stash", "list", "--format=%gd%x09%gs"]).map(|text| stash_list(&text))
}

/// What git answered to `args`, as text. A command that failed and an answer that is not
/// UTF-8 are both refusals: neither is an empty answer.
fn text_of(primary: &Path, args: &[&str]) -> Result<String> {
    git::run(primary, args)?.text()
}

/// The entries of `git stash list --format=%gd%x09%gs`: a reference and its subject per
/// line. A line with no reference names no entry and is skipped rather than invented.
fn stash_list(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let (reference, subject) = line.split_once('\t')?;
            if reference.trim().is_empty() {
                return None;
            }
            Some((reference.trim().to_string(), subject.trim().to_string()))
        })
        .collect()
}

/// A commit, abbreviated the way git abbreviates it in prose.
fn short(commit: &str) -> &str {
    let end = commit.len().min(9);
    &commit[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The vocabulary is the contract: a reader that persists these words must see the
    /// same ones tomorrow.
    #[test]
    fn the_words_are_stable() {
        assert_eq!(Disposition::Integrated.as_str(), "integrated");
        assert_eq!(Disposition::Published.as_str(), "published");
        assert_eq!(Disposition::LocalOnly.as_str(), "local_only");
        assert_eq!(Disposition::Uncommitted.as_str(), "uncommitted");
        assert_eq!(HoldingKind::Worktree.as_str(), "worktree");
        assert_eq!(HoldingKind::Branch.as_str(), "branch");
        assert_eq!(HoldingKind::Stash.as_str(), "stash");
    }

    /// Exactly the two dispositions that mean "one disk" are at risk. This is the
    /// judgement the gate rests on, so it is asserted rather than assumed.
    #[test]
    fn at_risk_is_exactly_the_unreachable_two() {
        let all = [
            Disposition::Integrated,
            Disposition::Published,
            Disposition::LocalOnly,
            Disposition::Uncommitted,
        ];
        let at_risk: Vec<&str> = all
            .iter()
            .filter(|d| d.at_risk())
            .map(|d| d.as_str())
            .collect();
        assert_eq!(at_risk, vec!["local_only", "uncommitted"]);
    }

    /// A summary states the count it refuses over, so a person reading the one line knows
    /// how much is in danger without reading the list.
    #[test]
    fn the_summary_states_the_count_it_refuses_over() {
        let mut report = ConvergenceReport {
            schema: SCHEMA.into(),
            repository: "/tmp/r".into(),
            trunk: Some("master".into()),
            holdings: vec![Holding {
                kind: HoldingKind::Branch,
                identity: "feature/x".into(),
                disposition: Disposition::LocalOnly,
                evidence: "abc on no remote".into(),
                remedy: "git push -u origin feature/x".into(),
                at_risk: true,
            }],
            tallies: BTreeMap::new(),
            at_risk: 1,
            converged: false,
        };
        assert!(report.summary().contains("1 of 1"));
        report.at_risk = 0;
        report.converged = true;
        assert!(report.summary().starts_with("converged"));
    }

    /// An abbreviation never panics on a short or empty commit: the topology can answer
    /// with an empty head for a branch git could not resolve.
    #[test]
    fn abbreviating_a_short_commit_is_not_a_panic() {
        assert_eq!(short(""), "");
        assert_eq!(short("abc"), "abc");
        assert_eq!(short("0123456789abcdef"), "012345678");
    }

    /// Run git in `dir` and insist it succeeded: a fixture that half-built is a test that
    /// asserts about something else.
    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .current_dir(dir)
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
            .expect("git runs");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A repository with one commit on `master` and a tracked file to change.
    fn repository() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "master"]);
        std::fs::write(repo.join("a.txt"), "one\n").unwrap();
        git(&repo, &["add", "a.txt"]);
        git(&repo, &["commit", "-q", "-m", "base"]);
        dir
    }

    /// Point a ref at an object this repository does not have, the way a crashed write or a
    /// half-copied `.git` leaves one.
    fn break_ref(repo: &Path, name: &str) {
        let path = repo.join(".git").join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "1111111111111111111111111111111111111111\n").unwrap();
    }

    /// Every kind of holding gets the disposition git's evidence gives it: the trunk is
    /// integrated, a pushed branch is published, a branch no remote has seen is local only,
    /// and a stash and a dirty work tree are uncommitted. A registered work tree whose
    /// directory is gone holds nothing that could be lost and is not a holding.
    #[test]
    fn every_holding_is_given_the_disposition_its_evidence_supports() {
        let dir = repository();
        let repo = dir.path().join("repo");
        let origin = dir.path().join("origin.git");
        git(
            dir.path(),
            &["init", "-q", "--bare", origin.to_str().unwrap()],
        );
        git(
            &repo,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        git(&repo, &["push", "-q", "origin", "master"]);

        git(&repo, &["checkout", "-q", "-b", "feature/pushed"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "shared"]);
        git(&repo, &["push", "-q", "-u", "origin", "feature/pushed"]);
        git(&repo, &["checkout", "-q", "-b", "feature/local"]);
        git(
            &repo,
            &[
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "nobody else has this",
            ],
        );
        let local_head = git(&repo, &["rev-parse", "HEAD"]);
        git(&repo, &["checkout", "-q", "master"]);

        // a registered work tree whose directory was removed by hand
        let gone = dir.path().join("gone");
        git(
            &repo,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feature/gone",
                gone.to_str().unwrap(),
            ],
        );
        std::fs::remove_dir_all(&gone).unwrap();

        std::fs::write(repo.join("a.txt"), "two\n").unwrap();
        git(&repo, &["stash", "push", "-q", "-m", "parked work"]);
        std::fs::write(repo.join("draft.txt"), "half written\n").unwrap();

        let verdict = report(&repo).unwrap();
        let of = |kind: HoldingKind, identity: &str| {
            verdict
                .holdings
                .iter()
                .find(|h| h.kind == kind && h.identity == identity)
                .unwrap_or_else(|| panic!("no {identity}: {:#?}", verdict.holdings))
                .clone()
        };

        let local = of(HoldingKind::Branch, "feature/local");
        assert_eq!(local.disposition, Disposition::LocalOnly);
        assert!(local.at_risk);
        assert_eq!(
            local.evidence,
            format!(
                "{} is on no remote-tracking ref of this checkout",
                &local_head[..9]
            )
        );
        assert_eq!(local.remedy, "git push -u origin feature/local");

        let pushed = of(HoldingKind::Branch, "feature/pushed");
        assert_eq!(pushed.disposition, Disposition::Published);
        assert!(!pushed.at_risk);
        assert!(
            pushed
                .evidence
                .starts_with("a remote-tracking ref reaches "),
            "{}",
            pushed.evidence
        );
        assert_eq!(
            pushed.remedy, "",
            "nothing to do for what is already shared"
        );

        assert_eq!(
            of(HoldingKind::Branch, "master").disposition,
            Disposition::Integrated
        );

        let stash = of(HoldingKind::Stash, "stash@{0}");
        assert_eq!(stash.disposition, Disposition::Uncommitted);
        assert!(stash.at_risk);
        assert_eq!(stash.evidence, "On master: parked work");
        assert_eq!(stash.remedy, "git stash show -p stash@{0}");

        let worktrees: Vec<&Holding> = verdict
            .holdings
            .iter()
            .filter(|h| h.kind == HoldingKind::Worktree)
            .collect();
        assert_eq!(worktrees.len(), 1, "only the dirty one: {worktrees:#?}");
        assert_eq!(worktrees[0].disposition, Disposition::Uncommitted);
        assert!(!worktrees[0].identity.ends_with("gone"));

        assert!(!verdict.converged);
        assert_eq!(
            verdict.at_risk, 3,
            "the local branch, the stash, the work tree"
        );
        assert_eq!(verdict.tallies.get("local_only"), Some(&1));
        assert_eq!(verdict.tallies.get("uncommitted"), Some(&2));
        assert_eq!(verdict.tallies.get("published"), Some(&1));
        // what is at risk is listed before what is not, and within the at-risk group the
        // order is the kind's rank: the work tree, then the branch, then the stash
        let at_risk: Vec<HoldingKind> = verdict
            .holdings
            .iter()
            .take_while(|h| h.at_risk)
            .map(|h| h.kind)
            .collect();
        assert_eq!(
            at_risk,
            vec![
                HoldingKind::Worktree,
                HoldingKind::Branch,
                HoldingKind::Stash
            ]
        );
    }

    /// A repository git cannot read in full is an error, never a verdict over the part it
    /// could read: a broken branch ref stops the topology, a broken remote-tracking ref stops
    /// the reachability question, and a broken stash ref stops the stash list.
    #[test]
    fn a_repository_git_cannot_read_is_an_error_and_never_a_verdict() {
        for broken in [
            "refs/heads/broken",
            "refs/remotes/origin/broken",
            "refs/stash",
        ] {
            let dir = repository();
            let repo = dir.path().join("repo");
            break_ref(&repo, broken);
            let refused = report(&repo).expect_err(broken);
            assert_eq!(refused.code(), "GitCommandFailed", "{broken}: {refused}");
        }
    }

    /// The stash list is read line by line; a line that carries no reference is not an
    /// entry, and the subject keeps everything after the first tab.
    #[test]
    fn a_stash_line_without_a_reference_names_no_entry() {
        assert_eq!(
            stash_list("stash@{0}\tOn master: a\tb\nno tab here\n\tOn master: orphan\n"),
            vec![("stash@{0}".to_string(), "On master: a\tb".to_string())]
        );
        assert!(stash_list("").is_empty());
    }
}
